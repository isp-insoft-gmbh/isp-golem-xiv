use std::{cell::Cell, fmt::Write};

use base64::{Engine, engine::general_purpose::STANDARD};
use futures_util::{
    StreamExt,
    future::{Either, select},
    pin_mut,
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use wasm_bindgen::prelude::*;
use worker::wasm_bindgen_futures::JsFuture;
use worker::{
    AbortController, Context, Env, Fetch, Headers, Method, Request, RequestInit, Response,
};

use crate::{
    APP, COMMENT_BYTES_MAX, Delivery, DispatchV1, EventDelivery, FILE_BYTES_MAX, GOLEMS_MAX, Golem,
    OPS, ORGANIZATION, TEST_TOPIC, event_seat, next_schedule_minute, occurrence_id, parse_golem,
    parse_mention, refusal, seat_secret, trigger_matches, valid_delivery, valid_repository,
    verify_queue_signature, verify_signature,
};

#[wasm_bindgen(module = "/src/runtime.js")]
extern "C" {
    #[wasm_bindgen(js_name = appJwt)]
    fn app_jwt(pem: &str) -> worker::js_sys::Promise;
    #[wasm_bindgen(js_name = makeDeadline)]
    fn make_deadline(milliseconds: u32) -> JsValue;
    #[wasm_bindgen(js_name = deadlinePromise)]
    fn deadline_promise(deadline: &JsValue) -> worker::js_sys::Promise;
    #[wasm_bindgen(js_name = cancelDeadline)]
    fn cancel_deadline(deadline: &JsValue);
    #[wasm_bindgen(js_name = randomState)]
    fn random_state() -> String;
}

type Result<T> = std::result::Result<T, worker::Error>;
const BODY_BYTES_MAX: usize = 262_144;
const RESPONSE_BYTES_MAX: usize = 524_288;
const SUBREQUESTS_MAX: u32 = 45;

fn failure(message: &'static str) -> worker::Error {
    worker::Error::RustError(message.into())
}

struct Deadline(JsValue);
impl Drop for Deadline {
    fn drop(&mut self) {
        cancel_deadline(&self.0);
    }
}

async fn bounded<T>(
    future: impl std::future::Future<Output = Result<T>>,
    milliseconds: u32,
) -> Result<T> {
    // Current Workers timers can return opaque handles; worker::Delay assumes i32.
    let deadline = Deadline(make_deadline(milliseconds));
    let timer = JsFuture::from(deadline_promise(&deadline.0));
    pin_mut!(future, timer);
    match select(future, timer).await {
        Either::Left((result, _)) => result,
        Either::Right(_) => Err(failure("operation deadline exceeded")),
    }
}

async fn read_stream(mut stream: worker::ByteStream, maximum: usize) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    // Also bound chunk count, including empty chunks from a hostile streaming client.
    for _ in 0..4096 {
        let Some(chunk) = stream.next().await else {
            return Ok(bytes);
        };
        let chunk = chunk?;
        if chunk.len() > maximum.saturating_sub(bytes.len()) {
            return Err(failure("body limit exceeded"));
        }
        bytes.extend_from_slice(&chunk);
    }
    Err(failure("body chunk limit exceeded"))
}

struct AbortOnDrop(Option<AbortController>);

impl Drop for AbortOnDrop {
    fn drop(&mut self) {
        if let Some(controller) = self.0.take() {
            controller.abort();
        }
    }
}

struct Github {
    token: String,
    requests: Cell<u32>,
}

impl Github {
    fn new(token: String) -> Self {
        Self {
            token,
            requests: Cell::new(0),
        }
    }

    async fn call(&self, method: Method, path: &str, body: Option<Value>) -> Result<(u16, Value)> {
        if self.requests.get() >= SUBREQUESTS_MAX {
            return Err(failure("GitHub subrequest limit exceeded"));
        }
        self.requests.set(self.requests.get() + 1);
        if !path.starts_with('/') || path.contains(['\r', '\n']) {
            return Err(failure("invalid API path"));
        }
        let headers = Headers::new();
        headers.set("Authorization", &format!("Bearer {}", self.token))?;
        headers.set("Accept", "application/vnd.github+json")?;
        headers.set("User-Agent", APP)?;
        headers.set("X-GitHub-Api-Version", "2022-11-28")?;
        headers.set("Content-Type", "application/json")?;
        let mut init = RequestInit::new();
        init.with_method(method)
            .with_headers(headers)
            .with_redirect(worker::RequestRedirect::Manual);
        if let Some(body) = body {
            init.with_body(Some(JsValue::from_str(&body.to_string())));
        }
        let request = Request::new_with_init(&format!("https://api.github.com{path}"), &init)?;
        let controller = AbortController::default();
        let signal = controller.signal();
        let _abort_on_drop = AbortOnDrop(Some(controller));
        bounded(
            async {
                let mut response = Fetch::Request(request).send_with_signal(&signal).await?;
                let status = response.status_code();
                let bytes = match response.body() {
                    worker::ResponseBody::Empty => Vec::new(),
                    worker::ResponseBody::Body(bytes) if bytes.len() <= RESPONSE_BYTES_MAX => {
                        bytes.clone()
                    }
                    worker::ResponseBody::Body(_) => {
                        return Err(failure("response body limit exceeded"));
                    }
                    worker::ResponseBody::Stream(_) => {
                        read_stream(response.stream()?, RESPONSE_BYTES_MAX).await?
                    }
                };
                // Error bodies can contain sensitive context; never forward or log them.
                let value = if bytes.is_empty() {
                    Value::Null
                } else {
                    serde_json::from_slice(&bytes)
                        .map_err(|_| failure("GitHub returned invalid JSON"))?
                };
                Ok((status, value))
            },
            5000,
        )
        .await
    }

    async fn get(&self, path: &str) -> Result<Value> {
        let (status, value) = self.call(Method::Get, path, None).await?;
        if status != 200 {
            return Err(failure("GitHub read failed"));
        }
        Ok(value)
    }

    async fn post(&self, path: &str, body: Value) -> Result<Value> {
        let (status, value) = self.call(Method::Post, path, Some(body)).await?;
        if !matches!(status, 200 | 201 | 204) {
            return Err(failure("GitHub write failed"));
        }
        Ok(value)
    }

    async fn installation(env: &Env) -> Result<Self> {
        let private_key = env.secret("APP_PRIVATE_KEY")?.to_string();
        let jwt = JsFuture::from(app_jwt(&private_key))
            .await
            .map_err(|_| failure("App JWT signing failed"))?
            .as_string()
            .ok_or_else(|| failure("App JWT signing failed"))?;
        let app = Self::new(jwt);
        let value = app
            .post("/app/installations/153964219/access_tokens", json!({}))
            .await?;
        let token = string(&value, "token")?.to_owned();
        let github = Self::new(token);
        github.requests.set(app.requests.get());
        Ok(github)
    }

    async fn repository(&self, repository: &str) -> Result<Option<Value>> {
        let (status, info) = self
            .call(Method::Get, &format!("/repos/{repository}"), None)
            .await?;
        if status == 404 {
            return Ok(None);
        }
        if status != 200 {
            return Err(failure("repository read failed"));
        }
        let allowlisted = info
            .get("topics")
            .and_then(Value::as_array)
            .is_some_and(|topics| {
                topics
                    .iter()
                    .any(|topic| topic.as_str() == Some(TEST_TOPIC))
            });
        if !allowlisted || info.get("full_name").and_then(Value::as_str) != Some(repository) {
            return Ok(None);
        }
        Ok(Some(info))
    }

