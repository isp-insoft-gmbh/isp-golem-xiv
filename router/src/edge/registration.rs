//! Sign-in, seat pages, and ciphertext relay. Credentials arrive sealed by the browser and are
//! forwarded unchanged to the ops repository; nothing here can decrypt, log, or echo them.
use serde_json::{Value, json};
use worker::{Env, Fetch, Headers, Method, Request, RequestInit, Response};

use super::{
    Github, Result, bounded, failure, has_seat, preferred_seat, random_state, read_stream,
    response_bytes, seats_of,
};
use crate::registration::{
    self as policy, CONTENT_SECURITY_POLICY, CSRF_HEADER, SESSION_COOKIE, STATE_COOKIE,
};
use crate::{OPS, ORGANIZATION, ROUTER_URL, seat_secret, valid_login};

const SCRIPT: &str = concat!(
    "/* TweetNaCl.js 1.0.3, https://github.com/dchest/tweetnacl-js\n",
    include_str!("../vendor/tweetnacl.LICENSE"),
    "*/\n",
    include_str!("../vendor/nacl.min.js"),
    "\n/* blakejs 1.2.1 blake2b.js, https://github.com/dcposch/blakejs\n",
    include_str!("../vendor/blakejs.LICENSE"),
    "*/\n(() => {\nconst module = {exports: {}};\n// Only blake2b()/blake2bHex() use util; the page calls blake2bInit/Update/Final.\nconst require = () => ({});\n",
    include_str!("../vendor/blake2b.js"),
    "\nself.golemBlake2b = module.exports;\n})();\n",
    include_str!("../seats.js"),
);

struct Session {
    key: [u8; 32],
    value: String,
    login: String,
}

fn now_s() -> u64 {
    worker::Date::now().as_millis() / 1000
}

fn session_key(env: &Env) -> Result<[u8; 32]> {
    let secret = env
        .secret("APP_CLIENT_SECRET")
        .map_err(|_| failure("sign-in not configured"))?
        .to_string();
    policy::session_key(&secret).ok_or_else(|| failure("sign-in not configured"))
}

fn session(request: &Request, env: &Env) -> Result<Option<Session>> {
    let cookies = request.headers().get("Cookie")?.unwrap_or_default();
    let Some(value) = policy::cookie(&cookies, SESSION_COOKIE) else {
        return Ok(None);
    };
    let key = session_key(env)?;
    Ok(
        policy::session_login(&key, value, now_s()).map(|login| Session {
            key,
            value: value.to_owned(),
            login: login.to_owned(),
        }),
    )
}

fn secured(mut response: Response, content_type: &str) -> Result<Response> {
    let headers = response.headers_mut();
    headers.set("Content-Type", content_type)?;
    headers.set("Content-Security-Policy", CONTENT_SECURITY_POLICY)?;
    headers.set("Cache-Control", "private, no-store")?;
    headers.set("X-Content-Type-Options", "nosniff")?;
    headers.set("Referrer-Policy", "no-referrer")?;
    Ok(response)
}

fn html(page: String) -> Result<Response> {
    secured(Response::ok(page)?, "text/html; charset=utf-8")
}

fn json_answer(value: &Value) -> Result<Response> {
    secured(Response::from_json(value)?, "application/json")
}

/// Without a session every page sends the browser to GitHub sign-in; state binds the callback.
fn sign_in() -> Result<Response> {
    let state = random_state();
    let Some(location) = crate::sign_in_location(&state) else {
        return Response::error("Sign-in unavailable", 503);
    };
    let headers = Headers::new();
    headers.set("Location", &location)?;
    headers.set("Set-Cookie", &policy::state_cookie(&state))?;
    headers.set("Cache-Control", "no-store")?;
    Ok(Response::empty()?.with_status(302).with_headers(headers))
}

/// Routes owned by registration; `None` leaves the request to other handlers.
pub(super) async fn route(
    request: &mut Request,
    env: &Env,
    path: &str,
) -> Option<Result<Response>> {
    let method = request.method();
    let result = match (method, path) {
        (Method::Get, "/seats.js") => Response::ok(SCRIPT)
            .and_then(|script| secured(script, "text/javascript; charset=utf-8")),
        (Method::Get, "/seats") => bounded(seats(request, env), 20_000).await,
        (Method::Get, "/admin") => admin(request, env),
        (Method::Get, "/auth/github/callback") => bounded(callback(request, env), 20_000).await,
        (Method::Get, "/v1/seats/key") => bounded(key(request, env), 20_000).await,
        (Method::Post, "/v1/seats") => bounded(register(request, env), 20_000).await,
        (Method::Post, "/v1/seats/preferred") => bounded(prefer(request, env), 20_000).await,
        _ => return None,
    };
    Some(result.or_else(|_| {
        worker::console_error!("registration request failed (details suppressed)");
        Response::error("Temporarily unavailable", 502)
    }))
}

