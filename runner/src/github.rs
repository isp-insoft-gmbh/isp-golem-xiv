use base64::{Engine as _, engine::general_purpose};
use golem_runner::{APP_ID, AgentOutput, DispatchV1, INSTALLATION_ID, OutputGuard, Result};
use hmac::{Hmac, KeyInit, Mac};
use jsonwebtoken::{Algorithm, EncodingKey, Header, encode};
use reqwest::{Client, Method, Response, StatusCode};
use serde::Serialize;
use serde_json::{Value, json};
use sha2::Sha256;
use std::{
    fmt::Write as _,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

const API: &str = "https://api.github.com";
const QUEUE: &str = "https://isp-golem-xiv.isp-insoft-gmbh.workers.dev/internal/queue";
const RESPONSE_BYTES_MAX: usize = 2_097_152;

pub struct Github {
    client: Client,
    base_url: String,
}

impl Github {
    #[cfg(test)]
    pub fn for_test(base_url: String) -> Result<Self> {
        Ok(Self {
            client: Client::builder()
                .no_proxy()
                .timeout(Duration::from_secs(3))
                .build()
                .map_err(|_| "cannot construct test client")?,
            base_url,
        })
    }

    pub fn new() -> Result<Self> {
        Ok(Self {
            base_url: API.to_owned(),
            client: Client::builder()
                .user_agent("isp-golem-xiv/v2")
                .redirect(reqwest::redirect::Policy::none())
                .timeout(Duration::from_secs(60))
                .https_only(true)
                .build()
                .map_err(|_| "cannot construct GitHub client")?,
        })
    }

    async fn call(
        &self,
        method: Method,
        path: &str,
        token: &str,
        body: Option<Value>,
    ) -> Result<Response> {
        let mut request = self
            .client
            .request(method, format!("{}{path}", self.base_url))
            .bearer_auth(token)
            .header("Accept", "application/vnd.github+json")
            .header("X-GitHub-Api-Version", "2022-11-28");
        if let Some(body) = body {
            request = request.json(&body);
        }
        request.send().await.map_err(|_| "GitHub request failed")
    }

    pub async fn json(
        &self,
        method: Method,
        path: &str,
        token: &str,
        body: Option<Value>,
    ) -> Result<Value> {
        let response = self.call(method, path, token, body).await?;
        if !response.status().is_success() {
            return Err("GitHub rejected request");
        }
        let bytes = bounded_body(response, RESPONSE_BYTES_MAX).await?;
        serde_json::from_slice(&bytes).map_err(|_| "invalid GitHub response")
    }

    pub async fn token(
        &self,
        private_key: &str,
        request: &DispatchV1,
        publish: bool,
        guard: &mut OutputGuard,
    ) -> Result<String> {
        #[derive(Serialize)]
        struct Claims {
            iat: u64,
            exp: u64,
            iss: String,
        }
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| "invalid clock")?
            .as_secs();
        let key = EncodingKey::from_rsa_pem(private_key.as_bytes())
            .map_err(|_| "invalid App private key")?;
        let jwt = encode(
            &Header::new(Algorithm::RS256),
            &Claims {
                iat: now.saturating_sub(60),
                exp: now + 540,
                iss: APP_ID.to_string(),
            },
            &key,
        )
        .map_err(|_| "App authentication failed")?;
        guard.add(&jwt);
        let permissions = if publish && request.version == 2 {
            json!({"contents":"write", "pull_requests":"write", "issues":"write", "checks":"write"})
        } else if publish {
            json!({"contents":"read", "issues":"write", "checks":"write"})
        } else if request.version == 2 {
            json!({"contents":"read", "checks":"write", "issues":"read", "members":"read", "pull_requests":"read"})
        } else {
            json!({"contents":"read", "checks":"write", "issues":"read", "members":"read"})
        };
        let response = self
            .json(
                Method::POST,
                &format!("/app/installations/{INSTALLATION_ID}/access_tokens"),
                &jwt,
                Some(json!({"repository_ids":[request.repository_id], "permissions":permissions})),
            )
            .await?;
        let token = response
            .get("token")
            .and_then(Value::as_str)
            .ok_or("missing installation token")?
            .to_owned();
        if token.len() < 16 || token.len() > 4096 {
            return Err("invalid installation token");
        }
        guard.add(&token);
        Ok(token)
    }

    async fn member(&self, login: &str, token: &str) -> Result<()> {
        let response = self
            .call(
                Method::GET,
                &format!("/orgs/isp-insoft-gmbh/members/{login}"),
                token,
                None,
            )
            .await?;
        if response.status() != StatusCode::NO_CONTENT {
            return Err("seat or actor is not an organization member");
        }
        Ok(())
    }

    pub async fn validate_target(&self, request: &DispatchV1, token: &str) -> Result<()> {
        let repository = self
            .json(
                Method::GET,
                &format!("/repos/{}", request.repository),
                token,
                None,
            )
            .await?;
        if repository.get("id").and_then(Value::as_u64) != Some(request.repository_id)
            || repository.get("full_name").and_then(Value::as_str)
                != Some(request.repository.as_str())
            || !repository
                .get("topics")
                .and_then(Value::as_array)
                .is_some_and(|topics| {
                    topics
                        .iter()
                        .any(|topic| topic.as_str() == Some("isp-golem-xiv-test"))
                })
        {
            return Err("target identity or allowlist mismatch");
        }
        self.member(&request.seat_login, token).await?;
        if request.human() {
            self.validate_human(request, token).await?;
        }
        let check = self
            .json(
                Method::GET,
                &format!(
                    "/repos/{}/check-runs/{}",
                    request.repository, request.check_run_id
                ),
                token,
                None,
            )
            .await?;
        if check.get("head_sha").and_then(Value::as_str) != Some(request.head_sha.as_str())
            || check.get("name").and_then(Value::as_str)
                != Some(format!("golem/{}", request.golem).as_str())
            || check.pointer("/app/id").and_then(Value::as_u64) != Some(APP_ID)
        {
            return Err("check identity mismatch");
        }
        if request.version == 2 {
            let configuration = self
                .json(
                    Method::GET,
                    &format!(
                        "/repos/{}/contents/.golems/{}.md?ref={}",
                        request.repository, request.golem, request.head_sha
                    ),
                    token,
                    None,
                )
                .await?;
            if configuration.get("type").and_then(Value::as_str) != Some("file")
                || configuration.get("encoding").and_then(Value::as_str) != Some("base64")
            {
                return Err("invalid default branch definition");
            }
            let encoded = configuration
                .get("content")
                .and_then(Value::as_str)
                .ok_or("missing default branch definition")?;
            let bytes = general_purpose::STANDARD
                .decode(encoded.replace(['\r', '\n'], ""))
                .map_err(|_| "invalid definition encoding")?;
            let document = std::str::from_utf8(&bytes).map_err(|_| "invalid definition text")?;
            request.validate_definition(document)?;
        }
        Ok(())
    }

    async fn validate_human(&self, request: &DispatchV1, token: &str) -> Result<()> {
        let actor = request
            .actor_login
            .as_deref()
            .ok_or("missing actor login")?;
        if !actor.eq_ignore_ascii_case(&request.seat_login) {
            return Err("human seat mismatch");
        }
        // The seat membership check already covers the actor when both identities coincide.
        let comment_id = request.comment_id.ok_or("missing comment identity")?;
        let issue = request.issue_number.ok_or("missing issue identity")?;
        let comment = self
            .json(
                Method::GET,
                &format!("/repos/{}/issues/comments/{comment_id}", request.repository),
                token,
                None,
            )
            .await?;
        if comment.pointer("/user/id").and_then(Value::as_u64) != request.actor_id
            || !comment
                .pointer("/user/login")
                .and_then(Value::as_str)
                .is_some_and(|login| login.eq_ignore_ascii_case(actor))
            || comment.get("issue_url").and_then(Value::as_str)
                != Some(format!("{API}/repos/{}/issues/{issue}", request.repository).as_str())
        {
            return Err("trigger identity mismatch");
        }
        Ok(())
    }

    fn ownership_marker(request: &DispatchV1) -> String {
        format!(
            "<!-- isp-golem-xiv repository={} golem={} -->",
            request.repository_id, request.golem
        )
    }

    async fn owned_pull(&self, request: &DispatchV1, token: &str) -> Result<Option<Value>> {
        let branch = request.branch_name();
        let query: String = reqwest::Url::parse_with_params(
            "https://github.com",
            [
                ("state", "open"),
                ("head", &format!("isp-insoft-gmbh:{branch}")),
                ("per_page", "100"),
            ],
        )
        .map_err(|_| "invalid branch query")?
        .query()
        .ok_or("missing branch query")?
        .to_owned();
        let pulls = self
            .json(
                Method::GET,
                &format!("/repos/{}/pulls?{query}", request.repository),
                token,
                None,
            )
            .await?;
        let pulls = pulls.as_array().ok_or("invalid pull request list")?;
        if pulls.len() > 1 {
            return Err("ambiguous owned pull request");
        }
        let Some(pull) = pulls.first() else {
            return Ok(None);
        };
        if pull.pointer("/user/login").and_then(Value::as_str) != Some("isp-golem-xiv[bot]")
            || pull.pointer("/head/ref").and_then(Value::as_str) != Some(branch.as_str())
            || pull.pointer("/head/repo/id").and_then(Value::as_u64) != Some(request.repository_id)
            || !pull
                .get("body")
                .and_then(Value::as_str)
                .is_some_and(|body| {
                    body.lines()
                        .any(|line| line == Self::ownership_marker(request))
                })
        {
            return Err("branch ownership mismatch");
        }
        Ok(Some(pull.clone()))
    }

    pub async fn mark_blocked(&self, request: &DispatchV1, token: &str) -> Result<()> {
        let Some(pull) = self.owned_pull(request, token).await? else {
            return Ok(());
        };
        let number = pull
            .get("number")
            .and_then(Value::as_u64)
            .filter(|number| *number > 0)
            .ok_or("missing pull request number")?;
        let label_path = format!("/repos/{}/labels/blocked", request.repository);
        let lookup = self.call(Method::GET, &label_path, token, None).await?;
        if lookup.status() == StatusCode::NOT_FOUND {
            let created = self
                .call(
                    Method::POST,
                    &format!("/repos/{}/labels", request.repository),
                    token,
                    Some(json!({"name":"blocked", "color":"d73a4a"})),
                )
                .await?;
            if created.status() == StatusCode::UNPROCESSABLE_ENTITY {
                let recheck = self.call(Method::GET, &label_path, token, None).await?;
                if !recheck.status().is_success() {
                    return Err("GitHub rejected request");
                }
            } else if !created.status().is_success() {
                return Err("GitHub rejected request");
            }
        } else if !lookup.status().is_success() {
            return Err("GitHub rejected request");
        }
        self.json(
            Method::POST,
            &format!("/repos/{}/issues/{number}/labels", request.repository),
            token,
            Some(json!({"labels":["blocked"]})),
        )
        .await?;
        Ok(())
    }

    async fn branch_sha(&self, request: &DispatchV1, token: &str) -> Result<Option<String>> {
        let branch = request.branch_name();
        let response = self
            .call(
                Method::GET,
                &format!("/repos/{}/git/ref/heads/{branch}", request.repository),
                token,
                None,
            )
            .await?;
        if response.status() == StatusCode::NOT_FOUND {
            return Ok(None);
        }
        if !response.status().is_success() {
            return Err("branch lookup rejected");
        }
        let value: Value =
            serde_json::from_slice(&bounded_body(response, RESPONSE_BYTES_MAX).await?)
                .map_err(|_| "invalid branch response")?;
        if value.get("ref").and_then(Value::as_str) != Some(format!("refs/heads/{branch}").as_str())
            || value.pointer("/object/type").and_then(Value::as_str) != Some("commit")
        {
            return Err("invalid branch reference");
        }
        Ok(Some(response_sha(
            value.get("object").ok_or("missing branch object")?,
        )?))
    }

    pub async fn branch_snapshot(&self, request: &DispatchV1, token: &str) -> Result<String> {
        let Some(sha) = self.branch_sha(request, token).await? else {
            return Ok(request.head_sha.clone());
        };
        let pull = self
            .owned_pull(request, token)
            .await?
            .ok_or("existing branch has no ownership proof")?;
        if pull.pointer("/head/sha").and_then(Value::as_str) != Some(sha.as_str()) {
            return Err("branch changed during ownership validation");
        }
        Ok(sha)
    }

    pub async fn snapshot(&self, request: &DispatchV1, token: &str) -> Result<Vec<u8>> {
        self.snapshot_at(request, token, &request.head_sha).await
    }

    pub async fn snapshot_at(
        &self,
        request: &DispatchV1,
        token: &str,
        sha: &str,
    ) -> Result<Vec<u8>> {
        validate_sha(sha)?;
        let response = self
            .call(
                Method::GET,
                &format!("/repos/{}/tarball/{sha}", request.repository),
                token,
                None,
            )
            .await?;
        if response.status() != StatusCode::FOUND {
            return Err("snapshot redirect missing");
        }
        let location = response
            .headers()
            .get("location")
            .and_then(|value| value.to_str().ok())
            .ok_or("invalid snapshot URL")?;
        let url = reqwest::Url::parse(location).map_err(|_| "invalid snapshot URL")?;
        if url.scheme() != "https"
            || url.host_str() != Some("codeload.github.com")
            || url.port().is_some()
            || !url.username().is_empty()
            || url.password().is_some()
        {
            return Err("untrusted snapshot destination");
        }
        // No Authorization header follows the archive redirect.
        let response = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|_| "snapshot download failed")?;
        if response.status() != StatusCode::OK {
            return Err("snapshot download rejected");
        }
        bounded_body(response, 16 * 1024 * 1024).await
    }

    pub async fn publish_changes(
        &self,
        request: &DispatchV1,
        token: &str,
        base_sha: &str,
        changes: &[golem_runner::changes::Change],
        output: &AgentOutput,
        guard: &OutputGuard,
    ) -> Result<String> {
        if request.version != 2 {
            return Err("invalid change publication");
        }
        validate_publication(base_sha, output, changes, guard)?;
        let repository = self
            .json(
                Method::GET,
                &format!("/repos/{}", request.repository),
                token,
                None,
            )
            .await?;
        let base = repository
            .get("default_branch")
            .and_then(Value::as_str)
            .ok_or("missing default branch")?;
        if base == request.branch_name() {
            return Err("cannot publish to default branch");
        }
        let current = self.branch_sha(request, token).await?;
        if current.as_deref().unwrap_or(&request.head_sha) != base_sha {
            return Err("publication base changed");
        }
        let pull = self.owned_pull(request, token).await?;
        if current.is_some()
            && pull
                .as_ref()
                .and_then(|pull| pull.pointer("/head/sha"))
                .and_then(Value::as_str)
                != Some(base_sha)
        {
            return Err("branch ownership mismatch");
        }
        let prefix = format!("/repos/{}/git", request.repository);
        let message = format!(
            "golem/{}\n\n{}\nseat: {}/{}\nmodel: {}",
            request.golem,
            Self::ownership_marker(request),
            request.seat_login,
            request.provider,
            output.model
        );
        guard.check(&message)?;
        let sha = self
            .create_commit(&prefix, token, base_sha, changes, &message)
            .await?;
        // Revalidate immediately before ref publication. Non-force updates reject divergent races.
        if self.branch_sha(request, token).await? != current {
            return Err("publication branch changed");
        }
        let branch = request.branch_name();
        if current.is_some() {
            self.json(
                Method::PATCH,
                &format!("{prefix}/refs/heads/{branch}"),
                token,
                Some(json!({"sha":sha, "force":false})),
            )
            .await?;
        } else {
            self.json(
                Method::POST,
                &format!("{prefix}/refs"),
                token,
                Some(json!({"ref":format!("refs/heads/{branch}"), "sha":sha})),
            )
            .await?;
        }
        let body = format!(
            "{}\n\n{}\n\nseat: {}/{}\nmodel: {}",
            Self::ownership_marker(request),
            output.text,
            request.seat_login,
            request.provider,
            output.model
        );
        guard.check(&body)?;
        let published = if let Some(pull) = pull {
            let number = pull
                .get("number")
                .and_then(Value::as_u64)
                .ok_or("missing pull request number")?;
            self.json(
                Method::PATCH,
                &format!("/repos/{}/pulls/{number}", request.repository),
                token,
                Some(json!({"body":body})),
            )
            .await?
        } else {
            self.json(Method::POST, &format!("/repos/{}/pulls", request.repository), token, Some(json!({"head":branch, "base":base, "title":format!("golem/{}", request.golem), "body":body}))).await?
        };
        published
            .get("html_url")
            .and_then(Value::as_str)
            .map(str::to_owned)
            .ok_or("missing pull request URL")
    }

    async fn create_commit(
        &self,
        prefix: &str,
        token: &str,
        base_sha: &str,
        changes: &[golem_runner::changes::Change],
        message: &str,
    ) -> Result<String> {
        let commit = self
            .json(
                Method::GET,
                &format!("{prefix}/commits/{base_sha}"),
                token,
                None,
            )
            .await?;
        let tree_sha = response_sha(commit.get("tree").ok_or("missing base tree")?)?;
        let mut tree = Vec::with_capacity(changes.len());
        for change in changes {
            let (mode, sha) = if let Some(file) = &change.file {
                let blob = self.json(Method::POST, &format!("{prefix}/blobs"), token, Some(json!({"encoding":"base64", "content":general_purpose::STANDARD.encode(&file.bytes)}))).await?;
                (
                    if file.executable { "100755" } else { "100644" },
                    Some(response_sha(&blob)?),
                )
            } else {
                ("100644", None)
            };
            tree.push(json!({"path":change.path, "mode":mode, "type":"blob", "sha":sha}));
        }
        let tree = self
            .json(
                Method::POST,
                &format!("{prefix}/trees"),
                token,
                Some(json!({"base_tree":tree_sha, "tree":tree})),
            )
            .await?;
        let commit = self
            .json(
                Method::POST,
                &format!("{prefix}/commits"),
                token,
                Some(json!({"message":message, "tree":response_sha(&tree)?, "parents":[base_sha]})),
            )
            .await?;
        response_sha(&commit)
    }

    pub async fn check(
        &self,
        request: &DispatchV1,
        token: &str,
        outcome: &str,
        model: &str,
        run_id: &str,
        guard: &OutputGuard,
    ) -> Result<()> {
        self.check_reason(request, token, outcome, model, run_id, None, guard)
            .await
    }

    #[allow(clippy::too_many_arguments)] // Preserve the established check API while adding an explicit outcome reason.
    pub async fn check_reason(
        &self,
        request: &DispatchV1,
        token: &str,
        outcome: &str,
        model: &str,
        run_id: &str,
        reason: Option<&str>,
        guard: &OutputGuard,
    ) -> Result<()> {
        let conclusion = match outcome {
            "running" => None,
            "changed" | "replied" | "no_change" => Some("success"),
            "incomplete" => Some("neutral"),
            "blocked" => Some("failure"),
            "stopped" => Some("cancelled"),
            _ => return Err("invalid check outcome"),
        };
        if outcome == "no_change" && reason.is_none_or(|reason| reason.trim().is_empty()) {
            return Err("no_change requires reason");
        }
        let mut summary = request.summary(outcome, model, run_id);
        if let Some(reason) = reason {
            write!(summary, "\nreason: {reason}").map_err(|_| "cannot format check reason")?;
        }
        let mut body = json!({"status":if conclusion.is_some() { "completed" } else { "in_progress" }, "details_url":request.log_url(run_id), "output":{"title":format!("golem/{}: {outcome}", request.golem), "summary":summary}});
        if let Some(conclusion) = conclusion {
            body.as_object_mut()
                .ok_or("invalid check body")?
                .insert("conclusion".to_owned(), json!(conclusion));
        }
        guard.check(&body.to_string())?;
        for attempt in 0..3 {
            let result = self
                .json(
                    Method::PATCH,
                    &format!(
                        "/repos/{}/check-runs/{}",
                        request.repository, request.check_run_id
                    ),
                    token,
                    Some(body.clone()),
                )
                .await;
            match result {
                Ok(_) => return Ok(()),
                Err(error) if attempt == 2 => return Err(error),
                Err(_) => tokio::time::sleep(Duration::from_millis(250)).await,
            }
        }
        Err("check update retry budget exhausted")
    }

    pub async fn reply(
        &self,
        request: &DispatchV1,
        token: &str,
        output: &str,
        guard: &OutputGuard,
    ) -> Result<()> {
        let issue = request.issue_number.ok_or("reply requires issue context")?;
        let body = json!({"body": request.reply(output)});
        guard.check(&body.to_string())?;
        self.json(
            Method::POST,
            &format!("/repos/{}/issues/{issue}/comments", request.repository),
            token,
            Some(body),
        )
        .await?;
        Ok(())
    }
}

