#![forbid(unsafe_code)]

pub mod changes;

use base64::{Engine as _, engine::general_purpose};
use serde::{Deserialize, Serialize};
use std::fmt::Write as _;

pub const APP_ID: u64 = 4_605_060;
pub const INSTALLATION_ID: u64 = 153_964_219;
pub const OPS_REPOSITORY: &str = "isp-insoft-gmbh/isp-golem-xiv-ops";
pub const REQUEST_BYTES_MAX: usize = 24_000;
pub const OUTPUT_BYTES_MAX: usize = 1_048_576;
pub type Result<T> = std::result::Result<T, &'static str>;

/// Reject workflow reruns before credentials or agent preparation are reached.
///
/// # Errors
///
/// Rejects every attempt other than the initial GitHub Actions run.
pub fn initial_workflow_attempt(attempt: &str) -> Result<()> {
    if attempt == "1" {
        Ok(())
    } else {
        Err("workflow rerun refused before agent access")
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DispatchV1 {
    pub version: u8,
    pub delivery_id: String,
    pub repository_id: u64,
    pub repository: String,
    pub head_sha: String,
    pub golem: String,
    pub issue_number: Option<u64>,
    pub comment_id: Option<u64>,
    pub actor_id: Option<u64>,
    pub actor_login: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trigger: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verify: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owner: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub queue_generation: Option<String>,
    pub seat_login: String,
    pub provider: String,
    pub harness: String,
    pub model: Option<String>,
    pub timeout_seconds: u64,
    pub instructions: String,
    pub check_run_id: u64,
}

fn identifier(value: &str, maximum: usize) -> bool {
    !value.is_empty()
        && value.len() <= maximum
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
        && !value.starts_with('-')
        && !value.ends_with('-')
        && !value.contains("--")
}

impl DispatchV1 {
    /// Validate again at the credential boundary, independently of router validation.
    ///
    /// # Errors
    /// Rejects malformed, oversized, unsupported, or inconsistent dispatches.
    pub fn parse(json: &str, repository: &str, golem: &str) -> Result<Self> {
        if json.len() > REQUEST_BYTES_MAX {
            return Err("request exceeds limit");
        }
        let request: Self = serde_json::from_str(json).map_err(|_| "invalid dispatch JSON")?;
        let (owner, repository_name) = request
            .repository
            .split_once('/')
            .ok_or("repository must be owner/name")?;
        let delivery_valid = request.delivery_id.len() == 36
            && request
                .delivery_id
                .bytes()
                .enumerate()
                .all(|(index, byte)| {
                    if [8, 13, 18, 23].contains(&index) {
                        byte == b'-'
                    } else {
                        byte.is_ascii_hexdigit()
                    }
                });
        let raw: serde_json::Value =
            serde_json::from_str(json).map_err(|_| "invalid dispatch JSON")?;
        if request.version == 1
            && ["trigger", "branch", "verify", "owner", "queue_generation"]
                .iter()
                .any(|field| raw.get(field).is_some())
        {
            return Err("version 1 cannot contain version 2 fields");
        }
        request.validate_context()?;
        if !matches!(request.version, 1 | 2)
            || !delivery_valid
            || owner != "isp-insoft-gmbh"
            || !identifier(owner, 39)
            || repository_name.is_empty()
            || repository_name.len() > 100
            || !repository_name
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || b"-_.".contains(&byte))
            || repository != request.repository
            || golem != request.golem
            || request.golem.is_empty()
            || request.golem.len() > 64
            || !request.golem.bytes().all(|byte| {
                byte.is_ascii_lowercase() || byte.is_ascii_digit() || b"-_".contains(&byte)
            })
            || request.golem == "_golems"
            || ["stop", "seat", "help"].contains(&request.golem.to_ascii_lowercase().as_str())
            || request.head_sha.len() != 40
            || !request
                .head_sha
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit())
            || !identifier(&request.seat_login, 39)
            || request.provider != "anthropic"
            || request.harness != "claude"
            || !(1..=18_000).contains(&request.timeout_seconds)
            || request.instructions.len() > 16_384
            || request.instructions.contains('\0')
            || [request.repository_id, request.check_run_id].contains(&0)
            || [request.issue_number, request.comment_id, request.actor_id].contains(&Some(0))
            || request.model.as_ref().is_some_and(|model| {
                model.len() > 128
                    || model.is_empty()
                    || model.starts_with('-')
                    || !model
                        .bytes()
                        .all(|byte| byte.is_ascii_alphanumeric() || b"-._".contains(&byte))
            })
        {
            return Err("unsupported or inconsistent dispatch");
        }
        Ok(request)
    }

    fn validate_context(&self) -> Result<()> {
        let actor = self.actor_login.as_deref();
        if self.actor_id.is_some() != actor.is_some()
            || actor.is_some_and(|login| {
                !(identifier(login, 39)
                    || (self.trigger.as_deref() == Some("event")
                        && login
                            .strip_suffix("[bot]")
                            .is_some_and(|name| identifier(name, 39))))
            })
            || (self.comment_id.is_some() && self.issue_number.is_none())
        {
            return Err("invalid dispatch context");
        }
        if self.version == 2 {
            if !matches!(
                self.trigger.as_deref(),
                Some("mention" | "event" | "schedule")
            ) || self
                .branch
                .as_deref()
                .is_none_or(|branch| !valid_branch(branch))
                || self
                    .queue_generation
                    .as_deref()
                    .is_none_or(|id| !router::valid_delivery(id))
                || self
                    .owner
                    .as_deref()
                    .is_some_and(|owner| !identifier(owner, 39))
                || self.verify.as_deref().is_some_and(|command| {
                    command.trim().is_empty()
                        || command.len() > 4096
                        || command.chars().any(char::is_control)
                })
            {
                return Err("invalid version 2 dispatch");
            }
            if self.trigger.as_deref() == Some("schedule")
                && (self.owner.is_none() || self.actor_id.is_some() || self.comment_id.is_some())
            {
                return Err("invalid scheduled context");
            }
        }
        if self.human() {
            if self.issue_number.is_none()
                || self.comment_id.is_none()
                || self.actor_id.is_none()
                || actor.is_none_or(|login| !login.eq_ignore_ascii_case(&self.seat_login))
            {
                return Err("human dispatch requires actor seat and comment");
            }
        } else {
            let actor_seat = self.trigger.as_deref() == Some("event")
                && actor.is_some_and(|login| {
                    identifier(login, 39) && login.eq_ignore_ascii_case(&self.seat_login)
                });
            let owner_seat = self
                .owner
                .as_deref()
                .is_some_and(|owner| owner.eq_ignore_ascii_case(&self.seat_login));
            if self.owner.is_none() || !(actor_seat || owner_seat) {
                return Err("unattended dispatch requires actor or owner seat");
            }
        }
        Ok(())
    }

    #[must_use]
    pub fn human(&self) -> bool {
        self.version == 1 || self.trigger.as_deref() == Some("mention")
    }

    #[must_use]
    pub fn branch_name(&self) -> String {
        self.branch
            .clone()
            .unwrap_or_else(|| format!("golems/{}", self.golem))
    }

    /// # Errors
    /// Rejects invalid definitions or dispatch settings not matching the pinned definition.
    pub fn validate_definition(&self, document: &str) -> Result<()> {
        let definition = router::parse_golem(&self.golem, document);
        if !definition.errors.is_empty() {
            return Err("invalid golem definition");
        }
        let timeout = definition
            .timeout_seconds()
            .ok_or("invalid definition timeout")?;
        let front = definition.frontmatter;
        if u64::from(timeout) != self.timeout_seconds
            || front.harness.as_deref().unwrap_or("claude") != self.harness
            || front.provider.as_deref().unwrap_or("anthropic") != self.provider
            || front.model != self.model
        {
            return Err("dispatch differs from definition");
        }
        if self.version == 2
            && (front
                .branch
                .unwrap_or_else(|| format!("golems/{}", self.golem))
                != self.branch_name()
                || front.verify != self.verify
                || front.owner != self.owner
                || (self.trigger.as_deref() == Some("event") && front.triggers.is_empty())
                || (self.trigger.as_deref() == Some("schedule") && front.schedule.is_none()))
        {
            return Err("version 2 dispatch differs from definition");
        }
        Ok(())
    }

    #[must_use]
    pub fn seat_secret_name(&self) -> String {
        format!(
            "SEAT_{}__ANTHROPIC",
            self.seat_login.to_ascii_uppercase().replace('-', "_")
        )
    }

    #[must_use]
    pub fn log_url(&self, run_id: &str) -> String {
        format!("https://github.com/{OPS_REPOSITORY}/actions/runs/{run_id}")
    }

    #[must_use]
    pub fn summary(&self, outcome: &str, model: &str, run_id: &str) -> String {
        format!(
            "outcome: {outcome}\nseat: {}/{}\nharness: {}\nmodel: {model}\nbudget: {}s\nlog: {}",
            self.seat_login,
            self.provider,
            self.harness,
            self.timeout_seconds,
            self.log_url(run_id)
        )
    }

    #[must_use]
    pub fn reply(&self, output: &str) -> String {
        format!(
            "<!-- isp-golem-xiv kind=reply golem={} -->\n\n{output}",
            self.golem
        )
    }
}

