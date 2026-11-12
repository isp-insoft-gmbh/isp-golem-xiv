//! Routing policy, independent of the Workers runtime.
use hmac::{Hmac, KeyInit, Mac};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[cfg(target_arch = "wasm32")]
mod edge;
pub mod registration;

pub const ORGANIZATION: &str = "isp-insoft-gmbh";
pub const APP: &str = "isp-golem-xiv";
pub const BOT: &str = "isp-golem-xiv[bot]";
pub const OPS: &str = "isp-insoft-gmbh/isp-golem-xiv-ops";
pub const TEST_TOPIC: &str = "isp-golem-xiv-test";
pub const SEAT_URL: &str = "https://isp-golem-xiv.isp-insoft-gmbh.workers.dev/seats";
pub const ROUTER_URL: &str = "https://isp-golem-xiv.isp-insoft-gmbh.workers.dev";
pub const OAUTH_CALLBACK: &str = "/auth/github/callback";
/// Public client id of the App's user authorization; the client secret stays a Worker secret.
pub const CLIENT_ID: &str = "Iv23liEnfLEVCSS2aUdR";

fn percent_encode(value: &str) -> String {
    value
        .bytes()
        .map(|b| {
            if b.is_ascii_alphanumeric() || b"-._~".contains(&b) {
                char::from(b).to_string()
            } else {
                format!("%{b:02X}")
            }
        })
        .collect()
}

/// GitHub sign-in URL; `state` must be a UUID bound to the browser by cookie.
#[must_use]
pub fn sign_in_location(state: &str) -> Option<String> {
    valid_delivery(state).then(|| {
        format!(
            "https://github.com/login/oauth/authorize?client_id={CLIENT_ID}&redirect_uri={}&state={state}",
            percent_encode(&format!("{ROUTER_URL}{OAUTH_CALLBACK}"))
        )
    })
}
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

