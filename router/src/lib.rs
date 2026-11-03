//! Slice 1 routing policy, independent of the Workers runtime.
use hmac::{Hmac, KeyInit, Mac};
use serde::{Deserialize, Serialize};
use sha2::Sha256;

#[cfg(target_arch = "wasm32")]
mod edge;

pub const ORGANIZATION: &str = "isp-insoft-gmbh";
pub const APP: &str = "isp-golem-xiv";
pub const BOT: &str = "isp-golem-xiv[bot]";
pub const OPS: &str = "isp-insoft-gmbh/isp-golem-xiv-ops";
pub const TEST_TOPIC: &str = "isp-golem-xiv-test";
pub const SEAT_URL: &str = "https://isp-golem-xiv.isp-insoft-gmbh.workers.dev/seats";
pub const FILE_BYTES_MAX: usize = 65_536;
pub const COMMENT_BYTES_MAX: usize = 16_384;
pub const GOLEMS_MAX: usize = 20;

#[must_use]
pub fn valid_login(login: &str) -> bool {
    !login.is_empty()
        && login.len() <= 39
        && !login.starts_with('-')
        && !login.ends_with('-')
        && !login.contains("--")
        && login
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-')
}

#[must_use]
pub fn valid_repository(repository: &str) -> bool {
    let Some((owner, name)) = repository.split_once('/') else {
        return false;
    };
    owner == ORGANIZATION
        && !name.is_empty()
        && name.len() <= 100
        && !matches!(name, "." | "..")
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-_.".contains(&b))
}

#[must_use]
pub fn valid_delivery(delivery: &str) -> bool {
    delivery.len() == 36
        && delivery.bytes().enumerate().all(|(index, byte)| {
            if [8, 13, 18, 23].contains(&index) {
                byte == b'-'
            } else {
                byte.is_ascii_hexdigit()
            }
        })
}

#[must_use]
pub fn valid_golem_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 64
        && name
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-' || b == b'_')
        && !matches!(name, "stop" | "seat" | "help" | "_golems")
}

#[derive(Debug, PartialEq, Eq)]
pub struct Mention<'a> {
    pub golem: &'a str,
    pub instructions: &'a str,
}

/// Only the first line's command prefix has authority; subsequent text is context.
#[must_use]
pub fn parse_mention(body: &str) -> Option<Mention<'_>> {
    if body.len() > COMMENT_BYTES_MAX || body.contains('\0') {
        return None;
    }
    let first = body.lines().next()?;
    let remainder = first.strip_prefix("@isp-golem-xiv")?;
    if !remainder.is_empty() && !remainder.starts_with(char::is_whitespace) {
        return None;
    }
    let command = remainder.trim_start();
    let end = command.find(char::is_whitespace).unwrap_or(command.len());
    let golem = command.get(..end)?;
    // Locate the token in the original body, not the first-line view, so context
    // retains its exact line endings, indentation, blank lines, and trailing text.
    let after_golem = first.len() - command.len() + end;
    let instructions = body
        .get(after_golem..)?
        .trim_start_matches(|character: char| {
            character.is_whitespace() && !matches!(character, '\r' | '\n')
        });
    Some(Mention {
        golem,
        instructions,
    })
}

#[must_use]
pub fn verify_signature(secret: &[u8], signature: &str, body: &[u8]) -> bool {
    let Some(hex) = signature.strip_prefix("sha256=") else {
        return false;
    };
    if hex.len() != 64 {
        return false;
    }
    let mut bytes = Vec::with_capacity(32);
    for &[a, b] in hex.as_bytes().as_chunks::<2>().0 {
        let (Some(a), Some(b)) = (char::from(a).to_digit(16), char::from(b).to_digit(16)) else {
            return false;
        };
        let Ok(byte) = u8::try_from(a * 16 + b) else {
            return false;
        };
        bytes.push(byte);
    }
    let Ok(mut mac) = Hmac::<Sha256>::new_from_slice(secret) else {
        return false;
    };
    mac.update(body);
    mac.verify_slice(&bytes).is_ok()
}

#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Frontmatter {
    pub triggers: Vec<String>,
    pub schedule: Option<Schedule>,
    pub owner: Option<String>,
    pub harness: Option<String>,
    pub model: Option<String>,
    pub effort: Option<String>,
    pub provider: Option<String>,
    pub branch: Option<String>,
    pub timeout: Option<String>,
    pub verify: Option<String>,
    pub runner: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum Schedule {
    Daily(String),
    Days(Vec<String>),
}

#[derive(Debug, Serialize)]
pub struct Golem {
    pub name: String,
    pub errors: Vec<String>,
    #[serde(skip)]
    pub frontmatter: Frontmatter,
}