// Match Git ref rules without invoking Git on untrusted arguments.
#[allow(clippy::case_sensitive_file_extension_comparisons)]
fn valid_branch(branch: &str) -> bool {
    !branch.is_empty()
        && branch.len() <= 200
        && branch != "@"
        && !branch.contains("..")
        && !branch.contains("@{")
        && branch
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"/_-.".contains(&byte))
        && branch.split('/').all(|part| {
            !part.is_empty()
                && !part.starts_with('.')
                && !part.ends_with('.')
                && !part.ends_with(".lock")
        })
}

/// Fail closed on handled secrets, common encodings, and recognizable credential formats.
/// This is defense in depth, not protection against arbitrary covert encodings.
#[derive(Default)]
pub struct OutputGuard {
    variants: Vec<String>,
}

impl OutputGuard {
    pub fn add(&mut self, secret: &str) {
        if secret.len() < 8 {
            return;
        }
        self.variants.push(secret.to_owned());
        for line in secret
            .lines()
            .filter(|line| line.len() >= 16 && !line.starts_with("-----"))
        {
            self.variants.push(line.to_owned());
        }
        self.variants.push(general_purpose::STANDARD.encode(secret));
        self.variants
            .push(general_purpose::STANDARD_NO_PAD.encode(secret));
        self.variants
            .push(general_purpose::URL_SAFE_NO_PAD.encode(secret));
        for percent in [false, true] {
            let encoded = secret.bytes().fold(
                String::with_capacity(secret.len() * 3),
                |mut encoded, byte| {
                    if percent {
                        encoded.push('%');
                    }
                    let _written = write!(encoded, "{byte:02x}");
                    encoded
                },
            );
            self.variants.push(encoded);
        }
        if let Ok(escaped) = serde_json::to_string(secret) {
            let escaped = escaped.trim_matches('"');
            if escaped.len() >= 8 {
                self.variants.push(escaped.to_owned());
            }
        }
    }

