//! Codex sign-ins (the Codex CLI `auth.json`) and the strict `codex exec --json` protocol.
//!
//! A refreshing sign-in rotates its refresh token, so the sign-in a run ends with must be
//! written back; a credential that fails validation never is.

use crate::{AgentOutput, OutputGuard, Result, WritableAnswer, checked_output, valid_model};
use serde::Deserialize;
use std::path::Path;

/// Bound of a seat sign-in, below GitHub's 48 KB secret limit.
pub const AUTH_BYTES_MAX: usize = 32_000;
/// File name Codex reads and refreshes inside `CODEX_HOME`.
pub const AUTH_FILE: &str = "auth.json";
const TOKEN_BYTES_MAX: usize = 16_384;
const RECORDS_MAX: usize = 4096;
const ANSWER_BYTES_MAX: usize = 48_000;

#[derive(Deserialize, Clone, PartialEq, Eq)]
struct Tokens {
    id_token: String,
    access_token: String,
    refresh_token: String,
    #[serde(default)]
    account_id: Option<String>,
}

// Unknown fields stay in the original text: Codex owns its format, the runner only checks it.
#[derive(Deserialize)]
struct AuthFile {
    #[serde(default)]
    auth_mode: Option<String>,
    #[serde(rename = "OPENAI_API_KEY", default)]
    api_key: Option<String>,
    tokens: Option<Tokens>,
    #[serde(default)]
    last_refresh: Option<String>,
}

/// A validated `ChatGPT` sign-in, kept as the exact text Codex wrote.
#[derive(Clone)]
pub struct CodexAuth {
    text: String,
    tokens: Tokens,
    api_key: Option<String>,
    last_refresh: Option<String>,
}

fn valid_token(token: &str) -> bool {
    (16..=TOKEN_BYTES_MAX).contains(&token.len())
        && token.bytes().all(|byte| byte.is_ascii_graphic())
}

impl CodexAuth {
    /// # Errors
    /// Rejects oversized text, API-key-only or malformed sign-ins and implausible tokens.
    pub fn parse(text: &str) -> Result<Self> {
        if text.len() > AUTH_BYTES_MAX {
            return Err("Codex sign-in exceeds limit");
        }
        let file: AuthFile =
            serde_json::from_str(text).map_err(|_| "selected seat is not a Codex sign-in")?;
        let tokens = file.tokens.ok_or("selected seat is not a Codex sign-in")?;
        if file
            .auth_mode
            .as_deref()
            .is_some_and(|mode| mode != "chatgpt")
            || ![
                &tokens.id_token,
                &tokens.access_token,
                &tokens.refresh_token,
            ]
            .iter()
            .all(|token| valid_token(token))
            || tokens.account_id.as_deref().is_some_and(|id| {
                id.is_empty() || id.len() > 128 || !id.bytes().all(|byte| byte.is_ascii_graphic())
            })
            || file.last_refresh.as_deref().is_some_and(|time| {
                time.is_empty() || time.len() > 64 || time.chars().any(char::is_control)
            })
        {
            return Err("selected seat is not a Codex sign-in");
        }
        Ok(Self {
            text: text.to_owned(),
            tokens,
            api_key: file.api_key.filter(|key| !key.is_empty()),
            last_refresh: file.last_refresh,
        })
    }

    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }

    /// The whole file and every credential value on its own: a bare token is not a line of
    /// the pretty-printed file, so line-wise guarding alone would miss it.
    pub fn guard(&self, guard: &mut OutputGuard) {
        guard.add(&self.text);
        for value in [
            &self.tokens.id_token,
            &self.tokens.access_token,
            &self.tokens.refresh_token,
        ] {
            guard.add(value);
        }
        if let Some(key) = &self.api_key {
            guard.add(key);
        }
    }
}

/// Guard whatever a run left in `auth.json`, valid or not, before any publication.
pub fn guard_text(text: &str, guard: &mut OutputGuard) {
    guard.add(text);
    if let Ok(auth) = CodexAuth::parse(text) {
        auth.guard(guard);
    }
}