    async fn index(&self, repository: &str, sha: &str) -> Result<Vec<Golem>> {
        let (status, entries) = self
            .call(
                Method::Get,
                &format!("/repos/{repository}/contents/.golems?ref={sha}"),
                None,
            )
            .await?;
        if status == 404 {
            return Ok(Vec::new());
        }
        if status != 200 {
            return Err(failure("golem directory read failed"));
        }
        let entries = entries
            .as_array()
            .ok_or_else(|| failure("invalid golem directory"))?;
        if entries.len() > 100 {
            return Err(failure("golem directory entry limit exceeded"));
        }
        // The repository interface names lowercase `.md` files, not OS extensions.
        #[allow(clippy::case_sensitive_file_extension_comparisons)]
        let candidates: Vec<_> = entries
            .iter()
            .filter(|entry| {
                entry
                    .get("name")
                    .and_then(Value::as_str)
                    .is_some_and(|name| name.ends_with(".md") && name != "_golems.md")
            })
            .collect();
        if candidates.len() > GOLEMS_MAX {
            return Err(failure("golem count limit exceeded"));
        }
        let mut golems = Vec::with_capacity(candidates.len());
        for entry in candidates {
            let filename = string(entry, "name")?;
            let name = filename
                .strip_suffix(".md")
                .ok_or_else(|| failure("invalid golem filename"))?;
            // Unsafe names are reported without interpolating them into URLs.
            if !filename
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || b"-_.".contains(&byte))
                || entry.get("type").and_then(Value::as_str) != Some("file")
                || entry
                    .get("size")
                    .and_then(Value::as_u64)
                    .is_none_or(|size| size > FILE_BYTES_MAX as u64)
            {
                let mut golem = parse_golem(name, "");
                golem
                    .errors
                    .push("not a regular bounded Markdown file".into());
                golems.push(golem);
                continue;
            }
            golems.push(
                self.golem(repository, sha, name)
                    .await?
                    .ok_or_else(|| failure("golem disappeared from pinned snapshot"))?,
            );
        }
        golems.sort_by(|left, right| left.name.cmp(&right.name));
        Ok(golems)
    }

    async fn golem(&self, repository: &str, sha: &str, name: &str) -> Result<Option<Golem>> {
        let (status, file) = self
            .call(
                Method::Get,
                &format!("/repos/{repository}/contents/.golems/{name}.md?ref={sha}"),
                None,
            )
            .await?;
        if status == 404 {
            return Ok(None);
        }
        if status != 200
            || file.get("type").and_then(Value::as_str) != Some("file")
            || file.get("encoding").and_then(Value::as_str) != Some("base64")
        {
            return Err(failure("unsupported golem content encoding"));
        }
        let encoded = string(&file, "content")?;
        if encoded.len() > FILE_BYTES_MAX * 2 {
            return Err(failure("golem content limit exceeded"));
        }
        let compact: String = encoded
            .chars()
            .filter(|c| !c.is_ascii_whitespace())
            .collect();
        let document = STANDARD
            .decode(compact)
            .ok()
            .and_then(|bytes| String::from_utf8(bytes).ok());
        let golem = if let Some(document) = document {
            parse_golem(name, &document)
        } else {
            let mut golem = parse_golem(name, "");
            golem.errors.push("invalid UTF-8 or base64".into());
            golem
        };
        Ok(Some(golem))
    }

    async fn next_golem(
        &self,
        repository: &str,
        sha: &str,
        after: Option<&str>,
    ) -> Result<Option<Golem>> {
        let (status, entries) = self
            .call(
                Method::Get,
                &format!("/repos/{repository}/contents/.golems?ref={sha}"),
                None,
            )
            .await?;
        if status == 404 {
            return Ok(None);
        }
        if status != 200 {
            return Err(failure("golem directory read failed"));
        }
        let entries = entries
            .as_array()
            .filter(|entries| entries.len() <= 100)
            .ok_or_else(|| failure("golem directory limit exceeded"))?;
        let mut names: Vec<_> = entries
            .iter()
            .filter_map(|entry| {
                entry
                    .get("name")
                    .and_then(Value::as_str)
                    .and_then(|name| name.strip_suffix(".md"))
            })
            .filter(|name| *name != "_golems")
            .collect();
        if names.len() > GOLEMS_MAX {
            return Err(failure("golem count limit exceeded"));
        }
        names.sort_unstable();
        let Some(name) = names
            .into_iter()
            .find(|name| crate::valid_golem_name(name) && after.is_none_or(|after| *name > after))
        else {
            return Ok(None);
        };
        self.golem(repository, sha, name).await
    }

    async fn head(&self, repository: &str, info: &Value) -> Result<String> {
        let branch = string(info, "default_branch")?;
        let mut url = worker::Url::parse("https://api.github.com/")?;
        url.path_segments_mut()
            .map_err(|()| failure("invalid URL base"))?
            .extend([
                "repos",
                ORGANIZATION,
                repository
                    .split_once('/')
                    .ok_or_else(|| failure("invalid repository"))?
                    .1,
                "commits",
                branch,
            ]);
        let commit = self.get(url.path()).await?;
        let sha = string(&commit, "sha")?;
        if !valid_sha(sha) {
            return Err(failure("invalid head SHA"));
        }
        Ok(sha.into())
    }

    async fn claim(&self, delivery: &str, digest: &str) -> Result<bool> {
        let head = self
            .get(&format!("/repos/{OPS}/git/ref/heads/trunk"))
            .await?;
        let sha = head
            .pointer("/object/sha")
            .and_then(Value::as_str)
            .filter(|sha| valid_sha(sha))
            .ok_or_else(|| failure("invalid ops head"))?;
        let tag_name = format!("golem-delivery/{delivery}");
        let message = format!("isp-golem-xiv delivery v1 sha256:{digest}");
        let tag = self
            .post(
                &format!("/repos/{OPS}/git/tags"),
                json!({"tag":tag_name,"message":message,"object":sha,"type":"commit"}),
            )
            .await?;
        let tag_sha = string(&tag, "sha")?;
        if !valid_sha(tag_sha) {
            return Err(failure("invalid claim SHA"));
        }
        let reference = format!("refs/tags/{tag_name}");
        let (status, _) = self
            .call(
                Method::Post,
                &format!("/repos/{OPS}/git/refs"),
                Some(json!({"ref":reference,"sha":tag_sha})),
            )
            .await?;
        if status == 201 {
            return Ok(true);
        }
        if status != 422 {
            return Err(failure("delivery claim failed; no retry"));
        }
        let existing = self
            .get(&format!("/repos/{OPS}/git/ref/tags/{tag_name}"))
            .await?;
        let existing_sha = existing
            .pointer("/object/sha")
            .and_then(Value::as_str)
            .filter(|sha| valid_sha(sha))
            .ok_or_else(|| failure("invalid existing delivery claim"))?;
        if existing.get("ref").and_then(Value::as_str) != Some(reference.as_str())
            || existing.pointer("/object/type").and_then(Value::as_str) != Some("tag")
        {
            return Err(failure("delivery claim mismatch"));
        }
        let existing_tag = self
            .get(&format!("/repos/{OPS}/git/tags/{existing_sha}"))
            .await?;
        if existing_tag.get("tag").and_then(Value::as_str) != Some(tag_name.as_str())
            || existing_tag
                .get("message")
                .and_then(Value::as_str)
                .map(str::trim_end)
                != Some(message.as_str())
        {
            return Err(failure("delivery claim fingerprint mismatch"));
        }
        Ok(false)
    }

    async fn member(&self, login: &str) -> Result<bool> {
        let (status, membership) = self
            .call(
                Method::Get,
                &format!("/orgs/{ORGANIZATION}/memberships/{login}"),
                None,
            )
            .await?;
        if status == 404 {
            return Ok(false);
        }
        if status != 200 {
            return Err(failure("membership check failed"));
        }
        Ok(membership.get("state").and_then(Value::as_str) == Some("active"))
    }

    async fn reaction(&self, repository: &str, comment: u64, content: &str) -> Result<()> {
        self.post(
            &format!("/repos/{repository}/issues/comments/{comment}/reactions"),
            json!({"content":content}),
        )
        .await?;
        Ok(())
    }

    async fn refuse(&self, delivery: &Delivery, reason: &str, golems: &[Golem]) -> Result<()> {
        self.post(
            &format!(
                "/repos/{}/issues/{}/comments",
                delivery.repository.full_name, delivery.issue.number
            ),
            json!({"body":refusal(reason, golems)}),
        )
        .await?;
        Ok(())
    }
}

fn string<'a>(value: &'a Value, key: &str) -> Result<&'a str> {
    value
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| failure("missing GitHub response field"))
}

fn valid_sha(sha: &str) -> bool {
    sha.len() == 40 && sha.bytes().all(|byte| byte.is_ascii_hexdigit())
}

async fn process_legacy(
    github: &Github,
    delivery: Delivery,
    delivery_id: String,
    digest: String,
) -> Result<()> {
    let repository = &delivery.repository.full_name;
    let Some(info) = github.repository(repository).await? else {
        return Ok(());
    };
    if info.get("id").and_then(Value::as_u64) != Some(delivery.repository.id) {
        return Err(failure("repository identity mismatch"));
    }
    if !github.claim(&delivery_id, &digest).await? {
        return Ok(());
    }
    github
        .reaction(repository, delivery.comment.id, "eyes")
        .await?;
    if !github.member(&delivery.sender.login).await? {
        return github.refuse(&delivery, "not-member", &[]).await;
    }
    let head = github.head(repository, &info).await?;
    let golems = github.index(repository, &head).await?;
    let mention =
        parse_mention(&delivery.comment.body).ok_or_else(|| failure("invalid mention"))?;
    let Some(golem) = golems
        .iter()
        .find(|golem| golem.name == mention.golem && golem.errors.is_empty())
    else {
        return github.refuse(&delivery, "unknown-golem", &golems).await;
    };
    // Slice 1 supports the directly provisioned Anthropic/Claude seat only.
    let provider = golem.frontmatter.provider.as_deref().unwrap_or("anthropic");
    let harness = golem.frontmatter.harness.as_deref().unwrap_or("claude");
    if provider != "anthropic" || harness != "claude" {
        return github.refuse(&delivery, "no-seat", &golems).await;
    }
    let secret = seat_secret(&delivery.sender.login, provider)
        .ok_or_else(|| failure("invalid seat identity"))?;
    // This endpoint returns metadata only, never the subscription credential.
    let (status, _) = github
        .call(
            Method::Get,
            &format!("/repos/{OPS}/actions/secrets/{secret}"),
            None,
        )
        .await?;
    if status == 404 {
        return github.refuse(&delivery, "no-seat", &golems).await;
    }
    if status != 200 {
        return Err(failure("seat metadata lookup failed"));
    }
    let timeout = golem
        .timeout_seconds()
        .ok_or_else(|| failure("invalid timeout"))?;
    let model = golem.frontmatter.model.as_deref();
    let summary = format!(
        "outcome: queued\nseat: {}/{provider}\nharness: {harness}\nmodel: {}\nbudget: {timeout}s\nlog: https://github.com/{OPS}/actions\nStop: @isp-golem-xiv stop (available in Slice 2).",
        delivery.sender.login,
        model.unwrap_or("default")
    );
    let check = github.post(&format!("/repos/{repository}/check-runs"), json!({"name":format!("golem/{}",golem.name),"head_sha":head,"status":"queued","external_id":delivery_id,"output":{"title":"Golem queued","summary":summary}})).await?;
    let check_id = check
        .get("id")
        .and_then(Value::as_u64)
        .filter(|id| *id > 0)
        .ok_or_else(|| failure("missing check run ID"))?;
    let request = DispatchV1 {
        version: 1,
        delivery_id: &delivery_id,
        repository_id: delivery.repository.id,
        repository,
        head_sha: &head,
        golem: &golem.name,
        issue_number: delivery.issue.number,
        comment_id: delivery.comment.id,
        actor_id: delivery.sender.id,
        actor_login: &delivery.sender.login,
        seat_login: &delivery.sender.login,
        provider,
        harness,
        model,
        timeout_seconds: timeout,
        instructions: mention.instructions,
        check_run_id: check_id,
    };
    let serialized =
        serde_json::to_string(&request).map_err(|_| failure("dispatch serialization failed"))?;
    let (status, _) = github.call(Method::Post, &format!("/repos/{OPS}/actions/workflows/runner.yml/dispatches"), Some(json!({"ref":"trunk","inputs":{"repo":repository,"golem":golem.name,"request":serialized}}))).await?;
    if status != 204 {
        // A non-success or transport timeout may still have created a workflow run.
        // Never retry; preserve the durable claim for operator investigation.
        return Err(failure(
            "workflow dispatch failed or ambiguous; claim retained",
        ));
    }
    github
        .reaction(repository, delivery.comment.id, "rocket")
        .await?;
    Ok(())
}