    /// # Errors
    /// Rejects recognizable credentials or handled secrets in publication text.
    pub fn check(&self, output: &str) -> Result<()> {
        let lower = output.to_ascii_lowercase();
        if self
            .variants
            .iter()
            .any(|variant| lower.contains(&variant.to_ascii_lowercase()))
            || [
                "ghp_",
                "ghs_",
                "gho_",
                "ghu_",
                "ghr_",
                "github_pat_",
                "sk-ant-",
                "-----begin private key",
                "-----begin rsa private key",
                "authorization: bearer ",
            ]
            .iter()
            .any(|prefix| lower.contains(prefix))
        {
            return Err("credential detected; publication blocked");
        }
        Ok(())
    }
}

#[derive(Deserialize)]
struct ClaudeResult {
    #[serde(rename = "type")]
    message_type: String,
    subtype: String,
    is_error: bool,
    result: String,
    structured_output: Option<serde_json::Value>,
    #[serde(rename = "modelUsage")]
    model_usage: std::collections::BTreeMap<String, serde_json::Value>,
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AgentDisposition {
    Completed,
    Blocked,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WritableAnswer {
    disposition: AgentDisposition,
    message: String,
}

pub const WRITABLE_RESULT_SCHEMA: &str = r#"{"type":"object","properties":{"disposition":{"type":"string","enum":["completed","blocked"],"description":"Use blocked if policy, permissions or another obstacle prevented fulfilling the task; otherwise completed, including ordinary answers or nothing to do."},"message":{"type":"string","minLength":1,"maxLength":48000}},"required":["disposition","message"],"additionalProperties":false}"#;

#[derive(Debug, PartialEq, Eq)]
pub struct AgentOutput {
    pub text: String,
    pub model: String,
    pub disposition: Option<AgentDisposition>,
}

/// # Errors
/// Rejects unsuccessful, malformed, oversized, empty, or credential-bearing results.
pub fn agent_result(stdout: &[u8], stderr: &[u8], guard: &OutputGuard) -> Result<AgentOutput> {
    parse_agent_result(stdout, stderr, guard, false)
}

/// # Errors
/// Requires a successful, strictly typed writable result; never infers status from prose.
pub fn writable_agent_result(
    stdout: &[u8],
    stderr: &[u8],
    guard: &OutputGuard,
) -> Result<AgentOutput> {
    parse_agent_result(stdout, stderr, guard, true)
}

fn parse_agent_result(
    stdout: &[u8],
    stderr: &[u8],
    guard: &OutputGuard,
    writable: bool,
) -> Result<AgentOutput> {
    checked_output(stdout, stderr, guard)?;
    let mut result = None;
    for (index, value) in serde_json::Deserializer::from_slice(stdout)
        .into_iter::<serde_json::Value>()
        .enumerate()
    {
        if index >= 4096 {
            return Err("too many harness records");
        }
        let value = value.map_err(|_| "invalid harness result")?;
        if value.get("type").and_then(serde_json::Value::as_str) == Some("result") {
            if result.is_some() {
                return Err("duplicate harness result");
            }
            result = Some(value);
        }
    }
    let value = result.ok_or("missing harness result")?;
    completed_agent_result(value.to_string().as_bytes(), stderr, guard, writable)
}

fn checked_output(stdout: &[u8], stderr: &[u8], guard: &OutputGuard) -> Result<()> {
    if stdout.len() > OUTPUT_BYTES_MAX || stderr.len() > OUTPUT_BYTES_MAX {
        return Err("harness output exceeds limit");
    }
    guard.check(std::str::from_utf8(stdout).map_err(|_| "invalid harness output")?)?;
    guard.check(std::str::from_utf8(stderr).map_err(|_| "invalid harness diagnostics")?)
}

/// # Errors
/// Rejects unsafe output; only complete observed records contribute model attribution.
pub fn partial_agent_result(
    stdout: &[u8],
    stderr: &[u8],
    guard: &OutputGuard,
) -> Result<AgentOutput> {
    checked_output(stdout, stderr, guard)?;
    let mut models = std::collections::BTreeSet::new();
    let mut text = String::new();
    let mut terminal_seen = false;
    let mut completed_terminal = false;
    for (index, value) in serde_json::Deserializer::from_slice(stdout)
        .into_iter::<serde_json::Value>()
        .enumerate()
    {
        if index >= 4096 {
            return Err("too many harness records");
        }
        let value = match value {
            Ok(value) => value,
            Err(error) if error.is_eof() => break,
            Err(_) => return Err("invalid partial harness output"),
        };
        if value.get("type").and_then(serde_json::Value::as_str) == Some("result") {
            if terminal_seen {
                return Err("duplicate harness result");
            }
            terminal_seen = true;
            // A completed or structured record must satisfy the writable protocol,
            // even after timeout. Budget errors without one remain partial work.
            let partial_error = value.get("is_error").and_then(serde_json::Value::as_bool)
                == Some(true)
                && matches!(
                    value.get("subtype").and_then(serde_json::Value::as_str),
                    Some(
                        "error_during_execution"
                            | "error_max_turns"
                            | "error_max_budget_usd"
                            | "error_max_structured_output_retries"
                    )
                )
                && value.get("structured_output").is_none();
            completed_terminal = !partial_error;
        }
        if let Some(usage) = value
            .get("modelUsage")
            .and_then(serde_json::Value::as_object)
        {
            models.extend(usage.keys().cloned());
        }
        if value.get("type").and_then(serde_json::Value::as_str) == Some("assistant") {
            if let Some(model) = value
                .pointer("/message/model")
                .and_then(serde_json::Value::as_str)
            {
                models.insert(model.to_owned());
            }
            if let Some(content) = value
                .pointer("/message/content")
                .and_then(serde_json::Value::as_array)
            {
                for block in content {
                    if block.get("type").and_then(serde_json::Value::as_str) == Some("text")
                        && let Some(part) = block.get("text").and_then(serde_json::Value::as_str)
                    {
                        text.push_str(part);
                    }
                }
            }
        }
        if let Some(result) = value.get("result").and_then(serde_json::Value::as_str) {
            result.clone_into(&mut text);
        }
        if text.len() > 48_000 || models.len() > 8 {
            return Err("partial result exceeds limit");
        }
    }
    if completed_terminal {
        return writable_agent_result(stdout, stderr, guard);
    }
    if models.iter().any(|model| !valid_model(model)) {
        return Err("invalid observed model");
    }
    if text.trim().is_empty() {
        text = "Budget exhausted before a final answer was observed.".into();
    }
    guard.check(&text)?;
    Ok(AgentOutput {
        text,
        disposition: None,
        model: if models.is_empty() {
            "unobserved".into()
        } else {
            models.into_iter().collect::<Vec<_>>().join(",")
        },
    })
}

fn valid_model(model: &str) -> bool {
    !model.is_empty()
        && model != "unobserved"
        && model.len() <= 128
        && model
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"-._".contains(&byte))
}

fn completed_agent_result(
    stdout: &[u8],
    stderr: &[u8],
    guard: &OutputGuard,
    writable: bool,
) -> Result<AgentOutput> {
    let stdout = std::str::from_utf8(stdout).map_err(|_| "invalid harness output")?;
    let stderr = std::str::from_utf8(stderr).map_err(|_| "invalid harness diagnostics")?;
    guard.check(stdout)?;
    guard.check(stderr)?;
    let result: ClaudeResult =
        serde_json::from_str(stdout).map_err(|_| "invalid harness result")?;
    if result.message_type != "result" || result.subtype != "success" || result.is_error {
        return Err("harness did not complete successfully");
    }
    let (text, disposition) = if writable {
        let answer: WritableAnswer =
            serde_json::from_value(result.structured_output.ok_or("missing writable result")?)
                .map_err(|_| "invalid writable result")?;
        (answer.message, Some(answer.disposition))
    } else {
        (result.result, None)
    };
    if text.trim().is_empty() || text.len() > 48_000 {
        return Err("empty or oversized reply");
    }
    if result.model_usage.is_empty()
        || result.model_usage.len() > 8
        || result.model_usage.keys().any(|model| !valid_model(model))
    {
        return Err("missing or invalid observed model usage");
    }
    let model = result
        .model_usage
        .keys()
        .cloned()
        .collect::<Vec<_>>()
        .join(",");
    guard.check(&text)?;
    guard.check(&model)?;
    Ok(AgentOutput {
        text,
        model,
        disposition,
    })
}

#[cfg(test)]
// Test assertions deliberately panic on malformed fixtures.
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use super::*;