fn timeout_seconds(value: Option<&str>) -> Option<u32> {
    let Some(value) = value else {
        return Some(1800);
    };
    let (amount, multiplier) = if let Some(amount) = value.strip_suffix('m') {
        (amount, 60)
    } else if let Some(amount) = value.strip_suffix('h') {
        (amount, 3600)
    } else {
        (value.strip_suffix('s')?, 1)
    };
    if !amount.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let seconds = amount.parse::<u32>().ok()?.checked_mul(multiplier)?;
    (1..=18_000).contains(&seconds).then_some(seconds)
}

// Git ref suffix rules are case-sensitive, unlike Windows filename extensions.
#[allow(clippy::case_sensitive_file_extension_comparisons)]
fn valid_branch(branch: &str) -> bool {
    !branch.is_empty()
        && branch.len() <= 200
        && branch != "@"
        && !branch.starts_with('/')
        && !branch.ends_with('/')
        && !branch.contains("..")
        && !branch.contains("@{")
        && branch
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"/_-.".contains(&b))
        && branch.split('/').all(|part| {
            !part.is_empty()
                && !part.starts_with('.')
                && !part.ends_with('.')
                && !part.ends_with(".lock")
        })
}

fn validate_frontmatter(front: &Frontmatter) -> Vec<String> {
    let mut errors = Vec::new();
    if (!front.triggers.is_empty() || front.schedule.is_some()) && front.owner.is_none() {
        errors.push("owner required with triggers or schedule".into());
    }
    if front
        .owner
        .as_deref()
        .is_some_and(|owner| !valid_login(owner))
    {
        errors.push("invalid owner".into());
    }
    if front.triggers.len() > 32
        || front.triggers.iter().any(|trigger| {
            let mut parts = trigger.split('.');
            let event = parts.next().unwrap_or_default();
            !matches!(
                event,
                "issues" | "issue_comment" | "pull_request" | "pull_request_review" | "push"
            ) || parts.next().is_some_and(|action| {
                action.is_empty()
                    || action.len() > 40
                    || !action.bytes().all(|b| b.is_ascii_lowercase() || b == b'_')
            }) || parts.next().is_some()
        })
    {
        errors.push("invalid triggers".into());
    }
    if let Some(schedule) = &front.schedule {
        let valid = match schedule {
            Schedule::Daily(value) => value == "daily",
            Schedule::Days(days) => {
                !days.is_empty()
                    && days.len() <= 7
                    && days.iter().all(|day| {
                        matches!(
                            day.as_str(),
                            "mon" | "tue" | "wed" | "thu" | "fri" | "sat" | "sun"
                        )
                    })
                    && days
                        .iter()
                        .enumerate()
                        .all(|(index, day)| !days.iter().take(index).any(|prior| prior == day))
            }
        };
        if !valid {
            errors.push("invalid schedule".into());
        }
    }
    let harness = front.harness.as_deref().unwrap_or("claude");
    let provider = front.provider.as_deref().unwrap_or(match harness {
        "codex" => "openai-codex",
        _ => "anthropic",
    });
    if !matches!(
        (harness, provider),
        ("claude" | "pi", "anthropic")
            | ("codex" | "pi", "openai-codex")
            | ("pi", "github-copilot")
    ) {
        errors.push("invalid or incompatible harness/provider".into());
    }
    if harness == "claude"
        && front.model.as_deref().is_some_and(|model| {
            model.len() > 128
                || model.starts_with('-')
                || !model
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || b"-._".contains(&byte))
        })
    {
        errors.push("unsupported Claude model identifier".into());
    }
    if timeout_seconds(front.timeout.as_deref()).is_none() {
        errors.push("timeout must be positive and at most 5h".into());
    }
    if front
        .branch
        .as_deref()
        .is_some_and(|branch| !valid_branch(branch))
    {
        errors.push("invalid branch".into());
    }
    for (label, value, limit) in [
        ("model", &front.model, 200),
        ("effort", &front.effort, 40),
        ("verify", &front.verify, 4096),
        ("runner", &front.runner, 100),
    ] {
        if value.as_deref().is_some_and(|value| {
            value.trim().is_empty() || value.len() > limit || value.chars().any(char::is_control)
        }) {
            errors.push(format!("invalid {label}"));
        }
    }
    errors
}