async fn protocol_v2(github: &Github) -> Result<bool> {
    let (status, variable) = github
        .call(
            Method::Get,
            &format!("/repos/{OPS}/actions/variables/GOLEM_PROTOCOL_VERSION"),
            None,
        )
        .await?;
    match status {
        404 => Ok(false),
        200 => Ok(variable.get("value").and_then(Value::as_str) == Some("2")),
        _ => Err(failure("protocol metadata unavailable")),
    }
}

async fn coordinator(env: &Env, action: &str, mut body: Value) -> Result<Response> {
    body.as_object_mut()
        .ok_or_else(|| failure("invalid coordinator body"))?
        .insert("action".into(), json!(action));
    let namespace = env.durable_object("COORDINATOR")?;
    let stub = namespace.id_from_name(ORGANIZATION)?.get_stub()?;
    let mut init = RequestInit::new();
    init.with_method(Method::Post)
        .with_body(Some(JsValue::from_str(&body.to_string())));
    stub.fetch_with_request(Request::new_with_init(
        "https://coordinator.internal/",
        &init,
    )?)
    .await
}

async fn coordinate_budgeted(
    env: &Env,
    github: &Github,
    action: &str,
    body: Value,
) -> Result<Response> {
    if github.requests.get() >= SUBREQUESTS_MAX {
        return Err(failure("subrequest budget exceeded"));
    }
    github.requests.set(github.requests.get() + 1);
    coordinator(env, action, body).await
}

async fn internal_queue(request: &mut Request, env: &Env) -> Result<Response> {
    let signature = request
        .headers()
        .get("X-Golem-Signature-256")?
        .unwrap_or_default();
    if signature.len() != 71 {
        return Response::error("Invalid signature", 401);
    }
    let Ok(bytes) = bounded(read_stream(request.stream()?, 16_384), 5000).await else {
        return Response::error("Queue body too large", 413);
    };
    let pem = env.secret("APP_PRIVATE_KEY")?.to_string();
    let now = runtime_timestamp(worker::js_sys::Date::now())?;
    if !verify_queue_signature(&pem, &signature, &bytes, now) {
        return Response::error("Invalid queue authorization", 401);
    }
    let value: Value = serde_json::from_slice(&bytes).map_err(|_| failure("invalid queue body"))?;
    let action = string(&value, "action")?.to_owned();
    coordinator(env, &action, value).await
}

// JS epoch milliseconds are integral and safely representable throughout the supported range.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn runtime_timestamp(milliseconds: f64) -> Result<u64> {
    if !milliseconds.is_finite() || !(0.0..=8_640_000_000_000_000.0).contains(&milliseconds) {
        return Err(failure("invalid runtime timestamp"));
    }
    Ok(milliseconds as u64)
}

/// Names only: secret values are never readable through the API.
async fn seats_of(github: &Github, login: &str) -> Result<Vec<&'static str>> {
    let mut seats = Vec::new();
    for provider in crate::PROVIDERS {
        let secret = seat_secret(login, provider).ok_or_else(|| failure("invalid seat login"))?;
        let (status, _) = github
            .call(
                Method::Get,
                &format!("/repos/{OPS}/actions/secrets/{secret}"),
                None,
            )
            .await?;
        match status {
            200 => seats.push(provider),
            404 => {}
            _ => return Err(failure("seat metadata unavailable")),
        }
    }
    Ok(seats)
}

async fn preferred_seat(github: &Github, login: &str) -> Result<Option<String>> {
    if !crate::valid_login(login) {
        return Err(failure("invalid seat login"));
    }
    let name = format!(
        "SEAT_PREFERRED_{}",
        login.replace('-', "_").to_ascii_uppercase()
    );
    let (status, value) = github
        .call(
            Method::Get,
            &format!("/repos/{OPS}/actions/variables/{name}"),
            None,
        )
        .await?;
    match status {
        200 => Ok(value
            .get("value")
            .and_then(Value::as_str)
            .filter(|v| crate::PROVIDERS.contains(v))
            .map(str::to_owned)),
        404 => Ok(None),
        _ => Err(failure("preferred seat unavailable")),
    }
}

/// Default-branch upkeep on push, reusing the delivery's head and index so the event
/// path keeps its subrequest budget: onboard without `.golems/`, else keep the dashboard current.
async fn maintain_repository(
    github: &Github,
    repository: &str,
    info: &Value,
    head: &str,
    golems: &[Golem],
) -> Result<()> {
    if golems.is_empty() {
        // An empty index means a missing directory or one without golems; only the former onboards.
        let (status, _) = github
            .call(
                Method::Get,
                &format!("/repos/{repository}/contents/.golems?ref={head}"),
                None,
            )
            .await?;
        match status {
            404 => return onboard(github, repository, info, head).await,
            200 => {}
            _ => return Err(failure("golem directory lookup failed")),
        }
    }
    update_dashboard(github, repository, info, golems).await
}

/// Opens the onboarding pull request once per repository; an ops tag is the durable claim.
async fn onboard(github: &Github, repository: &str, info: &Value, head: &str) -> Result<()> {
    let id = info
        .get("id")
        .and_then(Value::as_u64)
        .ok_or_else(|| failure("missing repository id"))?;
    let trunk = github
        .get(&format!("/repos/{OPS}/git/ref/heads/trunk"))
        .await?;
    let anchor = trunk
        .pointer("/object/sha")
        .and_then(Value::as_str)
        .filter(|sha| valid_sha(sha))
        .ok_or_else(|| failure("invalid ops trunk"))?;
    let (status, _) = github
        .call(
            Method::Post,
            &format!("/repos/{OPS}/git/refs"),
            Some(json!({"ref":format!("refs/tags/golem-onboarding/{id}"),"sha":anchor})),
        )
        .await?;
    if status == 422 {
        return Ok(()); // Already onboarded once; closing the pull request is respected.
    }
    if status != 201 {
        return Err(failure("onboarding claim failed"));
    }
    let (status, _) = github
        .call(
            Method::Post,
            &format!("/repos/{repository}/git/refs"),
            Some(json!({"ref":format!("refs/heads/{}", crate::ONBOARDING_BRANCH),"sha":head})),
        )
        .await?;
    if status != 201 {
        return Err(failure("onboarding branch creation failed"));
    }
    for (path, content) in crate::onboarding_files() {
        let (status, _) = github
            .call(
                Method::Put,
                &format!("/repos/{repository}/contents/{path}"),
                Some(json!({"message":crate::ONBOARDING_TITLE,"content":STANDARD.encode(content),"branch":crate::ONBOARDING_BRANCH})),
            )
            .await?;
        if status != 201 {
            return Err(failure("onboarding file write failed"));
        }
    }
    let root = github
        .get(&format!("/repos/{repository}/contents?ref={head}"))
        .await?;
    let names: Vec<&str> = root
        .as_array()
        .map(|entries| {
            entries
                .iter()
                .filter_map(|entry| entry.get("name").and_then(Value::as_str))
                .collect()
        })
        .unwrap_or_default();
    let toolchain = ["mise.toml", ".mise.toml", ".tool-versions"]
        .into_iter()
        .find(|file| names.contains(file));
    github
        .post(
            &format!("/repos/{repository}/pulls"),
            json!({"title":crate::ONBOARDING_TITLE,"head":crate::ONBOARDING_BRANCH,"base":string(info, "default_branch")?,"body":crate::onboarding_body(toolchain)}),
        )
        .await?;
    Ok(())
}

/// Seat holders from ops secret names and current organization members; never values.
async fn live_seat_holders(github: &Github) -> Result<crate::SeatHolders> {
    let secrets = github
        .get(&format!("/repos/{OPS}/actions/secrets?per_page=100"))
        .await?;
    if secrets
        .get("total_count")
        .and_then(Value::as_u64)
        .is_none_or(|count| count > 100)
    {
        return Err(failure("seat secret listing incomplete"));
    }
    let names: Vec<&str> = secrets
        .get("secrets")
        .and_then(Value::as_array)
        .ok_or_else(|| failure("invalid seat secret listing"))?
        .iter()
        .filter_map(|secret| secret.get("name").and_then(Value::as_str))
        .collect();
    let mut members = Vec::new();
    for page in 1..=3 {
        let listed = github
            .get(&format!(
                "/orgs/{ORGANIZATION}/members?per_page=100&page={page}"
            ))
            .await?;
        let listed = listed
            .as_array()
            .ok_or_else(|| failure("invalid member listing"))?;
        members.extend(
            listed
                .iter()
                .filter_map(|member| member.get("login").and_then(Value::as_str))
                .map(str::to_owned),
        );
        if listed.len() < 100 {
            let members: Vec<&str> = members.iter().map(String::as_str).collect();
            return Ok(crate::seat_holders(&names, &members));
        }
    }
    Err(failure("member listing exceeds bound"))
}

