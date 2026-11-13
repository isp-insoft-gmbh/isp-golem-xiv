//! Codex sign-ins (the Codex CLI `auth.json`) and the strict `codex exec --json` protocol.
//!
//! A refreshing sign-in rotates its refresh token, so the sign-in a run ends with must be
//! written back; a credential that fails validation never is. The stored seat is always in
//! Codex's format; Pi runs on the same seat through a converted copy in Pi's format.

use crate::{AgentOutput, OutputGuard, Result, WritableAnswer, checked_output, valid_model};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
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

    /// The whole file in both formats and every credential value on its own: a bare token is
    /// not a line of the pretty-printed file, so line-wise guarding alone would miss it.
    pub fn guard(&self, guard: &mut OutputGuard) {
        guard.add(&self.text);
        if let Ok(pi) = self.pi_text() {
            guard.add(&pi);
        }
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

/// Pi's provider id for `ChatGPT` subscriptions, the key of its entry in Pi's `auth.json`.
pub const PI_PROVIDER: &str = "openai-codex";
/// Largest integer a JavaScript number holds exactly; Pi compares `expires` as a number.
const JS_SAFE_INTEGER_MAX: u64 = (1 << 53) - 1;

#[derive(Deserialize)]
struct AccessClaims {
    #[serde(default)]
    exp: Option<u64>,
    #[serde(rename = "https://api.openai.com/auth", default)]
    auth: Option<AccountClaims>,
}

#[derive(Deserialize)]
struct AccountClaims {
    #[serde(default)]
    chatgpt_account_id: Option<String>,
}

/// Unverified claims of a `ChatGPT` access token, read like Codex and Pi read them.
fn access_claims(token: &str) -> Option<AccessClaims> {
    let mut parts = token.split('.');
    let (Some(_), Some(payload), Some(_), None) =
        (parts.next(), parts.next(), parts.next(), parts.next())
    else {
        return None;
    };
    serde_json::from_slice(&URL_SAFE_NO_PAD.decode(payload).ok()?).ok()
}

/// The account Pi and Codex send with every request: the access token's claim.
fn access_account(token: &str) -> Option<String> {
    access_claims(token)?
        .auth?
        .chatgpt_account_id
        .filter(|id| !id.is_empty())
}

impl CodexAuth {
    /// The same sign-in as Pi 1.5.1 stores it in `<agent dir>/auth.json`:
    /// `{"openai-codex": {type: "oauth", access, refresh, expires, accountId}}`, `expires` in
    /// milliseconds. Pi refreshes five minutes before `expires`, so it is the access token's
    /// own `exp`, as Codex uses it.
    ///
    /// # Errors
    /// An access token without a decodable `exp` (guessing would make Pi refresh, and rotate
    /// the seat, on every run) or without the account claim Pi sends with every request
    /// (`openai-codex-responses.js` reads it from the token, never from the stored `accountId`).
    pub fn pi_text(&self) -> Result<String> {
        let claims = access_claims(&self.tokens.access_token)
            .ok_or("Codex sign-in access token is not a JWT; Pi cannot use it")?;
        let expires = claims
            .exp
            .and_then(|exp| exp.checked_mul(1000))
            .filter(|expires| *expires <= JS_SAFE_INTEGER_MAX)
            .ok_or("Codex sign-in access token has no expiry; Pi cannot use it")?;
        let account = claims
            .auth
            .and_then(|auth| auth.chatgpt_account_id)
            .filter(|id| !id.is_empty())
            .ok_or("Codex sign-in access token names no account; Pi cannot use it")?;
        let credential = serde_json::json!({
            "type": "oauth",
            "access": self.tokens.access_token,
            "refresh": self.tokens.refresh_token,
            "expires": expires,
            "accountId": account,
        });
        serde_json::to_string_pretty(&serde_json::json!({ PI_PROVIDER: credential }))
            .map_err(|_| "cannot convert Codex sign-in for Pi")
    }
}

/// Pi's `openai-codex` entry: access token, refresh token and the account it names, if any.
fn pi_credential(text: &str) -> Option<(String, String, Option<String>)> {
    let value: serde_json::Value = serde_json::from_str(text).ok()?;
    let entry = value.get(PI_PROVIDER)?;
    let field = |key: &str| entry.get(key).and_then(serde_json::Value::as_str);
    if field("type") != Some("oauth")
        || !entry
            .get("expires")
            .and_then(serde_json::Value::as_f64)
            .is_some_and(f64::is_finite)
    {
        return None;
    }
    let account = match entry.get("accountId") {
        None | Some(serde_json::Value::Null) => None,
        Some(account) => Some(account.as_str()?.to_owned()),
    };
    Some((
        field("access")?.to_owned(),
        field("refresh")?.to_owned(),
        account,
    ))
}

/// Guard whatever a Pi run left in its `auth.json`: the whole text and every string in it.
pub fn guard_pi_text(text: &str, guard: &mut OutputGuard) {
    fn strings(value: &serde_json::Value, guard: &mut OutputGuard) {
        match value {
            serde_json::Value::String(text) => guard.add(text),
            serde_json::Value::Array(values) => values.iter().for_each(|v| strings(v, guard)),
            serde_json::Value::Object(map) => map.values().for_each(|v| strings(v, guard)),
            _ => {}
        }
    }
    guard.add(text);
    if let Ok(value) = serde_json::from_str(text) {
        strings(&value, guard);
    }
}

/// RFC 3339 UTC time in whole seconds, as Codex accepts for `last_refresh`.
fn rfc3339(time: std::time::SystemTime) -> Option<String> {
    let seconds = time.duration_since(std::time::UNIX_EPOCH).ok()?.as_secs();
    let (days, rest) = (seconds / 86_400, seconds % 86_400);
    // Howard Hinnant's civil_from_days, for days since 1970-01-01.
    let z = i64::try_from(days).ok()? + 719_468;
    let era = z.div_euclid(146_097);
    let day_of_era = z.rem_euclid(146_097);
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let shifted_month = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * shifted_month + 2) / 5 + 1;
    let month = if shifted_month < 10 {
        shifted_month + 3
    } else {
        shifted_month - 9
    };
    let year = year_of_era + era * 400 + i64::from(month <= 2);
    Some(format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        rest / 3600,
        rest / 60 % 60,
        rest % 60
    ))
}