    fn request_json() -> String {
        serde_json::json!({
            "version": 1, "delivery_id": "12345678-1234-1234-1234-123456789abc",
            "repository_id": 123, "repository": "isp-insoft-gmbh/isp-golem-xiv-test-example",
            "head_sha": "a".repeat(40), "golem": "hello", "issue_number": 1,
            "comment_id": 123, "actor_id": 123, "actor_login": "test-member",
            "seat_login": "test-member", "provider": "anthropic", "harness": "claude",
            "model": null, "timeout_seconds": 1800, "instructions": "say this exact text", "check_run_id": 123
        }).to_string()
    }

    fn parse(json: &str) -> Result<DispatchV1> {
        DispatchV1::parse(json, "isp-insoft-gmbh/isp-golem-xiv-test-example", "hello")
    }

    #[test]
    fn only_the_first_workflow_attempt_can_reach_agent_access() {
        assert!(initial_workflow_attempt("1").is_ok());
        for rerun in ["0", "2", "01", ""] {
            assert!(initial_workflow_attempt(rerun).is_err(), "{rerun}");
        }
    }

    #[test]
    fn dispatch_binds_repository_seat_and_supported_slice() {
        let json = request_json();
        let request = parse(&json).unwrap();
        assert_eq!(request.seat_secret_name(), "SEAT_TEST_MEMBER__ANTHROPIC");
        assert!(DispatchV1::parse(&json, "other/repository", "hello").is_err());
        assert!(DispatchV1::parse(&json, &request.repository, "other").is_err());
        for (field, value) in [
            ("version", serde_json::json!(2)),
            ("repository", serde_json::json!("other/production")),
            ("seat_login", serde_json::json!("another-member")),
            ("head_sha", serde_json::json!("main")),
            ("delivery_id", serde_json::json!("../bad")),
            ("issue_number", serde_json::json!(0)),
            ("timeout_seconds", serde_json::json!(18001)),
            ("timeout_seconds", serde_json::json!(0)),
            ("harness", serde_json::json!("pi")),
            ("provider", serde_json::json!("openai-codex")),
            ("model", serde_json::json!("--injected")),
            ("unexpected", serde_json::json!(true)),
        ] {
            let mut modified: serde_json::Value = serde_json::from_str(&json).unwrap();
            modified[field] = value;
            assert!(parse(&modified.to_string()).is_err(), "{field}");
        }
        assert!(parse(&" ".repeat(REQUEST_BYTES_MAX + 1)).is_err());
    }

