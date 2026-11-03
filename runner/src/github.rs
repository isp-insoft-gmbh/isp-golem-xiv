use golem_runner::{APP_ID, DispatchV1, INSTALLATION_ID, OutputGuard, Result};
use jsonwebtoken::{Algorithm, EncodingKey, Header, encode};
use reqwest::{Client, Method, Response, StatusCode};
use serde::Serialize;
use serde_json::{Value, json};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const API: &str = "https://api.github.com";
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
                .user_agent("isp-golem-xiv/v0")
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
        let permissions = if publish {
            json!({"contents":"read", "issues":"write", "checks":"write"})
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
        let member = self
            .call(
                Method::GET,
                &format!("/orgs/isp-insoft-gmbh/members/{}", request.actor_login),
                token,
                None,
            )
            .await?;
        if member.status() != StatusCode::NO_CONTENT {
            return Err("triggering actor is not an organization member");
        }
        let comment = self
            .json(
                Method::GET,
                &format!(
                    "/repos/{}/issues/comments/{}",
                    request.repository, request.comment_id
                ),
                token,
                None,
            )
            .await?;
        if comment.pointer("/user/id").and_then(Value::as_u64) != Some(request.actor_id)
            || !comment
                .pointer("/user/login")
                .and_then(Value::as_str)
                .is_some_and(|login| login.eq_ignore_ascii_case(&request.actor_login))
            || comment.get("issue_url").and_then(Value::as_str)
                != Some(
                    format!(
                        "{API}/repos/{}/issues/{}",
                        request.repository, request.issue_number
                    )
                    .as_str(),
                )
        {
            return Err("trigger identity mismatch");
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
        Ok(())
    }

    pub async fn snapshot(&self, request: &DispatchV1, token: &str) -> Result<Vec<u8>> {
        let response = self
            .call(
                Method::GET,
                &format!("/repos/{}/tarball/{}", request.repository, request.head_sha),
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

    pub async fn check(
        &self,
        request: &DispatchV1,
        token: &str,
        outcome: &str,
        model: &str,
        run_id: &str,
        guard: &OutputGuard,
    ) -> Result<()> {
        let status = if outcome == "running" {
            "in_progress"
        } else {
            "completed"
        };
        let mut body = json!({
            "status": status,
            "details_url": request.log_url(run_id),
            "output": {"title": format!("golem/{}: {outcome}", request.golem), "summary": request.summary(outcome, model, run_id)}
        });
        if status == "completed" {
            body.as_object_mut().ok_or("invalid check payload")?.insert(
                "conclusion".to_owned(),
                json!(if outcome == "replied" {
                    "success"
                } else {
                    "failure"
                }),
            );
        }
        guard.check(&body.to_string())?;
        // PATCH is idempotent: retry completion safely without duplicating requester replies.
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
        let body = json!({"body": request.reply(output)});
        guard.check(&body.to_string())?;
        self.json(
            Method::POST,
            &format!(
                "/repos/{}/issues/{}/comments",
                request.repository, request.issue_number
            ),
            token,
            Some(body),
        )
        .await?;
        Ok(())
    }
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