/// Write-back decision for a Pi run on a Codex seat. Pi keeps only access and refresh tokens
/// and discards the refreshed `id_token`, so the result has the original Codex file's fields
/// (keys re-serialized in sorted order) with new tokens and `last_refresh` stamped by the
/// runner at read-back; every other field, including the `id_token` and unknown fields Codex
/// owns, keeps its value. Codex 0.167.1 reads the `id_token` only for claims (email, plan,
/// account), never its expiry, and keeps it itself when a refresh response lacks one. The account is compared on the access tokens' claims, which
/// both harnesses send with every request; the result then passes the Codex checks.
#[must_use]
pub fn pi_refresh(
    original: &CodexAuth,
    after: Result<String>,
    now: std::time::SystemTime,
) -> Refresh {
    let after = match after {
        Ok(after) => after,
        Err(error) => return Refresh::Invalid(error),
    };
    let Some((access, refresh_token, named_account)) = pi_credential(&after) else {
        return Refresh::Invalid("refreshed sign-in is not a valid Pi sign-in");
    };
    if access == original.tokens.access_token && refresh_token == original.tokens.refresh_token {
        return Refresh::Unchanged;
    }
    let before = access_account(&original.tokens.access_token)
        .or_else(|| original.tokens.account_id.clone());
    let Some(account) = access_account(&access) else {
        return Refresh::Invalid("refreshed sign-in names no account");
    };
    if before.is_some_and(|before| before != account)
        || named_account.is_some_and(|named| named != account)
    {
        return Refresh::Invalid("refreshed sign-in belongs to another account");
    }
    let Ok(mut file) =
        serde_json::from_str::<serde_json::Map<String, serde_json::Value>>(&original.text)
    else {
        return Refresh::Invalid("refreshed sign-in is not a valid Codex sign-in");
    };
    let (Some(tokens), Some(stamp)) = (
        file.get_mut("tokens")
            .and_then(serde_json::Value::as_object_mut),
        rfc3339(now),
    ) else {
        return Refresh::Invalid("refreshed sign-in is not a valid Codex sign-in");
    };
    tokens.insert("access_token".into(), access.into());
    tokens.insert("refresh_token".into(), refresh_token.into());
    file.insert("last_refresh".into(), stamp.into());
    match serde_json::to_string_pretty(&file) {
        Ok(text) => refresh(original, Ok(text)),
        Err(_) => Refresh::Invalid("refreshed sign-in is not a valid Codex sign-in"),
    }
}

/// A private directory holding only one sign-in file, `auth.json`, outside every workspace:
/// `CODEX_HOME` with the seat as is, or Pi's agent directory with its converted copy.
/// Dropping it removes the directory, including whatever the harness added.
///
/// # Errors
/// Fails when the directory or file cannot be created privately.
pub fn private_home(sign_in: &str) -> Result<tempfile::TempDir> {
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
        .and_then(|mut file| file.write_all(sign_in.as_bytes()))
        .map_err(|_| "cannot write private seat sign-in")?;
    Ok(home)
}

/// Read back only `auth.json`, as a bounded regular file; call after confirmed destruction.
/// Pi keeps its lock directory `auth.json.lock` beside it; that and everything else is ignored.
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