    #[test]
    fn dispatch_parser_enforces_repository_instruction_golem_and_login_boundaries() {
        let original: serde_json::Value = serde_json::from_str(&request_json()).unwrap();
        let parse_modified = |value: &serde_json::Value| {
            DispatchV1::parse(
                &value.to_string(),
                value["repository"].as_str().unwrap(),
                value["golem"].as_str().unwrap(),
            )
        };
        for (length, accepted) in [(100, true), (101, false)] {
            let mut value = original.clone();
            value["repository"] =
                serde_json::json!(format!("isp-insoft-gmbh/{}", "x".repeat(length)));
            assert_eq!(
                parse_modified(&value).is_ok(),
                accepted,
                "repository name length {length}"
            );
        }
        for (instructions, accepted) in [
            ("x".repeat(16_384), true),
            ("x".repeat(16_385), false),
            ("é".repeat(8_192), true),
            (format!("{}x", "é".repeat(8_192)), false),
        ] {
            let mut value = original.clone();
            value["instructions"] = serde_json::json!(instructions);
            assert_eq!(
                parse_modified(&value).is_ok(),
                accepted,
                "instruction byte boundary"
            );
        }
        for (golem, accepted) in [
            ("lower_09-name", true),
            ("_", true),
            ("-hello", true),
            ("Upper", false),
            ("has.dot", false),
            ("../evil", false),
            ("", false),
            ("help", false),
            ("_golems", false),
        ] {
            let mut value = original.clone();
            value["golem"] = serde_json::json!(golem);
            assert_eq!(parse_modified(&value).is_ok(), accepted, "golem {golem}");
        }
        for (login, accepted) in [
            ("test-member", true),
            ("test--member", false),
            ("-member", false),
            ("member-", false),
            ("test_member", false),
        ] {
            let mut value = original.clone();
            value["actor_login"] = serde_json::json!(login);
            value["seat_login"] = serde_json::json!(login);
            assert_eq!(parse_modified(&value).is_ok(), accepted, "login {login}");
        }
        let mut value = original;
        value["repository"] = serde_json::json!("isp-insoft-gmbh/production_repository.with.dots");
        assert!(parse_modified(&value).is_ok());
        value["repository"] = serde_json::json!("isp-insoft-gmbh/not/a-repository");
        assert!(parse_modified(&value).is_err());
    }

