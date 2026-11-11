//! Seat registration: signed sessions, CSRF tokens, request validation, and the pages.
//! Browsers seal credentials for the ops repository's public key; the router relays only
//! ciphertext, so it never sees a credential.
use std::fmt::Write;

use hmac::{Hmac, KeyInit, Mac};
use serde::Deserialize;
use sha2::Sha256;

use crate::{CLIENT_ID, OAUTH_CALLBACK, PROVIDERS, ROUTER_URL, percent_encode, valid_login};

type HmacSha256 = Hmac<Sha256>;

pub const SESSION_COOKIE: &str = "golem_session";
pub const STATE_COOKIE: &str = "golem_oauth_state";
pub const SESSION_SECONDS: u64 = 3600;
pub const STATE_COOKIE_CLEAR: &str =
    "golem_oauth_state=; Path=/auth/github; Max-Age=0; HttpOnly; Secure; SameSite=Lax";
pub const CSRF_HEADER: &str = "X-Golem-Csrf";
pub const CONTENT_SECURITY_POLICY: &str = "default-src 'none'; script-src 'self'; connect-src 'self'; form-action 'self'; frame-ancestors 'none'; base-uri 'none'";
/// Ephemeral X25519 public key plus Poly1305 tag of a libsodium sealed box.
pub const SEALED_OVERHEAD_BYTES: usize = 48;
/// GitHub's limit for one secret value.
pub const SECRET_BYTES_MAX: usize = 49_152;
/// Base64 of the largest sealed secret (65600 characters) plus the JSON envelope.
pub const REGISTRATION_BODY_BYTES_MAX: usize = 66_560;
pub const PREFERRED_BODY_BYTES_MAX: usize = 1024;
const COOKIE_HEADER_BYTES_MAX: usize = 8192;
const SESSION_BYTES_MAX: usize = 128;
/// Tolerated clock difference between the isolate that signed a session and the one checking it.
const CLOCK_SKEW_SECONDS: u64 = 60;

/// Session MAC key, derived so the client secret itself never signs anything.
#[must_use]
pub fn session_key(client_secret: &str) -> Option<[u8; 32]> {
    if client_secret.is_empty() {
        return None;
    }
    let mut mac = HmacSha256::new_from_slice(client_secret.as_bytes()).ok()?;
    mac.update(b"isp-golem-xiv session key v1");
    Some(mac.finalize().into_bytes().into())
}

/// Domain-separated MAC over newline-joined parts; no part may contain a newline.
fn mac(key: &[u8; 32], parts: &[&[u8]]) -> Option<HmacSha256> {
    let mut mac = HmacSha256::new_from_slice(key).ok()?;
    for (index, part) in parts.iter().enumerate() {
        if part.contains(&b'\n') {
            return None;
        }
        if index > 0 {
            mac.update(b"\n");
        }
        mac.update(part);
    }
    Some(mac)
}

fn hex(bytes: &[u8]) -> String {
    let mut text = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        let _ = write!(text, "{byte:02x}");
    }
    text
}

/// Lowercase-or-uppercase hex of exactly 32 bytes.
#[must_use]
pub fn unhex32(text: &str) -> Option<[u8; 32]> {
    if text.len() != 64 {
        return None;
    }
    let mut bytes = [0; 32];
    for (byte, &[high, low]) in bytes.iter_mut().zip(text.as_bytes().as_chunks::<2>().0) {
        let high = char::from(high).to_digit(16)?;
        let low = char::from(low).to_digit(16)?;
        *byte = u8::try_from(high * 16 + low).ok()?;
    }
    Some(bytes)
}

/// Cookie value `login.expires.mac`, valid for [`SESSION_SECONDS`] from `now_s`.
#[must_use]
pub fn session_value(key: &[u8; 32], login: &str, now_s: u64) -> Option<String> {
    if !valid_login(login) {
        return None;
    }
    let expires = now_s.checked_add(SESSION_SECONDS)?.to_string();
    let tag = mac(key, &[b"session", login.as_bytes(), expires.as_bytes()])?;
    Some(format!(
        "{login}.{expires}.{}",
        hex(&tag.finalize().into_bytes())
    ))
}