/// Creates and pins the dashboard issue, or updates it when its body changed.
async fn update_dashboard(
    github: &Github,
    repository: &str,
    info: &Value,
    golems: &[Golem],
) -> Result<()> {
    let holders = if info.get("private").and_then(Value::as_bool) == Some(true) {
        Some(live_seat_holders(github).await?)
    } else {
        None
    };
    let body = crate::dashboard_body(golems, holders.as_ref());
    let issues = github
        .get(&format!(
            "/repos/{repository}/issues?state=open&per_page=100"
        ))
        .await?;
    let existing = issues
        .as_array()
        .ok_or_else(|| failure("invalid issue listing"))?
        .iter()
        .find(|issue| {
            issue.pointer("/user/login").and_then(Value::as_str) == Some(crate::BOT)
                && issue.get("title").and_then(Value::as_str) == Some(crate::DASHBOARD_TITLE)
                && issue.get("pull_request").is_none()
                && issue
                    .get("body")
                    .and_then(Value::as_str)
                    .is_some_and(|text| text.starts_with(crate::DASHBOARD_MARKER))
        });
    if let Some(issue) = existing {
        if issue.get("body").and_then(Value::as_str) == Some(body.as_str()) {
            return Ok(());
        }
        let number = issue
            .get("number")
            .and_then(Value::as_u64)
            .ok_or_else(|| failure("invalid dashboard number"))?;
        let (status, _) = github
            .call(
                Method::Patch,
                &format!("/repos/{repository}/issues/{number}"),
                Some(json!({"body":body})),
            )
            .await?;
        return if status == 200 {
            Ok(())
        } else {
            Err(failure("dashboard update failed"))
        };
    }
    let created = github
        .post(
            &format!("/repos/{repository}/issues"),
            json!({"title":crate::DASHBOARD_TITLE,"body":body}),
        )
        .await?;
    let node = string(&created, "node_id")?;
    let pinned = github
        .post(
            "/graphql",
            json!({"query":"mutation($id:ID!){pinIssue(input:{issueId:$id}){issue{id}}}","variables":{"id":node}}),
        )
        .await?;
    if pinned.get("errors").is_some() {
        return Err(failure("dashboard pin failed"));
    }
    Ok(())
}

/// Validates `.golems/` at the pull request head when the pull request touches it.
async fn validate_pull(github: &Github, repository: &str, number: u64) -> Result<()> {
    let pull = github
        .get(&format!("/repos/{repository}/pulls/{number}"))
        .await?;
    let head = pull
        .pointer("/head/sha")
        .and_then(Value::as_str)
        .filter(|sha| valid_sha(sha))
        .ok_or_else(|| failure("invalid pull request head"))?
        .to_owned();
    let files = github
        .get(&format!(
            "/repos/{repository}/pulls/{number}/files?per_page=100"
        ))
        .await?;
    let files = files
        .as_array()
        .ok_or_else(|| failure("invalid pull request files"))?;
    // A full page may hide further files; validate rather than miss a change.
    let touched = files.len() >= 100
        || files.iter().any(|file| {
            ["filename", "previous_filename"].iter().any(|key| {
                file.get(*key)
                    .and_then(Value::as_str)
                    .is_some_and(|name| name.starts_with(".golems/"))
            })
        });
    if !touched {
        return Ok(());
    }
    let golems = github.index(repository, &head).await?;
    let (ok, summary) = crate::validation_summary(&golems);
    github
        .post(
            &format!("/repos/{repository}/check-runs"),
            json!({"name":crate::VALIDATION_CHECK,"head_sha":head,"status":"completed","conclusion":if ok {"success"} else {"failure"},"output":{"title":if ok {"Golems valid"} else {"Invalid golems"},"summary":summary}}),
        )
        .await?;
    Ok(())
}

async fn has_seat(github: &Github, login: &str) -> Result<bool> {
    let secret = seat_secret(login, "anthropic").ok_or_else(|| failure("invalid seat login"))?;
    let (status, _) = github
        .call(
            Method::Get,
            &format!("/repos/{OPS}/actions/secrets/{secret}"),
            None,
        )
        .await?;
    match status {
        200 => Ok(true),
        404 => Ok(false),
        _ => Err(failure("seat metadata unavailable")),
    }
}

async fn unattended_seat(
    github: &Github,
    golem: &Golem,
    actor: Option<&crate::User>,
) -> Result<Option<String>> {
    let human = actor.is_some_and(|actor| actor.kind == "User");
    let member = if human {
        github
            .member(actor.map_or("", |actor| actor.login.as_str()))
            .await?
    } else {
        false
    };
    if human && !member {
        return Ok(None);
    }
    let actor_seat = if member {
        has_seat(github, actor.map_or("", |actor| actor.login.as_str())).await?
    } else {
        false
    };
    if actor_seat {
        return Ok(actor.map(|actor| actor.login.clone()));
    }
    let Some(owner) = golem.frontmatter.owner.as_deref() else {
        return Ok(None);
    };
    let owner_member = github.member(owner).await?;
    let owner_seat = if owner_member {
        has_seat(github, owner).await?
    } else {
        false
    };
    Ok(event_seat(
        actor.map_or("", |actor| actor.login.as_str()),
        if human {
            crate::EventActor::MemberWithoutSeat
        } else {
            crate::EventActor::Bot
        },
        Some(owner),
        owner_member,
        owner_seat,
    )
    .map(str::to_owned))
}

fn supported(golem: &Golem) -> bool {
    golem.errors.is_empty()
        && golem.frontmatter.provider.as_deref().unwrap_or("anthropic") == "anthropic"
        && golem.frontmatter.harness.as_deref().unwrap_or("claude") == "claude"
}

async fn prepare_request(github: &Github, golem: &Golem, mut request: Value) -> Result<Value> {
    let repository = string(&request, "repository")?;
    let timeout = golem
        .timeout_seconds()
        .ok_or_else(|| failure("invalid timeout"))?;
    let summary = format!(
        "outcome: queued\nseat: {}/anthropic\nharness: claude\nmodel: {}\nbudget: {timeout}s\nlog: https://github.com/{OPS}/actions\nStop: @isp-golem-xiv stop",
        string(&request, "seat_login")?,
        golem.frontmatter.model.as_deref().unwrap_or("default")
    );
    let check = github.post(&format!("/repos/{repository}/check-runs"), json!({"name":format!("golem/{}",golem.name),"head_sha":request.get("head_sha"),"status":"queued","external_id":request.get("delivery_id"),"output":{"title":"Golem queued","summary":summary}})).await?;
    let fields = request
        .as_object_mut()
        .ok_or_else(|| failure("invalid queue request"))?;
    fields.insert(
        "check_run_id".into(),
        check
            .get("id")
            .filter(|id| id.as_u64().is_some_and(|id| id > 0))
            .cloned()
            .ok_or_else(|| failure("missing check ID"))?,
    );
    for (key, value) in [
        ("version", json!(2)),
        ("golem", json!(golem.name)),
        ("branch", json!(golem.branch())),
        ("verify", json!(golem.frontmatter.verify)),
        ("owner", json!(golem.frontmatter.owner)),
        ("provider", json!("anthropic")),
        ("harness", json!("claude")),
        ("model", json!(golem.frontmatter.model)),
        ("timeout_seconds", json!(timeout)),
    ] {
        fields.insert(key.into(), value);
    }
    Ok(request)
}

async fn admit_request(env: &Env, github: &Github, request: &Value) -> Result<()> {
    // Coordinator adds generation, durably reserves both resources, dispatches and reacts.
    // Retrying identical admission is safe; coordinator alone decides dispatch replay.
    let response = coordinate_budgeted(env, github, "enqueue", json!({"request":request})).await?;
    if !(200..300).contains(&response.status_code()) {
        return Err(failure("queue admission failed; claim retained"));
    }
    Ok(())
}

async fn process_mention(
    env: &Env,
    github: &Github,
    delivery: &EventDelivery,
    mention: &Delivery,
    delivery_id: &str,
    repository: &str,
    info: &Value,
) -> Result<()> {
    github
        .reaction(repository, mention.comment.id, "eyes")
        .await?;
    if !github.member(&mention.sender.login).await? {
        return github.refuse(mention, "not-member", &[]).await;
    }
    let command = parse_mention(&mention.comment.body).ok_or_else(|| failure("invalid mention"))?;
    if command.golem == "stop" {
        let response = coordinate_budgeted(
            env,
            github,
            "stop",
            json!({"repository":repository,"issue_number":mention.issue.number}),
        )
        .await?;
        if !(200..300).contains(&response.status_code()) {
            return Err(failure("stop failed"));
        }
        return Ok(());
    }
    if matches!(command.golem, "" | "help" | "seat") {
        let login = &mention.sender.login;
        let seats = seats_of(github, login).await?;
        let preferred = preferred_seat(github, login).await?;
        let body = if command.golem == "seat" {
            crate::seat_reply(login, &seats, preferred.as_deref())
        } else {
            let head = github.head(repository, info).await?;
            let golems = github.index(repository, &head).await?;
            crate::help_reply(login, &golems, &seats, preferred.as_deref())
        };
        github
            .post(
                &format!(
                    "/repos/{repository}/issues/{}/comments",
                    mention.issue.number
                ),
                json!({"body":body}),
            )
            .await?;
        return Ok(());
    }
    let head = github.head(repository, info).await?;
    let golems = github.index(repository, &head).await?;
    let Some(golem) = golems
        .iter()
        .find(|golem| golem.name == command.golem && golem.errors.is_empty())
    else {
        return github.refuse(mention, "unknown-golem", &golems).await;
    };
    if !supported(golem) || !has_seat(github, &mention.sender.login).await? {
        return github.refuse(mention, "no-seat", &golems).await;
    }
    let prepared = prepare_request(github, golem, json!({"delivery_id":delivery_id,"repository_id":delivery.repository.id,"repository":repository,"head_sha":head,"trigger":"mention","issue_number":mention.issue.number,"comment_id":mention.comment.id,"actor_id":mention.sender.id,"actor_login":mention.sender.login,"seat_login":mention.sender.login,"instructions":command.instructions})).await?;
    let mut work = EventWork {
        version: 2,
        head,
        delivery_id: delivery_id.into(),
        event: "issue_comment".into(),
        delivery: delivery.clone(),
        golems: vec![golem.name.clone()],
        next: 0,
        prepared: Some(prepared),
    };
    let path = event_path(&work.delivery_id);
    persist_event_source(github, &work).await?;
    update_event_index(github, &work.delivery_id, true).await?;
    let sha = save_event_work(github, &path, &work, None).await?;
    admit_request(
        env,
        github,
        work.prepared
            .as_ref()
            .ok_or_else(|| failure("prepared mention missing"))?,
    )
    .await?;
    work.prepared = None;
    work.next += 1;
    retire_event(github, &work.delivery_id, Some(&sha)).await?;
    Ok(())
}