fn validate_publication(
    base_sha: &str,
    output: &AgentOutput,
    changes: &[golem_runner::changes::Change],
    guard: &OutputGuard,
) -> Result<()> {
    if changes.is_empty() || changes.len() > 4096 {
        return Err("invalid change publication");
    }
    validate_sha(base_sha)?;
    guard.check(&output.text)?;
    guard.check(&output.model)?;
    let mut before = golem_runner::changes::Files::new();
    let mut after = golem_runner::changes::Files::new();
    let mut total = 0_usize;
    let mut paths = std::collections::BTreeSet::new();
    // Scan the entire bounded batch before publishing even an unreachable blob.
    for change in changes {
        if !paths.insert(&change.path) {
            return Err("duplicate changed path");
        }
        if let Some(file) = &change.file {
            total = total.saturating_add(file.bytes.len());
            if total > 16 * 1024 * 1024 {
                return Err("change bytes exceed limit");
            }
            after.insert(change.path.clone(), file.clone());
        } else {
            before.insert(
                change.path.clone(),
                golem_runner::changes::FileState {
                    bytes: vec![0],
                    executable: false,
                },
            );
        }
    }
    golem_runner::changes::diff(&before, &after, guard)?;
    Ok(())
}

fn validate_sha(sha: &str) -> Result<()> {
    if sha.len() != 40 || !sha.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err("invalid Git object SHA");
    }
    Ok(())
}