/// Login of an authentic, unexpired session; expiries beyond one session length (plus clock
/// skew) are refused even with a valid MAC.
#[must_use]
pub fn session_login<'a>(key: &[u8; 32], value: &'a str, now_s: u64) -> Option<&'a str> {
    if value.len() > SESSION_BYTES_MAX {
        return None;
    }
    let mut parts = value.split('.');
    let (Some(login), Some(expires), Some(tag), None) =
        (parts.next(), parts.next(), parts.next(), parts.next())
    else {
        return None;
    };
    if !valid_login(login)
        || expires.is_empty()
        || expires.len() > 12
        || !expires.bytes().all(|b| b.is_ascii_digit())
    {
        return None;
    }
    let tag = unhex32(tag)?;
    mac(key, &[b"session", login.as_bytes(), expires.as_bytes()])?
        .verify_slice(&tag)
        .ok()?;
    let expires_s: u64 = expires.parse().ok()?;
    let latest = now_s.saturating_add(SESSION_SECONDS + CLOCK_SKEW_SECONDS);
    (now_s < expires_s && expires_s <= latest).then_some(login)
}

/// Per-session CSRF token the page sends back in [`CSRF_HEADER`].
#[must_use]
pub fn csrf_token(key: &[u8; 32], session: &str) -> Option<String> {
    let tag = mac(key, &[b"csrf", session.as_bytes()])?;
    Some(hex(&tag.finalize().into_bytes()))
}

/// Constant-time check of a submitted CSRF token against the session it must derive from.
#[must_use]
pub fn csrf_valid(key: &[u8; 32], session: &str, submitted: &str) -> bool {
    let Some(tag) = unhex32(submitted) else {
        return false;
    };
    mac(key, &[b"csrf", session.as_bytes()]).is_some_and(|mac| mac.verify_slice(&tag).is_ok())
}

/// The single value of cookie `name`. Duplicates, such as a cookie tossed from a sibling
/// `workers.dev` subdomain next to ours, make the cookie unusable instead of ambiguous.
#[must_use]
pub fn cookie<'a>(header: &'a str, name: &str) -> Option<&'a str> {
    if header.len() > COOKIE_HEADER_BYTES_MAX {
        return None;
    }
    let mut found = None;
    for pair in header.split(';') {
        let Some((key, value)) = pair.trim().split_once('=') else {
            continue;
        };
        if key == name {
            if found.is_some() {
                return None;
            }
            found = Some(value);
        }
    }
    found
}

#[must_use]
pub fn state_cookie(state: &str) -> String {
    format!(
        "{STATE_COOKIE}={state}; Path=/auth/github; Max-Age=600; HttpOnly; Secure; SameSite=Lax"
    )
}

#[must_use]
pub fn session_cookie(value: &str) -> String {
    format!(
        "{SESSION_COOKIE}={value}; Path=/; Max-Age={SESSION_SECONDS}; HttpOnly; Secure; SameSite=Lax"
    )
}

/// Constant-time comparison of the callback's state with the state cookie.
#[must_use]
pub fn state_matches(cookie: &str, query: &str) -> bool {
    crate::valid_delivery(query)
        && cookie.len() == query.len()
        && cookie
            .bytes()
            .zip(query.bytes())
            .fold(0, |difference, (a, b)| difference | (a ^ b))
            == 0
}

/// Form body of the OAuth code exchange; `None` for a malformed code.
#[must_use]
pub fn oauth_exchange_body(client_secret: &str, code: &str) -> Option<String> {
    let valid = !client_secret.is_empty()
        && !code.is_empty()
        && code.len() <= 256
        && code
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_');
    valid.then(|| {
        format!(
            "client_id={CLIENT_ID}&client_secret={}&code={code}&redirect_uri={}",
            percent_encode(client_secret),
            percent_encode(&format!("{ROUTER_URL}{OAUTH_CALLBACK}"))
        )
    })
}