    #[test]
    fn reply_and_audit_keep_contract_markers_and_attribution() {
        let request = parse(&request_json()).unwrap();
        assert_eq!(
            request.reply("actual agent answer"),
            "<!-- isp-golem-xiv kind=reply golem=hello -->\n\nactual agent answer"
        );
        assert_eq!(
            request.summary("replied", "claude-sonnet-4-6", "99"),
            "outcome: replied\nseat: test-member/anthropic\nharness: claude\nmodel: claude-sonnet-4-6\nbudget: 1800s\nlog: https://github.com/isp-insoft-gmbh/isp-golem-xiv-ops/actions/runs/99"
        );
    }

    #[test]
    fn version_two_context_definition_and_owner_mapping_are_bound() {
        let mut value: serde_json::Value = serde_json::from_str(&request_json()).unwrap();
        value["version"] = serde_json::json!(2);
        value["trigger"] = serde_json::json!("mention");
        value["branch"] = serde_json::json!("golems/hello");
        value["queue_generation"] = serde_json::json!("12345678-1234-1234-1234-123456789abc");
        let request = parse(&value.to_string()).unwrap();
        assert!(request.human());
        assert_eq!(request.branch_name(), "golems/hello");
        assert!(request.validate_definition("---\n---\nTask").is_ok());
        assert!(
            request
                .validate_definition("---\nverify: cargo test\n---\nTask")
                .is_err()
        );
        for branch in [
            "",
            "../bad",
            "a//b",
            "/main",
            "main.lock",
            "a/.b",
            "a@{b",
            "a b",
        ] {
            let mut bad = value.clone();
            bad["branch"] = serde_json::json!(branch);
            assert!(parse(&bad.to_string()).is_err(), "{branch}");
        }
        value["trigger"] = serde_json::json!("schedule");
        for field in ["issue_number", "comment_id", "actor_id", "actor_login"] {
            value[field] = serde_json::Value::Null;
        }
        value["owner"] = serde_json::json!("test-member");
        let request = parse(&value.to_string()).unwrap();
        assert!(!request.human());
        assert!(
            request
                .validate_definition("---\nowner: test-member\nschedule: daily\n---\nTask")
                .is_ok()
        );
        value["seat_login"] = serde_json::json!("another-member");
        assert!(parse(&value.to_string()).is_err());
        let mut legacy: serde_json::Value = serde_json::from_str(&request_json()).unwrap();
        for field in ["trigger", "branch", "verify", "owner", "queue_generation"] {
            legacy[field] = serde_json::Value::Null;
            assert!(parse(&legacy.to_string()).is_err());
            legacy.as_object_mut().unwrap().remove(field);
        }
    }