async fn seats(request: &Request, env: &Env) -> Result<Response> {
    let Some(session) = session(request, env)? else {
        return sign_in();
    };
    let github = Github::installation(env).await?;
    let seats = seats_of(&github, &session.login).await?;
    let preferred = preferred_seat(&github, &session.login).await?;
    let csrf = policy::csrf_token(&session.key, &session.value)
        .ok_or_else(|| failure("csrf derivation failed"))?;
    html(policy::seats_page(
        &session.login,
        &seats,
        preferred.as_deref(),
        &csrf,
    ))
}

fn admin(request: &Request, env: &Env) -> Result<Response> {
    match session(request, env)? {
        Some(session) => html(policy::admin_page(&session.login)),
        None => sign_in(),
    }
}

fn query(request: &Request) -> Result<Option<(String, String)>> {
    let url = request.url()?;
    if url.query().is_some_and(|query| query.len() > 2048) {
        return Ok(None);
    }
    let (mut code, mut state) = (None, None);
    for (name, value) in url.query_pairs() {
        let slot = match name.as_ref() {
            "code" => &mut code,
            "state" => &mut state,
            _ => continue,
        };
        if slot.replace(value.into_owned()).is_some() {
            return Ok(None);
        }
    }
    Ok(code.zip(state))
}

async fn exchange(form: String) -> Result<Option<String>> {
    let headers = Headers::new();
    headers.set("Accept", "application/json")?;
    headers.set("Content-Type", "application/x-www-form-urlencoded")?;
    headers.set("User-Agent", crate::APP)?;
    let mut init = RequestInit::new();
    init.with_method(Method::Post)
        .with_headers(headers)
        .with_redirect(worker::RequestRedirect::Manual)
        .with_body(Some(form.into()));
    let request = Request::new_with_init("https://github.com/login/oauth/access_token", &init)?;
    let mut response = Fetch::Request(request).send().await?;
    if response.status_code() != 200 {
        return Err(failure("OAuth exchange failed"));
    }
    let bytes = response_bytes(&mut response, 16_384).await?;
    let answer: Value =
        serde_json::from_slice(&bytes).map_err(|_| failure("OAuth exchange invalid"))?;
    Ok(policy::oauth_access_token(&answer).map(str::to_owned))
}

/// The user token proves identity once and is dropped; only the signed login is kept.
async fn user_login(token: String) -> Result<Option<String>> {
    let user = Github::new(token);
    let (status, profile) = user.call(Method::Get, "/user", None).await?;
    if status != 200 {
        return Ok(None);
    }
    Ok(profile
        .get("login")
        .and_then(Value::as_str)
        .filter(|login| valid_login(login))
        .map(str::to_owned))
}

async fn callback(request: &Request, env: &Env) -> Result<Response> {
    let cookies = request.headers().get("Cookie")?.unwrap_or_default();
    let expected = policy::cookie(&cookies, STATE_COOKIE);
    let Some((code, state)) = query(request)? else {
        return Response::error("Sign-in failed; open /seats to start again.", 400);
    };
    if !expected.is_some_and(|expected| policy::state_matches(expected, &state)) {
        return Response::error(
            "Sign-in expired or was not started here; open /seats again.",
            400,
        );
    }
    let secret = env
        .secret("APP_CLIENT_SECRET")
        .map_err(|_| failure("sign-in not configured"))?
        .to_string();
    let key = policy::session_key(&secret).ok_or_else(|| failure("sign-in not configured"))?;
    let Some(form) = policy::oauth_exchange_body(&secret, &code) else {
        return Response::error("Sign-in failed; open /seats to start again.", 400);
    };
    let Some(token) = exchange(form).await? else {
        return Response::error(
            "GitHub did not confirm the sign-in; open /seats again.",
            403,
        );
    };
    let Some(login) = user_login(token).await? else {
        return Response::error(
            "GitHub did not confirm the sign-in; open /seats again.",
            403,
        );
    };
    let github = Github::installation(env).await?;
    if !github.member(&login).await? {
        return Response::error(
            format!("Only active {ORGANIZATION} members can sign in."),
            403,
        );
    }
    let value = policy::session_value(&key, &login, now_s())
        .ok_or_else(|| failure("session signing failed"))?;
    let headers = Headers::new();
    headers.set("Location", "/seats")?;
    headers.append("Set-Cookie", &policy::session_cookie(&value))?;
    headers.append("Set-Cookie", policy::STATE_COOKIE_CLEAR)?;
    headers.set("Cache-Control", "no-store")?;
    Ok(Response::empty()?.with_status(302).with_headers(headers))
}