/// A bearer token from GitHub's exchange answer; error answers yield `None`.
#[must_use]
pub fn oauth_access_token(answer: &serde_json::Value) -> Option<&str> {
    answer
        .get("access_token")
        .and_then(serde_json::Value::as_str)
        .filter(|token| {
            !token.is_empty() && token.len() <= 4096 && token.bytes().all(|b| b.is_ascii_graphic())
        })
}

/// Decoded length of padded standard base64, `None` for anything else.
#[must_use]
pub fn base64_length(text: &str) -> Option<usize> {
    let bytes = text.as_bytes();
    if bytes.is_empty() || !bytes.len().is_multiple_of(4) {
        return None;
    }
    let padding = bytes.iter().rev().take_while(|&&b| b == b'=').count();
    if padding > 2 {
        return None;
    }
    let body = bytes.get(..bytes.len() - padding)?;
    if !body
        .iter()
        .all(|&b| b.is_ascii_alphanumeric() || b == b'+' || b == b'/')
    {
        return None;
    }
    Some(bytes.len() / 4 * 3 - padding)
}

#[must_use]
pub fn valid_key_id(key_id: &str) -> bool {
    !key_id.is_empty() && key_id.len() <= 64 && key_id.bytes().all(|b| b.is_ascii_alphanumeric())
}

/// The ops repository's Actions public key, a base64 X25519 key.
#[must_use]
pub fn public_key(answer: &serde_json::Value) -> Option<(&str, &str)> {
    let key_id = answer.get("key_id")?.as_str()?;
    let key = answer.get("key")?.as_str()?;
    (valid_key_id(key_id) && base64_length(key) == Some(32)).then_some((key_id, key))
}

fn provider(name: &str) -> Option<&'static str> {
    PROVIDERS.into_iter().find(|provider| *provider == name)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SeatUpload {
    provider: String,
    key_id: String,
    encrypted_value: String,
}

#[derive(Debug, PartialEq, Eq)]
pub struct SeatRegistration {
    pub provider: &'static str,
    pub key_id: String,
    pub encrypted_value: String,
}

/// Validates `POST /v1/seats`; error messages are fixed and never echo the request.
///
/// # Errors
/// A fixed reason when the body is too large, malformed, names an unknown provider or key, or
/// carries a value that cannot be a sealed GitHub secret.
pub fn seat_registration(body: &[u8]) -> Result<SeatRegistration, &'static str> {
    if body.len() > REGISTRATION_BODY_BYTES_MAX {
        return Err("request too large");
    }
    let upload: SeatUpload = serde_json::from_slice(body).map_err(|_| "invalid request")?;
    let provider = provider(&upload.provider).ok_or("unknown provider")?;
    if !valid_key_id(&upload.key_id) {
        return Err("invalid key id");
    }
    let length = base64_length(&upload.encrypted_value).ok_or("encrypted value is not base64")?;
    if length <= SEALED_OVERHEAD_BYTES || length > SECRET_BYTES_MAX + SEALED_OVERHEAD_BYTES {
        return Err("encrypted value size is invalid");
    }
    Ok(SeatRegistration {
        provider,
        key_id: upload.key_id,
        encrypted_value: upload.encrypted_value,
    })
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PreferredUpload {
    provider: String,
}

/// Validates `POST /v1/seats/preferred`.
///
/// # Errors
/// A fixed reason when the body is too large, malformed, or names an unknown provider.
pub fn preferred_registration(body: &[u8]) -> Result<&'static str, &'static str> {
    if body.len() > PREFERRED_BODY_BYTES_MAX {
        return Err("request too large");
    }
    let upload: PreferredUpload = serde_json::from_slice(body).map_err(|_| "invalid request")?;
    provider(&upload.provider).ok_or("unknown provider")
}