/// Preserve signed event context as untrusted task data within dispatch bounds.
#[must_use]
pub fn event_instructions(event: &str, action: &str, comment: Option<&str>) -> Option<String> {
    if [event, action, comment.unwrap_or_default()]
        .iter()
        .any(|text| text.len() > COMMENT_BYTES_MAX || text.contains('\0'))
    {
        return None;
    }
    let mut instructions = format!("GitHub event: {event}.{action}");
    if let Some(body) = comment {
        instructions.push_str("\n\nTriggering comment:\n");
        instructions.push_str(body);
    }
    (instructions.len() <= COMMENT_BYTES_MAX).then_some(instructions)
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
    /// First task line for the dashboard; never part of the public index.
    #[serde(skip)]
    pub purpose: String,
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
    // Harness and provider are preferences (SPEC "Which seat and model"); an incompatible
    // pair is resolved at run time by falling back to `pi`, so only unknown values are invalid.
    let harness = front.harness.as_deref().unwrap_or("claude");
    let provider = front
        .provider
        .as_deref()
        .unwrap_or_else(|| default_provider(harness));
    if !matches!(harness, "claude" | "codex" | "pi") || !PROVIDERS.contains(&provider) {
        errors.push("unknown harness or provider".into());
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
        purpose: String::new(),
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
    golem.purpose = task
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or_default()
        .chars()
        .filter(|c| !c.is_control())
        .take(160)
        .collect();
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
    if !valid_login(login) || !PROVIDERS.contains(&provider) {
        return None;
    }
    Some(format!(
        "SEAT_{}__{}",
        login.replace('-', "_").to_ascii_uppercase(),
        provider.replace('-', "_").to_ascii_uppercase()
    ))
}

/// Ops repository variable holding a person's preferred seat provider.
#[must_use]
pub fn preferred_seat_variable(login: &str) -> Option<String> {
    valid_login(login).then(|| {
        format!(
            "SEAT_PREFERRED_{}",
            login.replace('-', "_").to_ascii_uppercase()
        )
    })
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct User {
    pub id: u64,
    pub login: String,
    #[serde(rename = "type")]
    pub kind: String,
}
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Repository {
    pub id: u64,
    pub full_name: String,
}
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Comment {
    pub id: u64,
    pub body: String,
    pub user: User,
}
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Issue {
    pub number: u64,
}
#[derive(Debug, Clone, Deserialize, Serialize)]
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

pub const ONBOARDING_TITLE: &str = "Enable golems";
pub const ONBOARDING_BRANCH: &str = "golems/enable";
pub const HOUSE_RULES: &str = "You are a golem working unattended in this repository.";
pub const DASHBOARD_TITLE: &str = "Golems";
pub const DASHBOARD_MARKER: &str = "<!-- isp-golem-xiv kind=dashboard -->";

/// Exactly the files of the onboarding pull request.
#[must_use]
pub fn onboarding_files() -> [(&'static str, String); 2] {
    [
        (".golems/_golems.md", format!("{HOUSE_RULES}\n")),
        (
            ".golems/hello.md",
            "---\n---\nLook around this repository and reply with what you see: its purpose, layout, toolchain, and how to build and test it.\nChange nothing in the repository.\n".into(),
        ),
    ]
}

/// Onboarding description: house rules, toolchain presence, and how to check your seat.
#[must_use]
pub fn onboarding_body(toolchain: Option<&str>) -> String {
    let toolchain = toolchain.map_or_else(
        || "No toolchain description found (`mise.toml`, `.mise.toml` or `.tool-versions`); golems then work with the base image's tools only.".to_owned(),
        |file| format!("Toolchain description found: `{file}`; golems prepare it before working."),
    );
    [
        "Merging this enables golems here; closing it leaves the repository alone until someone adds `.golems/`.".to_owned(),
        String::new(),
        "**House rules.** `.golems/_golems.md` holds rules every golem in this repository follows, like a briefing; edit it to describe how work is done here.".to_owned(),
        "`.golems/hello.md` is a first golem that changes nothing and reports what it sees.".to_owned(),
        String::new(),
        format!("**Toolchain.** {toolchain}"),
        String::new(),
        format!("**Seats.** Every run is paid by one person's seat. Check yours with `@isp-golem-xiv seat`; register one at {SEAT_URL}."),
    ]
    .join("\n")
        + "\n"
}

/// Seat holders derived from ops secret names; logins are matched to current members.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct SeatHolders {
    pub registered: Vec<(String, Vec<&'static str>)>,
    /// Normalized secret login of seats whose holder is no longer a member.
    pub departed: Vec<(String, Vec<&'static str>)>,
}

fn normalized_login(login: &str) -> String {
    login.replace('-', "_").to_ascii_uppercase()
}

#[must_use]
pub fn seat_holders(secret_names: &[&str], members: &[&str]) -> SeatHolders {
    let mut by_login: std::collections::BTreeMap<String, Vec<&'static str>> =
        std::collections::BTreeMap::new();
    for name in secret_names {
        let Some(rest) = name.strip_prefix("SEAT_") else {
            continue;
        };
        let Some(provider) = PROVIDERS
            .iter()
            .copied()
            .find(|p| rest.ends_with(&format!("__{}", normalized_login(p))))
        else {
            continue;
        };
        let login = &rest[..rest.len() - normalized_login(provider).len() - 2];
        if login.is_empty()
            || !login
                .bytes()
                .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit() || b == b'_')
        {
            continue;
        }
        by_login.entry(login.to_owned()).or_default().push(provider);
    }
    let mut holders = SeatHolders::default();
    for (login, mut providers) in by_login {
        providers.sort_by_key(|p| PROVIDERS.iter().position(|x| x == p));
        match members.iter().find(|m| normalized_login(m) == login) {
            Some(member) => holders.registered.push(((*member).to_owned(), providers)),
            None => holders.departed.push((login, providers)),
        }
    }
    holders.registered.sort();
    holders
}

fn wakes(front: &Frontmatter) -> String {
    let mut ways = vec!["mention".to_owned()];
    if !front.triggers.is_empty() {
        ways.push(format!(
            "events {}",
            front
                .triggers
                .iter()
                .map(|t| format!("`{t}`"))
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    match &front.schedule {
        Some(Schedule::Daily(_)) => ways.push("daily".into()),
        Some(Schedule::Days(days)) => ways.push(format!("on {}", days.join(", "))),
        None => {}
    }
    ways.join("; ")
}

/// The pinned dashboard issue; `holders` is `None` in public repositories.
#[must_use]
pub fn dashboard_body(golems: &[Golem], holders: Option<&SeatHolders>) -> String {
    let mut lines = vec![
        DASHBOARD_MARKER.to_owned(),
        "Golems in this repository, kept current by isp-golem-xiv.".to_owned(),
        String::new(),
        "## Golems".to_owned(),
        String::new(),
    ];
    let enabled: Vec<_> = golems.iter().filter(|g| g.errors.is_empty()).collect();
    if enabled.is_empty() {
        lines.push("None yet; add one to `.golems/`.".into());
    }
    for golem in enabled {
        let pays = golem.frontmatter.owner.as_deref().map_or_else(
            || "whoever mentions it".to_owned(),
            |owner| format!("whoever mentions it; unattended runs @{owner}"),
        );
        lines.push(format!(
            "- `{}`: {} Wakes: {}. Pays: {pays}. Start: comment `@isp-golem-xiv {}`.",
            golem.name,
            golem.purpose,
            wakes(&golem.frontmatter),
            golem.name
        ));
    }
    lines.extend([String::new(), "## Disabled".to_owned(), String::new()]);
    let disabled: Vec<_> = golems.iter().filter(|g| !g.errors.is_empty()).collect();
    if disabled.is_empty() {
        lines.push("None.".into());
    }
    for golem in disabled {
        lines.push(format!("- `{}`: {}", golem.name, golem.errors.join("; ")));
    }
    lines.extend([
        String::new(),
        "## Talk to golems".to_owned(),
        String::new(),
        "- `@isp-golem-xiv`: list the golems here and the seat and model each would use for you"
            .to_owned(),
        "- `@isp-golem-xiv <golem> [instructions]`: run a golem here".to_owned(),
        "- `@isp-golem-xiv stop`: stop the golem run started here".to_owned(),
        "- `@isp-golem-xiv seat`: show your seats and link to registration".to_owned(),
        String::new(),
        "## Seats".to_owned(),
        String::new(),
    ]);
    let list = |items: &[(String, Vec<&str>)], prefix: &str| -> Vec<String> {
        if items.is_empty() {
            return vec!["- none".into()];
        }
        items
            .iter()
            .map(|(who, providers)| {
                format!(
                    "- {prefix}{who}: {}",
                    providers
                        .iter()
                        .map(|p| format!("`{p}`"))
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            })
            .collect()
    };
    match holders {
        Some(holders) => {
            lines.push("Registered seats:".into());
            lines.extend(list(&holders.registered, "@"));
            lines.push(String::new());
            lines.push("Seats of people who left the organization:".into());
            lines.extend(list(&holders.departed, ""));
            lines.push(String::new());
            lines.push(format!("Register or replace your seat: {SEAT_URL}"));
        }
        None => lines.push(format!(
            "Check your own seat with `@isp-golem-xiv seat`; register one at {SEAT_URL}."
        )),
    }
    lines.join("\n") + "\n"
}

/// Check run name for pull requests that change `.golems/`.
pub const VALIDATION_CHECK: &str = "golems/validate";

/// Validation outcome with the same parser the router uses: all golems valid, and a summary.
#[must_use]
pub fn validation_summary(golems: &[Golem]) -> (bool, String) {
    if golems.is_empty() {
        return (true, "No golems in `.golems/`.".into());
    }
    let lines: Vec<String> = golems
        .iter()
        .map(|golem| {
            if golem.errors.is_empty() {
                format!("- `{}`: valid", golem.name)
            } else {
                format!("- `{}`: {}", golem.name, golem.errors.join("; "))
            }
        })
        .collect();
    let ok = golems.iter().all(|golem| golem.errors.is_empty());
    let head = if ok {
        "All golems are valid."
    } else {
        "Invalid golems are disabled after merge until fixed."
    };
    (ok, format!("{head}\n\n{}", lines.join("\n")))
}

/// Providers in fallback order, matching `contract.nuon` providers.
pub const PROVIDERS: [&str; 2] = ["anthropic", "openai-codex"];

#[must_use]
pub fn harness_works(provider: &str, harness: &str) -> bool {
    matches!(
        (provider, harness),
        ("anthropic", "claude" | "pi") | ("openai-codex", "codex" | "pi")
    )
}

fn default_provider(harness: &str) -> &'static str {
    if harness == "codex" {
        "openai-codex"
    } else {
        "anthropic"
    }
}

fn default_harness(provider: &str) -> &'static str {
    match provider {
        "anthropic" => "claude",
        "openai-codex" => "codex",
        _ => "pi",
    }
}

/// Seat, harness and model a run uses, and why it differs from the golem's preference.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SeatChoice {
    pub provider: String,
    pub harness: String,
    pub model: Option<String>,
    pub fallback: Option<String>,
}

impl SeatChoice {
    /// Whether the pinned runner (v7+) can execute this choice: Anthropic seats with `claude` or
    /// `pi`, `OpenAI` Codex seats with `codex`, and a model that is a single safe CLI argument.
    #[must_use]
    pub fn runnable(&self) -> bool {
        runner_supports(&self.provider, &self.harness, self.model.as_deref())
    }
}

/// Shared by router dispatch and the runner's credential-boundary validation.
#[must_use]
pub fn runner_supports(provider: &str, harness: &str, model: Option<&str>) -> bool {
    matches!(
        (provider, harness),
        ("anthropic", "claude" | "pi") | ("openai-codex", "codex")
    ) && model.is_none_or(|model| {
        !model.is_empty()
            && model.len() <= 128
            && !model.starts_with('-')
            && model
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || b"-._".contains(&byte))
    })
}

/// The provider a golem prefers: its own, else its harness's home provider.
#[must_use]
pub fn wanted_provider(frontmatter: &Frontmatter) -> &str {
    frontmatter
        .provider
        .as_deref()
        .unwrap_or_else(|| default_provider(frontmatter.harness.as_deref().unwrap_or("claude")))
}

/// Summary of the check run created at dispatch; a fallback always says so.
#[must_use]
pub fn queued_summary(seat_login: &str, choice: &SeatChoice, timeout_seconds: u32) -> String {
    let fallback = choice
        .fallback
        .as_deref()
        .map(|fallback| format!("\nfallback: {fallback}"))
        .unwrap_or_default();
    format!(
        "outcome: queued\nseat: {seat_login}/{}\nharness: {}\nmodel: {}\nbudget: {timeout_seconds}s\nlog: https://github.com/{OPS}/actions{fallback}\nStop: @isp-golem-xiv stop",
        choice.provider,
        choice.harness,
        choice.model.as_deref().unwrap_or("default"),
    )
}

/// SPEC "Which seat and model": golem provider first, else the preferred seat, else the first
/// registered seat; keep the harness if it works there, else `pi` with its default model, even at
/// the golem's own provider.
#[must_use]
pub fn choose_seat(
    frontmatter: &Frontmatter,
    seats: &[&str],
    preferred: Option<&str>,
) -> Option<SeatChoice> {
    let wanted = wanted_provider(frontmatter);
    let harness = frontmatter
        .harness
        .as_deref()
        .unwrap_or_else(|| default_harness(wanted));
    let (provider, reason) = if seats.contains(&wanted) {
        if harness_works(wanted, harness) {
            return Some(SeatChoice {
                provider: wanted.into(),
                harness: harness.into(),
                model: frontmatter.model.clone(),
                fallback: None,
            });
        }
        (wanted, format!("{harness} does not work with {wanted}"))
    } else {
        let provider = preferred
            .filter(|p| seats.contains(p))
            .or_else(|| PROVIDERS.iter().copied().find(|p| seats.contains(p)))?;
        (provider, format!("there is no {wanted} seat"))
    };
    let used = if harness_works(provider, harness) {
        harness
    } else {
        "pi"
    };
    Some(SeatChoice {
        provider: provider.into(),
        harness: used.into(),
        model: None,
        fallback: Some(format!(
            "asked for {wanted}/{harness}{}, used {provider}/{used} with its default model, because {reason}",
            frontmatter
                .model
                .as_deref()
                .map(|m| format!("/{m}"))
                .unwrap_or_default()
        )),
    })
}

fn seat_list(seats: &[&str]) -> String {
    if seats.is_empty() {
        "none".into()
    } else {
        seats
            .iter()
            .map(|s| format!("`{s}`"))
            .collect::<Vec<_>>()
            .join(", ")
    }
}

/// Reply to a bare mention: what can I do here, whose seat pays, what happens next.
#[must_use]
pub fn help_reply(
    login: &str,
    golems: &[Golem],
    seats: &[&str],
    preferred: Option<&str>,
) -> String {
    let mut lines = vec![
        "<!-- isp-golem-xiv kind=help -->".to_owned(),
        format!("Golems here and what each would use for @{login}:"),
        String::new(),
    ];
    for golem in golems.iter().filter(|g| g.errors.is_empty()) {
        let line = match choose_seat(&golem.frontmatter, seats, preferred) {
            Some(choice) => format!(
                "seat `{login}/{}`, harness `{}`, model `{}`{}",
                choice.provider,
                choice.harness,
                choice.model.as_deref().unwrap_or("default"),
                choice
                    .fallback
                    .map(|f| format!(" (fallback: {f})"))
                    .unwrap_or_default()
            ),
            None => format!("no seat; register one at {SEAT_URL}"),
        };
        lines.push(format!("- `{}`: {line}", golem.name));
    }
    let disabled: Vec<_> = golems
        .iter()
        .filter(|g| !g.errors.is_empty())
        .map(|g| format!("`{}`", g.name))
        .collect();
    if !disabled.is_empty() {
        lines.push(String::new());
        lines.push(format!("Disabled: {}.", disabled.join(", ")));
    }
    lines.push(String::new());
    lines.push("Run one with `@isp-golem-xiv <golem> [instructions]`, stop it with `@isp-golem-xiv stop`, check your seats with `@isp-golem-xiv seat`.".into());
    lines.push(format!(
        "Your seats: {}. Register or replace a seat: {SEAT_URL}",
        seat_list(seats)
    ));
    lines.join("\n") + "\n"
}

/// Reply to `@isp-golem-xiv seat`: only the asker's own seats, never anyone else's.
#[must_use]
pub fn seat_reply(login: &str, seats: &[&str], preferred: Option<&str>) -> String {
    let mut lines = vec![
        "<!-- isp-golem-xiv kind=seat -->".to_owned(),
        format!("@{login}, your seats: {}.", seat_list(seats)),
    ];
    if let Some(preferred) = preferred.filter(|p| seats.contains(p)) {
        lines.push(format!(
            "Preferred seat when a golem's provider is missing: `{preferred}`."
        ));
    }
    lines.push(format!(
        "Register, replace, or choose your preferred seat: {SEAT_URL}"
    ));
    lines.join("\n") + "\n"
}

impl Golem {
    #[must_use]
    pub fn timeout_seconds(&self) -> Option<u32> {
        timeout_seconds(self.frontmatter.timeout.as_deref())
    }

    #[must_use]
    pub fn branch(&self) -> String {
        self.frontmatter
            .branch
            .clone()
            .unwrap_or_else(|| format!("golems/{}", self.name))
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct EventDelivery {
    #[serde(default)]
    pub action: String,
    pub sender: User,
    pub repository: Repository,
    pub installation: Installation,
    pub issue: Option<Issue>,
    pub pull_request: Option<Issue>,
    pub comment: Option<Comment>,
}

impl EventDelivery {
    #[must_use]
    pub fn accepted(&self) -> bool {
        self.sender.login != BOT
            && self
                .comment
                .as_ref()
                .is_none_or(|comment| comment.user.login != BOT)
            && self.sender.id > 0
            && ((self.sender.kind == "User" && valid_login(&self.sender.login))
                || (self.sender.kind == "Bot"
                    && self
                        .sender
                        .login
                        .strip_suffix("[bot]")
                        .is_some_and(valid_login)))
            && self.repository.id > 0
            && valid_repository(&self.repository.full_name)
            && self.installation.id == 153_964_219
            && self.action.len() <= 40
            && self
                .action
                .bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte == b'_')
            && self.issue.as_ref().is_none_or(|issue| issue.number > 0)
            && self
                .pull_request
                .as_ref()
                .is_none_or(|issue| issue.number > 0)
    }
}

#[must_use]
pub fn trigger_matches(triggers: &[String], event: &str, action: &str) -> bool {
    triggers.iter().take(32).any(|trigger| {
        trigger == event
            || trigger.strip_prefix(event).is_some_and(|suffix| {
                !action.is_empty() && suffix.strip_prefix('.') == Some(action)
            })
    })
}

#[derive(Debug, Clone, Copy)]
pub enum EventActor {
    MemberWithSeat,
    MemberWithoutSeat,
    Bot,
    Outsider,
}

/// Membership and seat availability are supplied by the edge, never inferred from a login.
#[must_use]
pub fn event_seat<'a>(
    actor: &'a str,
    identity: EventActor,
    owner: Option<&'a str>,
    owner_member: bool,
    owner_seat: bool,
) -> Option<&'a str> {
    match identity {
        EventActor::Outsider => None,
        EventActor::MemberWithSeat => valid_login(actor).then_some(actor),
        EventActor::MemberWithoutSeat | EventActor::Bot => {
            owner.filter(|login| valid_login(login) && owner_member && owner_seat)
        }
    }
}

#[must_use]
pub fn schedule_matches(schedule: &Schedule, utc_day: u64) -> bool {
    let weekday = match (utc_day % 7 + 3) % 7 {
        0 => "mon",
        1 => "tue",
        2 => "wed",
        3 => "thu",
        4 => "fri",
        5 => "sat",
        _ => "sun",
    };
    match schedule {
        Schedule::Daily(value) => value == "daily",
        Schedule::Days(days) => days.iter().take(7).any(|day| day == weekday),
    }
}

/// No catch-up: the chosen minute must remain in today's UTC day.
#[must_use]
pub fn next_schedule_minute(schedule: &Schedule, timestamp_ms: u64) -> Option<u64> {
    let minute = timestamp_ms / 60_000;
    let next = minute.checked_add(1)?;
    (next / 1440 == minute / 1440 && schedule_matches(schedule, minute / 1440)).then_some(next)
}

/// Stable UUID-shaped identity shared by all retries of one unattended occurrence.
#[must_use]
pub fn occurrence_id(namespace: &str) -> String {
    use std::fmt::Write;
    let hex = Sha256::digest(namespace.as_bytes()).iter().fold(
        String::with_capacity(64),
        |mut hex, byte| {
            let _result = write!(&mut hex, "{byte:02x}");
            hex
        },
    );
    // Digest encoding has a fixed, known length; get() keeps this panic-free.
    [
        hex.get(..8),
        hex.get(8..12),
        hex.get(12..16),
        hex.get(16..20),
        hex.get(20..32),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>()
    .join("-")
}

#[must_use]
pub fn verify_queue_signature(pem: &str, signature: &str, body: &[u8], now_ms: u64) -> bool {
    if body.len() > 16_384 {
        return false;
    }
    let normalized = pem.replace("\r\n", "\n");
    if !verify_signature(normalized.trim().as_bytes(), signature, body) {
        return false;
    }
    let Ok(value) = serde_json::from_slice::<serde_json::Value>(body) else {
        return false;
    };
    value
        .get("timestamp_ms")
        .and_then(serde_json::Value::as_u64)
        .is_some_and(|timestamp| now_ms.abs_diff(timestamp) <= 60_000)
        && match value.get("action").and_then(serde_json::Value::as_str) {
            Some("bind") => true,
            Some("complete") => {
                value
                    .get("containers_destroyed")
                    .and_then(serde_json::Value::as_bool)
                    == Some(true)
                    && value
                        .get("publication_complete")
                        .and_then(serde_json::Value::as_bool)
                        == Some(true)
            }
            _ => false,
        }
}