fn response_sha(value: &Value) -> Result<String> {
    let sha = value
        .get("sha")
        .and_then(Value::as_str)
        .ok_or("missing Git object SHA")?;
    validate_sha(sha)?;
    Ok(sha.to_owned())
}

fn queue_body(
    request: &DispatchV1,
    run_id: &str,
    attempt: &str,
    action: &str,
    timestamp_ms: u128,
) -> Result<Vec<u8>> {
    if request.version != 2
        || !["bind", "complete"].contains(&action)
        || [run_id, attempt].iter().any(|value| {
            value.is_empty()
                || value.len() > 20
                || !value.bytes().all(|byte| byte.is_ascii_digit())
                || value.bytes().all(|byte| byte == b'0')
        })
    {
        return Err("invalid queue identity or action");
    }
    let run_id = run_id.parse::<u64>().map_err(|_| "invalid queue run id")?;
    let attempt = attempt
        .parse::<u64>()
        .map_err(|_| "invalid queue attempt")?;
    if run_id > 9_007_199_254_740_991 || attempt != 1 {
        return Err("invalid queue run identity");
    }
    let generation = request
        .queue_generation
        .as_deref()
        .ok_or("missing queue generation")?;
    if generation.is_empty() {
        return Err("missing queue generation");
    }
    let mut body = json!({"action":action, "request_id":request.delivery_id, "generation":generation, "run_id":serde_json::Number::from(run_id), "attempt":serde_json::Number::from(attempt), "timestamp_ms":timestamp_ms});
    if action == "complete" {
        // Calling complete is the native supervisor's attestation, never container input.
        let object = body.as_object_mut().ok_or("invalid queue body")?;
        object.insert("containers_destroyed".to_owned(), json!(true));
        object.insert("publication_complete".to_owned(), json!(true));
    }
    serde_json::to_vec(&body).map_err(|_| "cannot encode queue request")
}