fn sign_in_help(provider: &str) -> &'static str {
    match provider {
        "anthropic" => {
            "Run <code>claude setup-token</code> and paste the token. It is valid for a year."
        }
        "openai-codex" => {
            "Sign in with <code>codex login</code> in a separate session used only for golems, then paste the content of <code>~/.codex/auth.json</code>. Runs refresh and write it back."
        }
        _ => {
            "Sign in to GitHub Copilot in a separate session used only for golems, then paste the content of the sign-in file. Runs refresh and write it back."
        }
    }
}

fn document(title: &str, body: &str) -> String {
    format!(
        "<!doctype html>\n<html lang=\"en\">\n<head>\n<meta charset=\"utf-8\">\n<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n<title>{title} · isp-golem-xiv</title>\n{body}</html>\n"
    )
}

/// The signed-in person's seats page. `login`, `seats` and `preferred` are validated
/// identifiers, and the CSRF token is hex, so nothing here needs HTML escaping.
#[must_use]
pub fn seats_page(login: &str, seats: &[&str], preferred: Option<&str>, csrf: &str) -> String {
    let mut body = format!(
        "<meta name=\"golem-csrf\" content=\"{csrf}\">\n<script src=\"/seats.js\" defer></script>\n</head>\n<body>\n<main>\n<h1>Seats</h1>\n<p>Signed in as <strong>@{login}</strong>. Only your own seats are shown.</p>\n<h2>Your seats</h2>\n"
    );
    if seats.is_empty() {
        body.push_str("<p>You have no seats yet.</p>\n");
    } else {
        body.push_str("<ul>\n");
        for seat in seats {
            let _ = writeln!(body, "<li data-seat=\"{seat}\"><code>{seat}</code></li>");
        }
        body.push_str("</ul>\n");
    }
    body.push_str("<h2>Register or replace a seat</h2>\n<p>Your sign-in is encrypted in this browser for the ops repository; the router only stores the ciphertext. Registering again replaces the previous sign-in.</p>\n");
    for provider in PROVIDERS {
        let _ = write!(
            body,
            "<form class=\"seat\" data-provider=\"{provider}\">\n<h3><code>{provider}</code></h3>\n<p>{}</p>\n<label>Sign-in for <code>{provider}</code><br><textarea name=\"credential\" rows=\"4\" cols=\"60\" required autocomplete=\"off\" spellcheck=\"false\"></textarea></label>\n<p><button type=\"submit\">Encrypt and register</button></p>\n</form>\n",
            sign_in_help(provider)
        );
    }
    body.push_str(
        "<h2>Preferred seat</h2>\n<p>Used when a golem's provider is not among your seats.</p>\n",
    );
    if seats.is_empty() {
        body.push_str("<p>Register a seat first.</p>\n");
    } else {
        body.push_str(
            "<form id=\"preferred\">\n<label>Preferred seat <select name=\"provider\">\n",
        );
        for seat in seats {
            let selected = if preferred == Some(*seat) {
                " selected"
            } else {
                ""
            };
            let _ = writeln!(body, "<option value=\"{seat}\"{selected}>{seat}</option>");
        }
        body.push_str("</select></label>\n<button type=\"submit\">Save</button>\n</form>\n");
    }
    body.push_str("<p id=\"status\" role=\"status\" aria-live=\"polite\"></p>\n</main>\n</body>\n");
    document("Seats", &body)
}

#[must_use]
pub fn admin_page(login: &str) -> String {
    document(
        "Admin",
        &format!(
            "</head>\n<body>\n<main>\n<h1>Admin</h1>\n<p>Signed in as <strong>@{login}</strong>.</p>\n<p>The admin dashboard arrives in Slice 4. Manage your seats on <a href=\"/seats\">the seats page</a>.</p>\n</main>\n</body>\n"
        ),
    )
}