    #[test]
    fn writable_results_require_strict_disposition_without_prose_inference() {
        let guard = OutputGuard::default();
        let mut value = serde_json::json!({"type":"result", "subtype":"success", "is_error":false,
            "result":"", "modelUsage":{"observed-model":{}},
            "structured_output":{"disposition":"blocked", "message":"Cannot perform this task."}});
        let parse = |value: &serde_json::Value| {
            writable_agent_result(value.to_string().as_bytes(), b"", &guard)
        };
        let blocked = parse(&value).unwrap();
        assert_eq!(blocked.disposition, Some(AgentDisposition::Blocked));
        assert_eq!(blocked.model, "observed-model");
        value["structured_output"] = serde_json::json!({"disposition":"completed", "message":"I refuse to change anything."});
        assert_eq!(
            parse(&value).unwrap().disposition,
            Some(AgentDisposition::Completed)
        );
        for invalid in [
            serde_json::Value::Null,
            serde_json::json!({"disposition":"unknown", "message":"answer"}),
            serde_json::json!({"disposition":"ghp_secret", "message":"answer"}),
            serde_json::json!({"message":"answer"}),
            serde_json::json!({"disposition":"blocked"}),
            serde_json::json!({"disposition":"blocked", "message":" "}),
            serde_json::json!({"disposition":"blocked", "message":"x".repeat(48_001)}),
            serde_json::json!({"disposition":"blocked", "message":"answer", "extra":true}),
            serde_json::json!({"disposition":true, "message":"answer"}),
        ] {
            value["structured_output"] = invalid;
            assert!(parse(&value).is_err());
            assert!(partial_agent_result(value.to_string().as_bytes(), b"", &guard).is_err());
        }
        value["structured_output"] =
            serde_json::json!({"disposition":"completed", "message":"done"});
        for (field, invalid) in [
            ("is_error", serde_json::json!(true)),
            ("subtype", serde_json::json!("unknown")),
            ("modelUsage", serde_json::json!({})),
        ] {
            let mut bad = value.clone();
            bad[field] = invalid;
            assert!(parse(&bad).is_err());
            assert!(partial_agent_result(bad.to_string().as_bytes(), b"", &guard).is_err());
        }
        value.as_object_mut().unwrap().remove("structured_output");
        value["result"] = serde_json::json!("legacy answer");
        assert!(parse(&value).is_err());
        assert!(agent_result(value.to_string().as_bytes(), b"", &guard).is_ok());
        assert!(writable_agent_result(&vec![b' '; OUTPUT_BYTES_MAX + 1], b"", &guard).is_err());
    }

    #[test]
    fn partial_results_reject_duplicate_terminals_in_every_order() {
        let error = r#"{"type":"result","subtype":"error_max_turns","is_error":true,"modelUsage":{"observed":{}}}"#;
        let success = r#"{"type":"result","subtype":"success","is_error":false,"result":"","structured_output":{"disposition":"completed","message":"done"},"modelUsage":{"observed":{}}}"#;
        for (first, second) in [
            (error, error),
            (error, success),
            (success, error),
            (success, success),
        ] {
            let stream = format!("{first}\n{second}");
            assert_eq!(
                partial_agent_result(stream.as_bytes(), b"", &OutputGuard::default()),
                Err("duplicate harness result")
            );
        }
    }