/// Invalid files remain visible in the index, but can never be dispatched.
#[must_use]
pub fn parse_golem(name: &str, document: &str) -> Golem {
    let mut golem = Golem {
        name: name.into(),
        errors: Vec::new(),
        frontmatter: Frontmatter::default(),
    };
    if !valid_golem_name(name) {
        golem.errors.push("invalid or reserved golem name".into());
    }
    if document.len() > FILE_BYTES_MAX {
        golem.errors.push("golem exceeds 64 KiB".into());
        return golem;
    }
    let normalized = document.replace("\r\n", "\n");
    let Some(rest) = normalized.strip_prefix("---\n") else {
        golem
            .errors
            .push("missing opening frontmatter delimiter".into());
        return golem;
    };
    let Some((yaml, task)) = rest
        .strip_prefix("---\n")
        .map(|task| ("", task))
        .or_else(|| rest.split_once("\n---\n"))
    else {
        golem
            .errors
            .push("missing closing frontmatter delimiter".into());
        return golem;
    };
    if task.trim().is_empty() {
        golem.errors.push("task must not be empty".into());
    }
    let options = serde_saphyr::options! {
        no_schema: true,
        budget: serde_saphyr::budget! {
            max_depth: 8, max_events: 1024, max_nodes: 512,
            max_aliases: 0, max_anchors: 0, max_documents: 1,
            max_total_scalar_bytes: FILE_BYTES_MAX,
        },
        merge_keys: serde_saphyr::MergeKeyPolicy::Error,
    };
    let result = if yaml.trim().is_empty() {
        Ok(Frontmatter::default())
    } else {
        serde_saphyr::from_str_with_options(yaml, options)
    };
    match result {
        Ok(frontmatter) => { golem.errors.extend(validate_frontmatter(&frontmatter)); golem.frontmatter = frontmatter; }
        Err(_) => golem.errors.push("invalid frontmatter: unknown/duplicate field, wrong type, invalid YAML, or parser limit".into()),
    }
    golem
}

#[must_use]
pub fn seat_secret(login: &str, provider: &str) -> Option<String> {
    if !valid_login(login) || !matches!(provider, "anthropic" | "openai-codex" | "github-copilot") {
        return None;
    }
    Some(format!(
        "SEAT_{}__{}",
        login.replace('-', "_").to_ascii_uppercase(),
        provider.replace('-', "_").to_ascii_uppercase()
    ))
}

#[derive(Debug, Deserialize)]
pub struct User {
    pub id: u64,
    pub login: String,
    #[serde(rename = "type")]
    pub kind: String,
}
#[derive(Debug, Deserialize)]
pub struct Repository {
    pub id: u64,
    pub full_name: String,
}
#[derive(Debug, Deserialize)]
pub struct Comment {
    pub id: u64,
    pub body: String,
    pub user: User,
}
#[derive(Debug, Deserialize)]
pub struct Issue {
    pub number: u64,
}
#[derive(Debug, Deserialize)]
pub struct Installation {
    pub id: u64,
}
#[derive(Debug, Deserialize)]
pub struct Delivery {
    pub action: String,
    pub sender: User,
    pub repository: Repository,
    pub comment: Comment,
    pub issue: Issue,
    pub installation: Installation,
}

impl Delivery {
    #[must_use]
    pub fn accepted(&self) -> bool {
        self.action == "created"
            && self.sender.login != BOT
            && self.comment.user.login != BOT
            && self.sender.kind == "User"
            && self.comment.user.kind == "User"
            && self.sender.id > 0
            && self.sender.id == self.comment.user.id
            && self.sender.login == self.comment.user.login
            && valid_login(&self.sender.login)
            && self.repository.id > 0
            && valid_repository(&self.repository.full_name)
            && self.installation.id == 153_964_219
            && self.issue.number > 0
            && self.comment.id > 0
            && parse_mention(&self.comment.body).is_some()
    }
}

#[derive(Debug, Serialize)]
pub struct DispatchV1<'a> {
    pub version: u8,
    pub delivery_id: &'a str,
    pub repository_id: u64,
    pub repository: &'a str,
    pub head_sha: &'a str,
    pub golem: &'a str,
    pub issue_number: u64,
    pub comment_id: u64,
    pub actor_id: u64,
    pub actor_login: &'a str,
    pub seat_login: &'a str,
    pub provider: &'a str,
    pub harness: &'a str,
    pub model: Option<&'a str>,
    pub timeout_seconds: u32,
    pub instructions: &'a str,
    pub check_run_id: u64,
}

#[must_use]
pub fn refusal(reason: &str, golems: &[Golem]) -> String {
    let explanation = match reason {
        "not-member" => format!(
            "Only {ORGANIZATION} members may start golems. Ask an organization admin for membership."
        ),
        "no-seat" => format!(
            "No supported seat registered for you. Register your Anthropic seat: {SEAT_URL}"
        ),
        _ => format!(
            "Unknown or disabled golem. Choose an enabled golem: {}.",
            golems
                .iter()
                .filter(|golem| golem.errors.is_empty())
                .map(|golem| golem.name.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        ),
    };
    format!("<!-- isp-golem-xiv kind=refused reason={reason} -->\n{explanation}")
}

impl Golem {
    #[must_use]
    pub fn timeout_seconds(&self) -> Option<u32> {
        timeout_seconds(self.frontmatter.timeout.as_deref())
    }
}
