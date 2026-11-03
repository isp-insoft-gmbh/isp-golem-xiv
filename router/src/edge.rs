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
    APP, BOT, COMMENT_BYTES_MAX, Delivery, DispatchV1, FILE_BYTES_MAX, GOLEMS_MAX, Golem, OPS,
    ORGANIZATION, TEST_TOPIC, parse_golem, parse_mention, refusal, seat_secret, valid_delivery,
    valid_repository, verify_signature,
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
            let file = self
                .get(&format!(
                    "/repos/{repository}/contents/.golems/{filename}?ref={sha}"
                ))
                .await?;
            if file.get("type").and_then(Value::as_str) != Some("file")
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
                .filter(|character| !character.is_ascii_whitespace())
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
            golems.push(golem);
        }
        golems.sort_by(|left, right| left.name.cmp(&right.name));
        Ok(golems)
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

async fn process(env: Env, delivery: Delivery, delivery_id: String, digest: String) -> Result<()> {
    let github = Github::installation(&env).await?;
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
    if request.method() != Method::Post || path != "/webhook" {
        return Response::error("Not found", 404);
    }
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
    if event != "issue_comment" {
        return Response::ok("ignored");
    }
    let delivery: Delivery = match serde_json::from_slice(&bytes) {
        Ok(delivery) => delivery,
        Err(_) => return Response::error("Invalid webhook payload", 400),
    };
    // App-origin events never cause API calls, claims, or replies.
    if delivery.sender.login == BOT || !delivery.accepted() {
        return Response::ok("ignored");
    }
    if delivery.comment.body.len() > COMMENT_BYTES_MAX {
        return Response::error("Comment too large", 413);
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
    ctx.wait_until(async move {
        // waitUntil has a 30-second post-response lifetime; cap our work below it.
        if bounded(process(env, delivery, delivery_id.clone(), digest), 27_000).await.is_err() {
            worker::console_error!("webhook processing failed; delivery={}; claim may be retained; investigate before replay", delivery_id);
        }
    });
    Ok(Response::ok("accepted")?.with_status(202))
}