    #[test]
    fn streamed_success_and_partial_results_only_attribute_observed_models() {
        let guard = OutputGuard::default();
        let assistant = r#"{"type":"assistant","message":{"model":"claude-observed","content":[{"type":"text","text":"partial work"}]}}"#;
        let success = r#"{"type":"result","subtype":"success","is_error":false,"result":"done","structured_output":{"disposition":"completed","message":"done"},"modelUsage":{"claude-observed":{}}}"#;
        let stream = format!("{assistant}\n{success}\n");
        assert_eq!(
            agent_result(stream.as_bytes(), b"", &guard).unwrap().text,
            "done"
        );
        assert_eq!(
            partial_agent_result(stream.as_bytes(), b"", &guard).unwrap(),
            writable_agent_result(success.as_bytes(), b"", &guard).unwrap()
        );
        let partial = format!("{assistant}\n{{\"type\":");
        let output = partial_agent_result(partial.as_bytes(), b"", &guard).unwrap();
        assert_eq!(output.text, "partial work");
        assert_eq!(output.model, "claude-observed");
        for subtype in [
            "error_during_execution",
            "error_max_turns",
            "error_max_budget_usd",
            "error_max_structured_output_retries",
        ] {
            let budget_error = format!(
                "{assistant}\n{{\"type\":\"result\",\"subtype\":\"{subtype}\",\"is_error\":true,\"modelUsage\":{{\"claude-observed\":{{}}}}}}"
            );
            let output = partial_agent_result(budget_error.as_bytes(), b"", &guard).unwrap();
            assert_eq!(output.text, "partial work");
            assert_eq!(output.model, "claude-observed");
            assert_eq!(output.disposition, None);
        }
        assert_eq!(
            partial_agent_result(b"", b"", &guard).unwrap().model,
            "unobserved"
        );
        assert!(agent_result(b"", b"", &guard).is_err());
        assert!(partial_agent_result(b"not json", b"", &guard).is_err());
        assert!(partial_agent_result(b"", b"ghp_secret", &guard).is_err());
        assert!(agent_result(&vec![b' '; OUTPUT_BYTES_MAX + 1], b"", &guard).is_err());
    }

    #[test]
    fn credentials_in_diagnostics_or_encoded_results_block_publication() {
        let secret = "sensitive-test-secret";
        let mut guard = OutputGuard::default();
        guard.add(secret);
        for leaked in [
            secret.to_owned(),
            general_purpose::STANDARD.encode(secret),
            "73656e7369746976652d746573742d736563726574".to_owned(),
            "%73%65%6e%73%69%74%69%76%65%2d%74%65%73%74%2d%73%65%63%72%65%74".to_owned(),
            "github_pat_never-publish".to_owned(),
        ] {
            assert!(guard.check(&leaked).is_err());
        }
        let successful = br#"{"type":"result","subtype":"success","is_error":false,"result":"answer from repo","modelUsage":{"claude-sonnet-4-6":{}}}"#;
        assert_eq!(
            agent_result(successful, b"", &guard).unwrap().text,
            "answer from repo"
        );
        assert!(agent_result(successful, secret.as_bytes(), &guard).is_err());
        for invalid in [
            r#"{"type":"result","subtype":"success","is_error":true,"result":"answer","modelUsage":{"claude-sonnet-4-6":{}}}"#,
            r#"{"type":"result","subtype":"error_max_turns","is_error":false,"result":"answer","modelUsage":{"claude-sonnet-4-6":{}}}"#,
            r#"{"type":"result","subtype":"success","is_error":false,"result":"","modelUsage":{"claude-sonnet-4-6":{}}}"#,
            r#"{"type":"result","subtype":"success","is_error":false,"result":"answer","modelUsage":{}}"#,
            r#"{"type":"result","subtype":"success","is_error":false,"result":"answer","modelUsage":{"bad\nmodel":{}}}"#,
            "not json",
        ] {
            assert!(agent_result(invalid.as_bytes(), b"", &guard).is_err());
        }
    }
}