/// What to do with the sign-in a run ended with.
pub enum Refresh {
    Unchanged,
    Refreshed(CodexAuth),
    Invalid(&'static str),
}

/// Pure write-back decision; compares parsed credentials, never formatting.
#[must_use]
pub fn refresh(original: &CodexAuth, after: Result<String>) -> Refresh {
    let after = match after {
        Ok(after) if after == original.text => return Refresh::Unchanged,
        Ok(after) => after,
        Err(error) => return Refresh::Invalid(error),
    };
    let Ok(after) = CodexAuth::parse(&after) else {
        return Refresh::Invalid("refreshed sign-in is not a valid Codex sign-in");
    };
    if after.tokens == original.tokens {
        return Refresh::Unchanged;
    }
    if original.tokens.account_id.is_some() && after.tokens.account_id != original.tokens.account_id
    {
        return Refresh::Invalid("refreshed sign-in belongs to another account");
    }
    if after.last_refresh.is_none() || after.last_refresh == original.last_refresh {
        return Refresh::Invalid("refreshed sign-in has no new refresh time");
    }
    Refresh::Refreshed(after)
}

/// A private `CODEX_HOME` holding only the seat's sign-in, outside every workspace.
/// Dropping it removes the directory, including whatever Codex added.
///
/// # Errors
/// Fails when the directory or file cannot be created privately.
pub fn private_home(auth: &CodexAuth) -> Result<tempfile::TempDir> {
    use std::io::Write as _;
    let home = tempfile::Builder::new()
        .prefix("golem-seat-")
        .tempdir()
        .map_err(|_| "cannot create private seat home")?;
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
        std::fs::set_permissions(home.path(), std::fs::Permissions::from_mode(0o700))
            .map_err(|_| "cannot restrict private seat home")?;
        options.mode(0o600);
    }
    options
        .open(home.path().join(AUTH_FILE))
        .and_then(|mut file| file.write_all(auth.text.as_bytes()))
        .map_err(|_| "cannot write private seat sign-in")?;
    Ok(home)
}

/// Read back only `auth.json`, as a bounded regular file; call after confirmed destruction.
///
/// # Errors
/// Missing, linked, special, oversized or non-UTF-8 files.
pub fn read_auth(home: &Path) -> Result<String> {
    use std::io::Read as _;
    let path = home.join(AUTH_FILE);
    let metadata = std::fs::symlink_metadata(&path).map_err(|_| "refreshed sign-in missing")?;
    if !metadata.is_file() || metadata.len() > AUTH_BYTES_MAX as u64 {
        return Err("refreshed sign-in is not a bounded regular file");
    }
    let mut bytes = Vec::new();
    std::fs::File::open(&path)
        .and_then(|file| file.take(AUTH_BYTES_MAX as u64 + 1).read_to_end(&mut bytes))
        .map_err(|_| "refreshed sign-in unreadable")?;
    if bytes.len() > AUTH_BYTES_MAX {
        return Err("refreshed sign-in is not a bounded regular file");
    }
    String::from_utf8(bytes).map_err(|_| "refreshed sign-in is not UTF-8")
}

/// Facts from `codex exec --json`; prose never decides an outcome.
#[derive(Default)]
struct Stream {
    threads: usize,
    turns: usize,
    completed: usize,
    failed: bool,
    sign_in_rejected: bool,
    after_completion: bool,
    last_message: Option<String>,
    rerouted: Option<String>,
}

// Codex reports refused or unrefreshable sign-ins only as text.
fn sign_in_rejected(message: &str) -> bool {
    let message = message.to_ascii_lowercase();
    [
        "401",
        "unauthorized",
        "sign in again",
        "refresh token",
        "access token",
    ]
    .iter()
    .any(|needle| message.contains(needle))
}

impl Stream {
    fn parse(stdout: &[u8], partial: bool) -> Result<Self> {
        let mut stream = Self::default();
        for (index, value) in serde_json::Deserializer::from_slice(stdout)
            .into_iter::<serde_json::Value>()
            .enumerate()
        {
            if index >= RECORDS_MAX {
                return Err("too many harness records");
            }
            let value = match value {
                Ok(value) => value,
                Err(error) if partial && error.is_eof() => break,
                Err(_) => return Err("invalid harness result"),
            };
            stream.record(&value)?;
            if stream
                .last_message
                .as_ref()
                .is_some_and(|message| message.len() > ANSWER_BYTES_MAX * 2)
            {
                return Err("harness result exceeds limit");
            }
        }
        Ok(stream)
    }