async fn public_key(github: &Github) -> Result<(String, String)> {
    let answer = github
        .get(&format!("/repos/{OPS}/actions/secrets/public-key"))
        .await?;
    let (key_id, key) =
        policy::public_key(&answer).ok_or_else(|| failure("ops public key invalid"))?;
    Ok((key_id.to_owned(), key.to_owned()))
}

async fn key(request: &Request, env: &Env) -> Result<Response> {
    if session(request, env)?.is_none() {
        return Response::error("Sign in at /seats first", 401);
    }
    let github = Github::installation(env).await?;
    let (key_id, key) = public_key(&github).await?;
    json_answer(&json!({"key_id":key_id,"key":key}))
}

/// A state-changing request from our own page: same origin, a session, and its CSRF token.
/// The member check repeats per write because a session outlives a membership removal.
async fn writer(
    request: &mut Request,
    env: &Env,
    limit: usize,
) -> Result<std::result::Result<(Session, Github, Vec<u8>), Response>> {
    let origin = request.headers().get("Origin")?;
    if origin.as_deref() != Some(ROUTER_URL) {
        return Ok(Err(Response::error("Cross-origin request refused", 403)?));
    }
    let Some(session) = session(request, env)? else {
        return Ok(Err(Response::error("Sign in at /seats first", 401)?));
    };
    let submitted = request.headers().get(CSRF_HEADER)?.unwrap_or_default();
    if !policy::csrf_valid(&session.key, &session.value, &submitted) {
        return Ok(Err(Response::error(
            "Invalid CSRF token; reload the page",
            403,
        )?));
    }
    let declared = request.headers().get("Content-Length")?;
    if declared.is_some_and(|length| {
        length
            .parse::<usize>()
            .map_or(true, |length| length > limit)
    }) {
        return Ok(Err(Response::error("Request too large", 413)?));
    }
    let Ok(body) = bounded(read_stream(request.stream()?, limit), 5000).await else {
        return Ok(Err(Response::error("Request too large", 413)?));
    };
    let github = Github::installation(env).await?;
    if !github.member(&session.login).await? {
        return Ok(Err(Response::error(
            format!("Only active {ORGANIZATION} members can register seats."),
            403,
        )?));
    }
    Ok(Ok((session, github, body)))
}

async fn register(request: &mut Request, env: &Env) -> Result<Response> {
    let (session, github, body) =
        match writer(request, env, policy::REGISTRATION_BODY_BYTES_MAX).await? {
            Ok(accepted) => accepted,
            Err(refused) => return Ok(refused),
        };
    let registration = match policy::seat_registration(&body) {
        Ok(registration) => registration,
        Err(reason) => return Response::error(reason, 400),
    };
    let (key_id, _) = public_key(&github).await?;
    if registration.key_id != key_id {
        return Response::error(
            "The encryption key changed; reload the page and register again.",
            409,
        );
    }
    let name = seat_secret(&session.login, registration.provider)
        .ok_or_else(|| failure("invalid seat name"))?;
    // GitHub replaces an existing secret of the same name: registering again replaces the seat.
    let (status, _) = github
        .call(
            Method::Put,
            &format!("/repos/{OPS}/actions/secrets/{name}"),
            Some(json!({"encrypted_value":registration.encrypted_value,"key_id":key_id})),
        )
        .await?;
    if !matches!(status, 201 | 204) {
        return Err(failure("seat secret write failed"));
    }
    json_answer(&json!({"seat":name,"replaced":status == 204}))
}

async fn prefer(request: &mut Request, env: &Env) -> Result<Response> {
    let (session, github, body) =
        match writer(request, env, policy::PREFERRED_BODY_BYTES_MAX).await? {
            Ok(accepted) => accepted,
            Err(refused) => return Ok(refused),
        };
    let provider = match policy::preferred_registration(&body) {
        Ok(provider) => provider,
        Err(reason) => return Response::error(reason, 400),
    };
    if !has_seat(&github, &session.login, provider).await? {
        return Response::error("Register a seat at this provider first.", 409);
    }
    let name = crate::preferred_seat_variable(&session.login)
        .ok_or_else(|| failure("invalid variable name"))?;
    let variable = json!({"name":name,"value":provider});
    let (status, _) = github
        .call(
            Method::Patch,
            &format!("/repos/{OPS}/actions/variables/{name}"),
            Some(variable.clone()),
        )
        .await?;
    let status = if status == 404 {
        github
            .call(
                Method::Post,
                &format!("/repos/{OPS}/actions/variables"),
                Some(variable),
            )
            .await?
            .0
    } else {
        status
    };
    if !matches!(status, 201 | 204) {
        return Err(failure("preferred seat write failed"));
    }
    json_answer(&json!({"preferred":provider}))
}