fn queue_signature(key: &str, body: &[u8]) -> Result<String> {
    let normalized = key.replace("\r\n", "\n");
    let key = normalized.trim();
    if key.is_empty() {
        return Err("missing queue signing key");
    }
    let mut mac =
        Hmac::<Sha256>::new_from_slice(key.as_bytes()).map_err(|_| "invalid queue signing key")?;
    mac.update(body);
    let mut signature = String::from("sha256=");
    for byte in mac.finalize().into_bytes() {
        write!(signature, "{byte:02x}").map_err(|_| "cannot encode queue signature")?;
    }
    Ok(signature)
}

/// Bind before agent access; call `complete` only after confirmed destruction of
/// every agent/verifier container and completion of all publication attempts.
/// Never retry execution after a denied or uncertain bind.
pub async fn queue(
    request: &DispatchV1,
    key: &str,
    run_id: &str,
    attempt: &str,
    action: &str,
) -> Result<()> {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| "invalid clock")?
        .as_millis();
    let body = queue_body(request, run_id, attempt, action, timestamp)?;
    let signature = queue_signature(key, &body)?;
    let client = Client::builder()
        .https_only(true)
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(60))
        .build()
        .map_err(|_| "cannot construct queue client")?;
    let response = client
        .post(QUEUE)
        .header("Content-Type", "application/json")
        .header("X-Golem-Signature-256", signature)
        .body(body)
        .send()
        .await
        .map_err(|_| "queue request failed; execution denied")?;
    if !response.status().is_success() {
        return Err("queue request denied");
    }
    let value: Value = serde_json::from_slice(&bounded_body(response, 4096).await?)
        .map_err(|_| "invalid queue response")?;
    validate_queue_response(action, &value)
}

fn validate_queue_response(action: &str, value: &Value) -> Result<()> {
    let expected = match action {
        "bind" => "granted",
        "complete" => "terminal",
        _ => return Err("invalid queue action"),
    };
    if value.get("state").and_then(Value::as_str) != Some(expected) {
        return Err("queue ownership not confirmed");
    }
    Ok(())
}

async fn bounded_body(mut response: Response, maximum: usize) -> Result<Vec<u8>> {
    if response
        .content_length()
        .is_some_and(|length| length > maximum as u64)
    {
        return Err("GitHub response exceeds limit");
    }
    let mut body = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| "GitHub response interrupted")?
    {
        if body.len().saturating_add(chunk.len()) > maximum {
            return Err("GitHub response exceeds limit");
        }
        body.extend_from_slice(&chunk);
    }
    Ok(body)
}

#[cfg(test)]
#[path = "github_tests.rs"]
mod tests;
