#![forbid(unsafe_code)]

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
    pub issue_number: u64,
    pub comment_id: u64,
    pub actor_id: u64,
    pub actor_login: String,
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
        if request.version != 1
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
            || !identifier(&request.actor_login, 39)
            || !identifier(&request.seat_login, 39)
            || !request
                .actor_login
                .eq_ignore_ascii_case(&request.seat_login)
            || request.provider != "anthropic"
            || request.harness != "claude"
            || !(1..=18_000).contains(&request.timeout_seconds)
            || request.instructions.len() > 16_384
            || request.instructions.contains('\0')
            || [
                request.repository_id,
                request.issue_number,
                request.comment_id,
                request.actor_id,
                request.check_run_id,
            ]
            .contains(&0)
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
    #[serde(rename = "modelUsage")]
    model_usage: std::collections::BTreeMap<String, serde_json::Value>,
}

#[derive(Debug, PartialEq, Eq)]
pub struct AgentOutput {
    pub text: String,
    pub model: String,
}

/// # Errors
/// Rejects unsuccessful, malformed, oversized, empty, or credential-bearing results.
pub fn agent_result(stdout: &[u8], stderr: &[u8], guard: &OutputGuard) -> Result<AgentOutput> {
    let stdout = std::str::from_utf8(stdout).map_err(|_| "invalid harness output")?;
    let stderr = std::str::from_utf8(stderr).map_err(|_| "invalid harness diagnostics")?;
    guard.check(stdout)?;
    guard.check(stderr)?;
    let result: ClaudeResult =
        serde_json::from_str(stdout).map_err(|_| "invalid harness result")?;
    if result.message_type != "result" || result.subtype != "success" || result.is_error {
        return Err("harness did not complete successfully");
    }
    if result.result.trim().is_empty() || result.result.len() > 48_000 {
        return Err("empty or oversized reply");
    }
    if result.model_usage.is_empty()
        || result.model_usage.len() > 8
        || result.model_usage.keys().any(|model| {
            model.is_empty()
                || model.len() > 128
                || !model
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || b"-._".contains(&byte))
        })
    {
        return Err("missing or invalid observed model usage");
    }
    let model = result
        .model_usage
        .keys()
        .cloned()
        .collect::<Vec<_>>()
        .join(",");
    guard.check(&result.result)?;
    Ok(AgentOutput {
        text: result.result,
        model,
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