async fn process(
    env: Env,
    event: String,
    delivery: EventDelivery,
    mention: Option<Delivery>,
    delivery_id: String,
    digest: String,
) -> Result<()> {
    let github = Github::installation(&env).await?;
    if !protocol_v2(&github).await? {
        return if let Some(mention) = mention {
            process_legacy(&github, mention, delivery_id, digest).await
        } else {
            Ok(())
        };
    }
    let repository = &delivery.repository.full_name;
    let Some(info) = github.repository(repository).await? else {
        return Ok(());
    };
    if info.get("id").and_then(Value::as_u64) != Some(delivery.repository.id) {
        return Err(failure("repository identity mismatch"));
    }
    // Contents cannot create its branch. Bootstrap before consuming the claim.
    let (status, _) = github
        .call(
            Method::Get,
            &format!("/repos/{OPS}/git/ref/heads/golem-state"),
            None,
        )
        .await?;
    if status == 404 {
        let response = coordinate_budgeted(&env, &github, "tick", json!({})).await?;
        if !(200..300).contains(&response.status_code()) {
            return Err(failure("event journal initialization failed"));
        }
    } else if status != 200 {
        return Err(failure("event journal branch lookup failed"));
    }
    if !github.claim(&delivery_id, &digest).await? {
        return Ok(());
    }
    if let Some(mention) = mention {
        return process_mention(
            &env,
            &github,
            &delivery,
            &mention,
            &delivery_id,
            repository,
            &info,
        )
        .await;
    }
    if event == "pull_request"
        && matches!(
            delivery.action.as_str(),
            "opened" | "synchronize" | "reopened"
        )
        && let Some(pull) = &delivery.pull_request
        && validate_pull(&github, repository, pull.number)
            .await
            .is_err()
    {
        // Validation is advisory to triggers; a failure must not drop matching golems.
        worker::console_error!("golem validation failed; delivery={delivery_id}");
    }
    let head = github.head(repository, &info).await?;
    let golems = github.index(repository, &head).await?;
    if event == "push"
        && maintain_repository(&github, repository, &info, &head, &golems)
            .await
            .is_err()
    {
        // Onboarding and dashboard upkeep never block event golems.
        worker::console_error!("repository upkeep failed; delivery={delivery_id}");
    }
    let selected: Vec<_> = golems
        .into_iter()
        .filter(|golem| {
            supported(golem)
                && trigger_matches(&golem.frontmatter.triggers, &event, &delivery.action)
        })
        .map(|golem| golem.name)
        .collect();
    if selected.is_empty() {
        return Ok(());
    }
    // A persistent continuation precedes bounded candidate processing. No matching golem
    // is discarded merely because this delivery exhausted its HTTP budget.
    let tick = coordinate_budgeted(&env, &github, "tick", json!({})).await?;
    if !(200..300).contains(&tick.status_code()) {
        return Err(failure("event journal initialization failed"));
    }
    let mut work = EventWork {
        version: 2,
        head,
        delivery_id,
        event,
        delivery,
        golems: selected,
        next: 0,
        prepared: None,
    };
    let path = event_path(&work.delivery_id);
    persist_event_source(&github, &work).await?;
    update_event_index(&github, &work.delivery_id, true).await?;
    let mut sha = save_event_work(&github, &path, &work, None).await?;
    while work.next < work.golems.len()
        && github.requests.get() <= SUBREQUESTS_MAX - if work.prepared.is_some() { 10 } else { 22 }
    {
        resume_event(&env, &github, &mut work).await?;
        sha = save_event_work(&github, &path, &work, Some(&sha)).await?;
    }
    if work.next == work.golems.len() && github.requests.get() <= SUBREQUESTS_MAX - 9 {
        retire_event(&github, &work.delivery_id, Some(&sha)).await?;
    }
    Ok(())
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct EventWork {
    version: u8,
    head: String,
    delivery_id: String,
    event: String,
    delivery: EventDelivery,
    golems: Vec<String>,
    next: usize,
    prepared: Option<Value>,
}

const EVENTS_PATH: &str = "/repos/isp-insoft-gmbh/isp-golem-xiv-ops/contents/events-v2";
const EVENT_INDEX_PATH: &str = "/repos/isp-insoft-gmbh/isp-golem-xiv-ops/contents/events-v2.json";

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct EventIndex {
    version: u8,
    ids: Vec<String>,
}

async fn event_index(github: &Github) -> Result<(EventIndex, Option<String>)> {
    let (status, file) = github
        .call(
            Method::Get,
            &format!("{EVENT_INDEX_PATH}?ref=golem-state"),
            None,
        )
        .await?;
    if status == 404 {
        return Ok((
            EventIndex {
                version: 2,
                ids: Vec::new(),
            },
            None,
        ));
    }
    if status != 200 || file.get("encoding").and_then(Value::as_str) != Some("base64") {
        return Err(failure("invalid event index"));
    }
    let sha = string(&file, "sha")?;
    let content = string(&file, "content")?;
    if !valid_sha(sha) || content.len() > 8192 {
        return Err(failure("event index bounds exceeded"));
    }
    let bytes = STANDARD
        .decode(
            content
                .chars()
                .filter(|c| !c.is_ascii_whitespace())
                .collect::<String>(),
        )
        .map_err(|_| failure("invalid event index encoding"))?;
    let index: EventIndex =
        serde_json::from_slice(&bytes).map_err(|_| failure("invalid event index"))?;
    if index.version != 2
        || index.ids.len() > 128
        || index.ids.iter().enumerate().any(|(position, id)| {
            !valid_delivery(id) || index.ids.iter().take(position).any(|prior| prior == id)
        })
    {
        return Err(failure("invalid event index entries"));
    }
    Ok((index, Some(sha.into())))
}

async fn update_event_index(github: &Github, id: &str, add: bool) -> Result<()> {
    // CAS conflict or ambiguous write: bounded reload, never overwrite another delivery.
    for _ in 0..3 {
        let (mut index, sha) = event_index(github).await?;
        if index.ids.iter().any(|prior| prior == id) == add {
            return Ok(());
        }
        if add {
            if index.ids.len() >= 128 {
                return Err(failure("event admission capacity exceeded"));
            }
            index.ids.push(id.into());
        } else {
            index.ids.retain(|prior| prior != id);
        }
        if save_event_work(github, EVENT_INDEX_PATH, &index, sha.as_deref())
            .await
            .is_ok()
        {
            return Ok(());
        }
    }
    Err(failure("event index CAS exhausted; claim retained"))
}
fn event_path(id: &str) -> String {
    format!("{EVENTS_PATH}/{id}.json")
}

async fn persist_event_source(github: &Github, work: &EventWork) -> Result<()> {
    let message =
        serde_json::to_string(work).map_err(|_| failure("event source serialization failed"))?;
    if message.len() > 131_072 {
        return Err(failure("event source limit exceeded"));
    }
    let head = github
        .get(&format!("/repos/{OPS}/git/ref/heads/trunk"))
        .await?;
    let sha = head
        .pointer("/object/sha")
        .and_then(Value::as_str)
        .filter(|sha| valid_sha(sha))
        .ok_or_else(|| failure("invalid event source base"))?;
    let tag = format!("golem-event/{}", work.delivery_id);
    let object = github
        .post(
            &format!("/repos/{OPS}/git/tags"),
            json!({"tag":tag,"message":message,"object":sha,"type":"commit"}),
        )
        .await?;
    let sha = string(&object, "sha")?;
    if !valid_sha(sha) {
        return Err(failure("invalid event source SHA"));
    }
    let (status, _) = github
        .call(
            Method::Post,
            &format!("/repos/{OPS}/git/refs"),
            Some(json!({"ref":format!("refs/tags/{tag}"),"sha":sha})),
        )
        .await?;
    if status != 201 {
        return Err(failure("event source publication failed; claim retained"));
    }
    Ok(())
}

async fn recover_event_work(github: &Github, id: &str) -> Result<EventWork> {
    let name = format!("golem-event/{id}");
    let reference = github
        .get(&format!("/repos/{OPS}/git/ref/tags/{name}"))
        .await?;
    let sha = reference
        .pointer("/object/sha")
        .and_then(Value::as_str)
        .filter(|sha| valid_sha(sha))
        .ok_or_else(|| failure("invalid event source reference"))?;
    if reference.get("ref").and_then(Value::as_str) != Some(format!("refs/tags/{name}").as_str())
        || reference.pointer("/object/type").and_then(Value::as_str) != Some("tag")
    {
        return Err(failure("invalid event source type"));
    }
    let tag = github.get(&format!("/repos/{OPS}/git/tags/{sha}")).await?;
    let message = string(&tag, "message")?;
    if tag.get("tag").and_then(Value::as_str) != Some(name.as_str()) || message.len() > 131_072 {
        return Err(failure("invalid event source identity"));
    }
    serde_json::from_str(message).map_err(|_| failure("invalid event source JSON"))
}

async fn save_event_work(
    github: &Github,
    path: &str,
    work: &impl serde::Serialize,
    sha: Option<&str>,
) -> Result<String> {
    let bytes = serde_json::to_vec(work).map_err(|_| failure("event serialization failed"))?;
    if bytes.len() > 131_072 {
        return Err(failure("event journal limit exceeded"));
    }
    let mut body = json!({"branch":"golem-state","message":"Retain event continuation","content":STANDARD.encode(bytes)});
    if let Some(sha) = sha {
        body.as_object_mut()
            .ok_or_else(|| failure("invalid event body"))?
            .insert("sha".into(), json!(sha));
    }
    let (status, file) = github.call(Method::Put, path, Some(body)).await?;
    if !matches!(status, 200 | 201) {
        return Err(failure("event journal CAS failed"));
    }
    let sha = file
        .pointer("/content/sha")
        .and_then(Value::as_str)
        .filter(|sha| valid_sha(sha))
        .ok_or_else(|| failure("invalid event journal SHA"))?;
    Ok(sha.into())
}

async fn delete_event_work(github: &Github, path: &str, sha: &str) -> Result<()> {
    let (status, _) = github
        .call(
            Method::Delete,
            path,
            Some(json!({"branch":"golem-state","message":"Retire event continuation","sha":sha})),
        )
        .await?;
    if status != 200 {
        return Err(failure("event journal retirement failed"));
    }
    Ok(())
}

async fn resume_event(env: &Env, github: &Github, work: &mut EventWork) -> Result<()> {
    if let Some(request) = &work.prepared {
        admit_request(env, github, request).await?;
        work.prepared = None;
        work.next += 1;
        return Ok(());
    }
    let Some(instructions) = crate::event_instructions(
        &work.event,
        &work.delivery.action,
        work.delivery
            .comment
            .as_ref()
            .map(|comment| comment.body.as_str()),
    ) else {
        worker::console_error!(
            "event context rejected before preparation; delivery={}",
            work.delivery_id
        );
        // Permanent input rejection, not an execution outcome. No check/request
        // exists for remaining candidates; archive the original source normally.
        work.next = work.golems.len();
        return Ok(());
    };
    let name = work
        .golems
        .get(work.next)
        .filter(|name| crate::valid_golem_name(name))
        .ok_or_else(|| failure("invalid event position"))?;
    let delivery = &work.delivery;
    let repository = &delivery.repository.full_name;
    let Some(info) = github.repository(repository).await? else {
        work.next += 1;
        return Ok(());
    };
    if info.get("id").and_then(Value::as_u64) != Some(delivery.repository.id) {
        return Err(failure("event repository identity changed"));
    }
    let head = github.head(repository, &info).await?;
    let Some(golem) = github.golem(repository, &head, name).await? else {
        work.next += 1;
        return Ok(());
    };
    if supported(&golem)
        && trigger_matches(&golem.frontmatter.triggers, &work.event, &delivery.action)
        && let Some(seat) = unattended_seat(github, &golem, Some(&delivery.sender)).await?
    {
        let id = occurrence_id(&format!("event-v2/{}/{}", work.delivery_id, golem.name));
        work.prepared = Some(prepare_request(github, &golem, json!({"delivery_id":id,"repository_id":delivery.repository.id,"repository":delivery.repository.full_name,"head_sha":head,"trigger":"event","issue_number":delivery.issue.as_ref().or(delivery.pull_request.as_ref()).map(|issue| issue.number),"comment_id":delivery.comment.as_ref().map(|comment| comment.id),"actor_id":delivery.sender.id,"actor_login":delivery.sender.login,"seat_login":seat,"instructions":instructions})).await?);
        // CAS this exact check/request before any admission. A rejected or uncertain
        // admission leaves it intact; next tick resubmits identical bytes, never a new run.
        return Ok(());
    }
    work.next += 1;
    Ok(())
}

// Completion retains the original annotated source, not a lossy summary.
async fn event_reference(github: &Github, prefix: &str, id: &str) -> Result<Option<String>> {
    let name = format!("refs/tags/{prefix}/{id}");
    let (status, value) = github
        .call(
            Method::Get,
            &format!("/repos/{OPS}/git/ref/tags/{prefix}/{id}"),
            None,
        )
        .await?;
    if status == 404 {
        return Ok(None);
    }
    if status != 200
        || value.get("ref").and_then(Value::as_str) != Some(name.as_str())
        || value.pointer("/object/type").and_then(Value::as_str) != Some("tag")
    {
        return Err(failure("invalid event receipt reference"));
    }
    let sha = value
        .pointer("/object/sha")
        .and_then(Value::as_str)
        .filter(|sha| valid_sha(sha))
        .ok_or_else(|| failure("invalid event receipt SHA"))?;
    Ok(Some(sha.into()))
}

async fn retire_event(github: &Github, id: &str, cursor_sha: Option<&str>) -> Result<()> {
    let source = event_reference(github, "golem-event", id).await?;
    let receipt = event_reference(github, "golem-event-done", id).await?;
    let sha = source
        .as_deref()
        .or(receipt.as_deref())
        .ok_or_else(|| failure("event retirement source missing"))?;
    if receipt.as_deref().is_some_and(|done| done != sha) {
        return Err(failure("event receipt source mismatch"));
    }
    if receipt.is_none() {
        let (status, _) = github
            .call(
                Method::Post,
                &format!("/repos/{OPS}/git/refs"),
                Some(json!({"ref":format!("refs/tags/golem-event-done/{id}"),"sha":sha})),
            )
            .await?;
        if !matches!(status, 201 | 422) {
            return Err(failure("event receipt publication failed"));
        }
    }
    if event_reference(github, "golem-event-done", id)
        .await?
        .as_deref()
        != Some(sha)
    {
        return Err(failure("event receipt verification failed"));
    }
    update_event_index(github, id, false).await?;
    if let Some(cursor_sha) = cursor_sha {
        delete_event_work(github, &event_path(id), cursor_sha).await?;
    }
    if source.is_some() {
        let (status, _) = github
            .call(
                Method::Delete,
                &format!("/repos/{OPS}/git/refs/tags/golem-event/{id}"),
                None,
            )
            .await?;
        if !matches!(status, 204 | 404) {
            return Err(failure("event source retirement failed"));
        }
    }
    Ok(())
}

async fn discover_event(github: &Github, index: &EventIndex) -> Result<()> {
    let refs = github
        .get(&format!("/repos/{OPS}/git/matching-refs/tags/golem-event/"))
        .await?;
    let refs = refs
        .as_array()
        .filter(|refs| refs.len() <= 512)
        .ok_or_else(|| failure("pending event source discovery limit exceeded"))?;
    for reference in refs {
        let id = reference
            .get("ref")
            .and_then(Value::as_str)
            .and_then(|name| name.strip_prefix("refs/tags/golem-event/"))
            .filter(|id| valid_delivery(id))
            .ok_or_else(|| failure("invalid pending event source"))?;
        if !index.ids.iter().any(|indexed| indexed == id) {
            if index.ids.len() >= 128 {
                worker::console_error!(
                    "event admission capacity exceeded; pending source retained"
                );
                return Ok(());
            }
            update_event_index(github, id, true).await?;
            break;
        }
    }
    Ok(())
}

async fn event_tick(env: &Env, github: &Github) -> Result<()> {
    // Include fresh config, owner lookup, duplicate claim, bounded index CAS,
    // check/DO and retirement. Failure retains durable work for another cron.
    if github.requests.get() > SUBREQUESTS_MAX - 22 {
        return Ok(());
    }
    let (index, _) = event_index(github).await?;
    discover_event(github, &index).await?;
    let (index, _) = event_index(github).await?;
    let Some(id) = index.ids.first() else {
        return Ok(());
    };
    // Move the selected head behind its peers before fallible external work.
    // Poisoned or capacity-blocked admission cannot monopolize every tick.
    if index.ids.len() > 1 {
        let (mut current, sha) = event_index(github).await?;
        if current.ids.first() == Some(id) {
            current.ids.rotate_left(1);
            save_event_work(github, EVENT_INDEX_PATH, &current, sha.as_deref()).await?;
        }
    }
    let path = event_path(id);
    let (status, file) = github
        .call(Method::Get, &format!("{path}?ref=golem-state"), None)
        .await?;
    let done = event_reference(github, "golem-event-done", id).await?;
    if done.is_some() {
        let sha = if status == 200 {
            Some(string(&file, "sha")?)
        } else if status == 404 {
            None
        } else {
            return Err(failure("event journal read failed"));
        };
        return retire_event(github, id, sha).await;
    }
    if status == 404 {
        // Immutable private source precedes index admission. A failed initial PUT
        // cannot strand the head: rebuild its cursor, never guess an execution ended.
        let work = recover_event_work(github, id).await?;
        if work.delivery_id != *id
            || work.next != 0
            || work.version != 2
            || work.golems.len() > GOLEMS_MAX
            || !work.delivery.accepted()
        {
            return Err(failure("invalid recovered event source"));
        }
        save_event_work(github, &path, &work, None).await?;
        return Ok(());
    }
    if status != 200 {
        return Err(failure("event journal read failed"));
    }
    let sha = string(&file, "sha")?;
    if !valid_sha(sha) {
        return Err(failure("invalid event journal SHA"));
    }
    let content = string(&file, "content")?;
    if content.len() > 262_144 || file.get("encoding").and_then(Value::as_str) != Some("base64") {
        return Err(failure("invalid event journal encoding"));
    }
    let bytes = STANDARD
        .decode(
            content
                .chars()
                .filter(|c| !c.is_ascii_whitespace())
                .collect::<String>(),
        )
        .map_err(|_| failure("invalid event journal encoding"))?;
    let mut work: EventWork =
        serde_json::from_slice(&bytes).map_err(|_| failure("invalid event journal"))?;
    if work.version != 2
        || work.delivery_id != *id
        || !valid_sha(&work.head)
        || !work.delivery.accepted()
        || work.golems.len() > GOLEMS_MAX
        || work.next > work.golems.len()
    {
        return Err(failure("invalid event journal identity"));
    }
    if work.next == work.golems.len() {
        return retire_event(github, id, Some(sha)).await;
    }
    resume_event(env, github, &mut work).await?;
    let mut sha = save_event_work(github, &path, &work, Some(sha)).await?;
    if work.prepared.is_some() && github.requests.get() <= SUBREQUESTS_MAX - 17 {
        resume_event(env, github, &mut work).await?;
        sha = save_event_work(github, &path, &work, Some(&sha)).await?;
    }
    if work.next == work.golems.len() && github.requests.get() <= SUBREQUESTS_MAX - 9 {
        retire_event(github, id, Some(&sha)).await?;
    }
    Ok(())
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct ScheduleCursor {
    version: u8,
    day: u64,
    page: u32,
    repository: Option<ScanRepository>,
    completed: Vec<u64>,
    sweep_new: bool,
    after: Option<String>,
    selected: Option<ScheduleSelection>,
    done: bool,
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct ScanRepository {
    name: String,
    id: u64,
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct ScheduleSelection {
    repository: String,
    repository_id: u64,
    golem: String,
    definition_sha: String,
    minute: u64,
    #[serde(default)]
    prepared: Option<Value>,
}

impl ScheduleCursor {
    fn new(day: u64) -> Self {
        Self {
            version: 2,
            day,
            page: 1,
            repository: None,
            completed: Vec::new(),
            sweep_new: false,
            after: None,
            selected: None,
            done: false,
        }
    }
}

const SCHEDULE_PATH: &str = "/repos/isp-insoft-gmbh/isp-golem-xiv-ops/contents/scheduler-v2.json";

async fn schedule_cursor(github: &Github, day: u64) -> Result<(ScheduleCursor, Option<String>)> {
    let (status, value) = github
        .call(
            Method::Get,
            &format!("{SCHEDULE_PATH}?ref=golem-state"),
            None,
        )
        .await?;
    if status == 404 {
        if scheduler_marker(github).await? {
            return Err(failure("established scheduler journal missing"));
        }
        // Establish the recoverable empty journal before marking it initialized.
        // An unsuccessful first PUT remains safely retryable after restart.
        let cursor = ScheduleCursor::new(day);
        let sha = save_schedule_cursor(github, &cursor, None).await?;
        let id = occurrence_id("scheduler-v2-journal-initialized");
        github.claim(&id, &id).await?;
        return Ok((cursor, Some(sha)));
    }
    if status != 200 || value.get("encoding").and_then(Value::as_str) != Some("base64") {
        return Err(failure("invalid scheduler journal"));
    }
    let sha = string(&value, "sha")?.to_owned();
    if !valid_sha(&sha) {
        return Err(failure("invalid scheduler journal SHA"));
    }
    let content = string(&value, "content")?;
    if content.len() > 262_144 {
        return Err(failure("scheduler journal too large"));
    }
    let bytes = STANDARD
        .decode(
            content
                .chars()
                .filter(|c| !c.is_ascii_whitespace())
                .collect::<String>(),
        )
        .map_err(|_| failure("invalid scheduler encoding"))?;
    let cursor: ScheduleCursor =
        serde_json::from_slice(&bytes).map_err(|_| failure("invalid scheduler cursor"))?;
    if cursor.version != 2
        || cursor.page == 0
        || cursor.page > 10_000
        || cursor.completed.len() > 10_000
        || cursor
            .repository
            .as_ref()
            .is_some_and(|repo| repo.id == 0 || !valid_repository(&repo.name))
        || cursor
            .after
            .as_deref()
            .is_some_and(|name| !crate::valid_golem_name(name))
        || cursor.selected.as_ref().is_some_and(|selected| {
            selected.minute / 1440 != cursor.day
                || !valid_repository(&selected.repository)
                || selected.repository_id == 0
                || !crate::valid_golem_name(&selected.golem)
                || !valid_sha(&selected.definition_sha)
        })
    {
        return Err(failure("invalid scheduler cursor bounds"));
    }
    if cursor.day > day {
        return Err(failure("scheduler clock moved backwards"));
    }
    if !scheduler_marker(github).await? {
        let id = occurrence_id("scheduler-v2-journal-initialized");
        github.claim(&id, &id).await?;
    }
    Ok((
        if cursor.day == day
            || cursor
                .selected
                .as_ref()
                .is_some_and(|selected| selected.prepared.is_some())
        {
            cursor
        } else {
            ScheduleCursor::new(day)
        },
        Some(sha),
    ))
}

async fn scheduler_marker(github: &Github) -> Result<bool> {
    let id = occurrence_id("scheduler-v2-journal-initialized");
    let (status, _) = github
        .call(
            Method::Get,
            &format!("/repos/{OPS}/git/ref/tags/golem-delivery/{id}"),
            None,
        )
        .await?;
    match status {
        200 => Ok(true),
        404 => Ok(false),
        _ => Err(failure("scheduler marker read failed")),
    }
}

async fn save_schedule_cursor(
    github: &Github,
    cursor: &ScheduleCursor,
    sha: Option<&str>,
) -> Result<String> {
    let content = STANDARD
        .encode(serde_json::to_vec(cursor).map_err(|_| failure("scheduler serialization failed"))?);
    if content.len() > 262_144 {
        return Err(failure("scheduler journal capacity exceeded"));
    }
    let mut body =
        json!({"branch":"golem-state","message":"Advance daily scheduler","content":content});
    if let Some(sha) = sha {
        body.as_object_mut()
            .ok_or_else(|| failure("invalid scheduler body"))?
            .insert("sha".into(), json!(sha));
    }
    let (status, value) = github.call(Method::Put, SCHEDULE_PATH, Some(body)).await?;
    if !matches!(status, 200 | 201) {
        return Err(failure("scheduler CAS failed; resume next cron"));
    }
    value
        .pointer("/content/sha")
        .and_then(Value::as_str)
        .filter(|sha| valid_sha(sha))
        .map(str::to_owned)
        .ok_or_else(|| failure("invalid saved scheduler SHA"))
}

async fn schedule_tick(env: &Env, now_ms: u64) -> Result<()> {
    let github = Github::installation(env).await?;
    if !protocol_v2(&github).await? {
        return Ok(());
    }
    let tick = coordinate_budgeted(env, &github, "tick", json!({})).await?;
    if !(200..300).contains(&tick.status_code()) {
        return Err(failure("coordinator tick failed"));
    }
    let schedule_result = schedule_batch(env, &github, now_ms).await;
    // A broken scheduler journal must not prevent independent event continuation.
    let event_result = event_tick(env, &github).await;
    schedule_result.and(event_result)
}

async fn schedule_batch(env: &Env, github: &Github, now_ms: u64) -> Result<()> {
    let minute = now_ms / 60_000;
    let (mut cursor, mut sha) = schedule_cursor(github, minute / 1440).await?;
    // A converged sweep starts another bounded metadata scan on the next tick.
    // Completed IDs remain authoritative for the day; definition changes wait.
    cursor.done = false;
    if cursor
        .selected
        .as_ref()
        .is_some_and(|selected| minute < selected.minute)
    {
        return Ok(());
    }
    if let Some(mut selected) = cursor.selected.take() {
        if selected.prepared.is_none() && selected.minute == minute {
            selected.prepared = prepare_schedule(github, &selected, cursor.day).await?;
            if selected.prepared.is_some() {
                cursor.selected = Some(selected);
                // Persist exact request/check before claiming or attempting admission.
                // Only the CAS winner may proceed; retries reuse this payload unchanged.
                sha = Some(save_schedule_cursor(github, &cursor, sha.as_deref()).await?);
                selected = cursor
                    .selected
                    .take()
                    .ok_or_else(|| failure("prepared schedule missing"))?;
            }
        }
        if let Some(request) = &selected.prepared {
            let id = string(request, "delivery_id")?;
            // An existing matching daily claim is not proof of queue admission.
            // Coordinator alone decides idempotent admission/dispatch for this payload.
            github.claim(id, id).await?;
            admit_request(env, github, request).await?;
        }
        // Persisted identity, not an unstable directory offset, advances the scan.
        cursor.repository = Some(ScanRepository {
            name: selected.repository,
            id: selected.repository_id,
        });
        cursor.after = Some(selected.golem);
        save_schedule_cursor(github, &cursor, sha.as_deref()).await?;
        return Ok(());
    }
    discover_schedule(github, &mut cursor, now_ms).await?;
    save_schedule_cursor(github, &cursor, sha.as_deref()).await?;
    Ok(())
}

async fn discover_schedule(
    github: &Github,
    cursor: &mut ScheduleCursor,
    now_ms: u64,
) -> Result<()> {
    // One installation repository per cron, one occurrence per batch: durable continuation,
    // never reading contents of non-allowlisted installation repositories.
    let info = if let Some(repository) = &cursor.repository {
        json!({"full_name":repository.name,"id":repository.id,"topics":[TEST_TOPIC]})
    } else {
        let page = github
            .get(&format!(
                "/installation/repositories?per_page=1&page={}",
                cursor.page
            ))
            .await?;
        let repositories = page
            .get("repositories")
            .and_then(Value::as_array)
            .filter(|repos| repos.len() <= 1)
            .ok_or_else(|| failure("invalid installation repository batch"))?;
        let Some(info) = repositories.first() else {
            // A second sweep discovers repos moved across earlier page boundaries.
            // Completed IDs prevent another execution or transferring a name cursor.
            cursor.done = !cursor.sweep_new;
            cursor.page = 1;
            cursor.sweep_new = false;
            return Ok(());
        };
        info.clone()
    };
    let repository = string(&info, "full_name")?;
    let scan_id = info
        .get("id")
        .and_then(Value::as_u64)
        .filter(|id| *id > 0)
        .ok_or_else(|| failure("invalid installation repository ID"))?;
    let allowlisted = !cursor.completed.contains(&scan_id)
        && valid_repository(repository)
        && info
            .get("topics")
            .and_then(Value::as_array)
            .is_some_and(|topics| {
                topics
                    .iter()
                    .any(|topic| topic.as_str() == Some(TEST_TOPIC))
            });
    if allowlisted {
        // Re-read metadata at execution time: revoked topics cannot authorize a run.
        if let Some(current) = github.repository(repository).await? {
            let repository_id = current
                .get("id")
                .and_then(Value::as_u64)
                .filter(|id| *id > 0)
                .ok_or_else(|| failure("invalid scheduled repository ID"))?;
            if repository_id != scan_id {
                return Err(failure("scheduled repository identity changed"));
            }
            cursor.repository = Some(ScanRepository {
                name: repository.into(),
                id: repository_id,
            });
            let head = github.head(repository, &current).await?;
            if let Some(golem) = github
                .next_golem(repository, &head, cursor.after.as_deref())
                .await?
            {
                if supported(&golem)
                    && let Some(due) = golem
                        .frontmatter
                        .schedule
                        .as_ref()
                        .and_then(|schedule| next_schedule_minute(schedule, now_ms))
                {
                    cursor.selected = Some(ScheduleSelection {
                        repository: repository.into(),
                        repository_id,
                        golem: golem.name.clone(),
                        definition_sha: head,
                        minute: due,
                        prepared: None,
                    });
                } else {
                    cursor.after = Some(golem.name.clone());
                }
                return Ok(());
            }
        }
    }
    cursor.page = cursor
        .page
        .checked_add(1)
        .filter(|page| *page <= 10_000)
        .ok_or_else(|| failure("installation scan capacity exceeded"))?;
    if !cursor.completed.contains(&scan_id) {
        if cursor.completed.len() >= 10_000 {
            return Err(failure("daily repository capacity exceeded"));
        }
        cursor.completed.push(scan_id);
        cursor.sweep_new = true;
    }
    cursor.repository = None;
    cursor.after = None;
    cursor.selected = None;
    Ok(())
}

async fn prepare_schedule(
    github: &Github,
    selected: &ScheduleSelection,
    day: u64,
) -> Result<Option<Value>> {
    let repository = &selected.repository;
    let Some(info) = github.repository(repository).await? else {
        return Ok(None);
    };
    if info.get("id").and_then(Value::as_u64) != Some(selected.repository_id) {
        return Ok(None);
    }
    let head = github.head(repository, &info).await?;
    let Some(golem) = github.golem(repository, &head, &selected.golem).await? else {
        return Ok(None);
    };
    if supported(&golem)
        && golem
            .frontmatter
            .schedule
            .as_ref()
            .is_some_and(|schedule| crate::schedule_matches(schedule, day))
        && let Some(seat) = unattended_seat(github, &golem, None).await?
    {
        let id = occurrence_id(&format!(
            "schedule-v2/{}/{}/{day}",
            selected.repository_id, golem.name
        ));
        return prepare_request(github, &golem, json!({"delivery_id":id,"repository_id":selected.repository_id,"repository":repository,"head_sha":head,"trigger":"schedule","issue_number":null,"comment_id":null,"actor_id":null,"actor_login":null,"seat_login":seat,"instructions":"Scheduled daily run."})).await.map(Some);
    }
    Ok(None)
}

#[worker::event(scheduled)]
async fn scheduled(event: worker::ScheduledEvent, env: Env, _ctx: worker::ScheduleContext) {
    let result = async {
        let now_ms = runtime_timestamp(event.schedule())?;
        bounded(schedule_tick(&env, now_ms), 27_000).await
    }
    .await;
    if result.is_err() {
        worker::console_error!("scheduled batch failed; durable cursor/claims retained");
    }
}

async fn index_request(request: &Request, repository: &str) -> Result<Response> {
    let Some(authorization) = request.headers().get("Authorization")? else {
        return Response::error("GitHub token required", 401);
    };
    let Some(token) = authorization.strip_prefix("Bearer ").filter(|token| {
        !token.is_empty() && token.len() <= 4096 && !token.chars().any(char::is_whitespace)
    }) else {
        return Response::error("Invalid authorization", 401);
    };
    let github = Github::new(token.into());
    // Even public repository metadata must not make an invalid token look valid.
    let (status, _) = github.call(Method::Get, "/user", None).await?;
    if status != 200 {
        return Response::error(
            "GitHub authorization rejected",
            if status == 401 { 401 } else { 403 },
        );
    }
    let (status, _) = github
        .call(Method::Get, &format!("/repos/{repository}"), None)
        .await?;
    if status != 200 {
        return Response::error(
            "Repository unavailable",
            if status == 404 { 404 } else { 403 },
        );
    }
    let Some(info) = github.repository(repository).await? else {
        return Response::error("Repository unavailable", 404);
    };
    let head = github.head(repository, &info).await?;
    let golems = github.index(repository, &head).await?;
    let mut response = Response::from_json(&json!({"golems":golems}))?;
    response
        .headers_mut()
        .set("Cache-Control", "private, no-store")?;
    Ok(response)
}

#[worker::event(fetch)]
async fn fetch(mut request: Request, env: Env, ctx: Context) -> Result<Response> {
    let path = request.path();
    if request.method() == Method::Get && path == "/health" {
        let version = env.var("VERSION")?.to_string();
        if version.trim().is_empty() {
            return Response::error("Version not configured", 503);
        }
        return Response::from_json(&json!({"app":APP,"version":version}));
    }
    if request.method() == Method::Get
        && let Some(repository) = path
            .strip_prefix("/v1/repos/")
            .and_then(|path| path.strip_suffix("/golems"))
    {
        if !valid_repository(repository) {
            return Response::error("Repository unavailable", 404);
        }
        return if let Ok(response) = bounded(index_request(&request, repository), 25_000).await {
            Ok(response)
        } else {
            worker::console_error!("golem index failed (details suppressed)");
            Response::error("Index temporarily unavailable", 502)
        };
    }
    // Without a session every page sends the browser to GitHub sign-in; state binds the callback.
    if request.method() == Method::Get && matches!(path.as_str(), "/seats" | "/admin") {
        let state = random_state();
        let Some(location) = crate::sign_in_location(&state) else {
            return Response::error("Sign-in unavailable", 503);
        };
        let headers = Headers::new();
        headers.set("Location", &location)?;
        headers.set(
            "Set-Cookie",
            &format!("golem_oauth_state={state}; Path=/auth/github; Max-Age=600; HttpOnly; Secure; SameSite=Lax"),
        )?;
        headers.set("Cache-Control", "no-store")?;
        return Ok(Response::empty()?.with_status(302).with_headers(headers));
    }
    if request.method() == Method::Post && path == "/internal/queue" {
        return match bounded(internal_queue(&mut request, &env), 25_000).await {
            Ok(response) => Ok(response),
            Err(_) => Response::error("Queue unavailable", 502),
        };
    }
    if request.method() != Method::Post || path != "/webhook" {
        return Response::error("Not found", 404);
    }
    webhook(request, env, ctx).await
}

async fn webhook(mut request: Request, env: Env, ctx: Context) -> Result<Response> {
    let signature = request
        .headers()
        .get("X-Hub-Signature-256")?
        .unwrap_or_default();
    if signature.len() != 71 {
        return Response::error("Invalid signature", 401);
    }
    if request
        .headers()
        .get("Content-Length")?
        .is_some_and(|length| {
            length
                .parse::<usize>()
                .map_or(true, |length| length > BODY_BYTES_MAX)
        })
    {
        return Response::error("Webhook body too large", 413);
    }
    let stream = request.stream()?;
    let Ok(bytes) = bounded(read_stream(stream, BODY_BYTES_MAX), 5000).await else {
        return Response::error("Webhook body limit exceeded", 413);
    };
    let secret = env.secret("WEBHOOK_SECRET")?.to_string();
    if !verify_signature(secret.as_bytes(), &signature, &bytes) {
        return Response::error("Invalid signature", 401);
    }
    let event = request.headers().get("X-GitHub-Event")?.unwrap_or_default();
    if !matches!(
        event.as_str(),
        "issues" | "issue_comment" | "pull_request" | "pull_request_review" | "push"
    ) {
        return Response::ok("ignored");
    }
    let delivery: EventDelivery = match serde_json::from_slice(&bytes) {
        Ok(delivery) => delivery,
        Err(_) => return Response::error("Invalid webhook payload", 400),
    };
    // App-origin events never cause API calls, claims, or replies, including other App surfaces.
    if !delivery.accepted() {
        return Response::ok("ignored");
    }
    if delivery
        .comment
        .as_ref()
        .is_some_and(|comment| comment.body.len() > COMMENT_BYTES_MAX)
    {
        return Response::error("Comment too large", 413);
    }
    let mention: Option<Delivery> = if event == "issue_comment" {
        serde_json::from_slice::<Delivery>(&bytes)
            .ok()
            .filter(Delivery::accepted)
    } else {
        None
    };
    if mention.is_none()
        && crate::event_instructions(
            &event,
            &delivery.action,
            delivery
                .comment
                .as_ref()
                .map(|comment| comment.body.as_str()),
        )
        .is_none()
    {
        return Response::error("Event context too large or invalid", 413);
    }
    let delivery_id = request
        .headers()
        .get("X-GitHub-Delivery")?
        .unwrap_or_default();
    if !valid_delivery(&delivery_id) {
        return Response::error("Invalid delivery ID", 400);
    }
    let mut digest = String::with_capacity(64);
    for byte in Sha256::digest(&bytes) {
        write!(&mut digest, "{byte:02x}").map_err(|_| failure("digest encoding failed"))?;
    }
    let mention_request = mention.is_some();
    ctx.wait_until(async move {
        // waitUntil has a 30-second post-response lifetime; cap our work below it.
        if bounded(process(env, event, delivery, mention, delivery_id.clone(), digest), 27_000).await.is_err() {
            worker::console_error!("webhook processing failed; delivery={}; claim may be retained; investigate before replay", delivery_id);
        }
    });
    if mention_request {
        Ok(Response::ok("accepted")?.with_status(202))
    } else {
        Response::ok("ignored")
    }
}