    fn record(&mut self, value: &serde_json::Value) -> Result<()> {
        let text = |value: &serde_json::Value, key: &str| {
            value
                .get(key)
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned)
        };
        let kind = text(value, "type").ok_or("invalid harness result")?;
        // Records of other types (project hooks, MCP, newer versions) never carry the result.
        if !matches!(
            kind.as_str(),
            "thread.started"
                | "turn.started"
                | "turn.completed"
                | "turn.failed"
                | "error"
                | "item.started"
                | "item.updated"
                | "item.completed"
        ) {
            return Ok(());
        }
        self.after_completion |= self.completed > 0;
        match kind.as_str() {
            "thread.started" => self.threads += 1,
            "turn.started" => self.turns += 1,
            "turn.completed" => self.completed += 1,
            "turn.failed" | "error" => {
                self.failed = true;
                let message = value
                    .pointer("/error/message")
                    .or_else(|| value.get("message"))
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or_default();
                self.sign_in_rejected |= sign_in_rejected(message);
            }
            "item.started" | "item.updated" | "item.completed" => {
                let item = value
                    .get("item")
                    .filter(|item| item.is_object())
                    .ok_or("invalid harness item")?;
                if kind != "item.completed" {
                    return Ok(());
                }
                match text(item, "type").as_deref() {
                    Some("agent_message") => {
                        self.last_message = Some(text(item, "text").ok_or("invalid harness item")?);
                    }
                    // Non-fatal notices; only a model reroute changes attribution.
                    Some("error") => {
                        if let Some(rerouted) = text(item, "message")
                            .as_deref()
                            .and_then(|message| message.strip_prefix("model rerouted: "))
                            .and_then(|rest| rest.split_once(" -> "))
                            .map(|(_, to)| to.split_once(" (").map_or(to, |(model, _)| model))
                        {
                            self.rerouted = Some(rerouted.to_owned());
                        }
                    }
                    Some(_) => {}
                    None => return Err("invalid harness item"),
                }
            }
            _ => {}
        }
        Ok(())
    }

    /// Codex never reports the model it used; this is the requested one or `default`,
    /// unless Codex announced a reroute.
    fn model(&self, requested: Option<&str>) -> Result<String> {
        let model = self.rerouted.as_deref().or(requested).unwrap_or("default");
        if !valid_model(model) {
            return Err("invalid observed model");
        }
        Ok(model.to_owned())
    }

    /// One thread, one turn completed last, no failure, and its final agent message is the
    /// strictly typed answer. Interim agent messages are normal; only the last one counts.
    fn completed(self, guard: &OutputGuard, requested: Option<&str>) -> Result<AgentOutput> {
        if self.failed {
            return Err(if self.sign_in_rejected {
                "seat sign-in rejected by provider; register the seat again"
            } else {
                "harness did not complete successfully"
            });
        }
        if self.threads != 1 || self.turns != 1 || self.completed != 1 || self.after_completion {
            return Err("harness did not complete successfully");
        }
        let message = self
            .last_message
            .as_deref()
            .ok_or("missing writable result")?;
        let answer: WritableAnswer =
            serde_json::from_str(message).map_err(|_| "invalid writable result")?;
        if answer.message.trim().is_empty() || answer.message.len() > ANSWER_BYTES_MAX {
            return Err("empty or oversized reply");
        }
        let model = self.model(requested)?;
        guard.check(&answer.message)?;
        guard.check(&model)?;
        Ok(AgentOutput {
            text: answer.message,
            model,
            disposition: Some(answer.disposition),
        })
    }
}

/// # Errors
/// Requires the strict Codex protocol; never infers status from prose.
pub fn writable_result(
    stdout: &[u8],
    stderr: &[u8],
    guard: &OutputGuard,
    requested_model: Option<&str>,
) -> Result<AgentOutput> {
    checked_output(stdout, stderr, guard)?;
    Stream::parse(stdout, false)?.completed(guard, requested_model)
}

/// # Errors
/// After a timeout: a finished or failed turn must satisfy the full protocol; otherwise
/// bounded partial text of the last agent message.
pub fn partial_result(
    stdout: &[u8],
    stderr: &[u8],
    guard: &OutputGuard,
    requested_model: Option<&str>,
) -> Result<AgentOutput> {
    checked_output(stdout, stderr, guard)?;
    let stream = Stream::parse(stdout, true)?;
    if stream.completed > 0 || stream.failed {
        return stream.completed(guard, requested_model);
    }
    let model = stream.model(requested_model)?;
    let mut text = match stream.last_message.as_deref() {
        // Interim messages may already follow the output schema.
        Some(message) => serde_json::from_str::<WritableAnswer>(message)
            .map_or_else(|_| message.to_owned(), |answer| answer.message),
        None => String::new(),
    };
    if text.len() > ANSWER_BYTES_MAX {
        return Err("partial result exceeds limit");
    }
    if text.trim().is_empty() {
        text = "Budget exhausted before a final answer was observed.".into();
    }
    guard.check(&text)?;
    Ok(AgentOutput {
        text,
        model,
        disposition: None,
    })
}

#[cfg(test)]
#[path = "codex_tests.rs"]
pub(crate) mod tests;
