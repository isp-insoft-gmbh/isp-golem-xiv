//! Bounded organization dashboard and explicit onboarding. Never fetch credential values.
use serde_json::{Value, json};
use worker::{Env, Method, Request, Response};

use super::{Github, Result, failure, registration};
use crate::{OPS, ORGANIZATION, admin as policy, valid_repository};

pub(super) async fn page(request: &Request, env: &Env) -> Result<Response> {
    let Some(login) = registration::signed_login(request, env)? else {
        return registration::sign_in(true);
    };
    let github = Github::installation(env).await?;
    if !github.member(&login).await? {
        return denied(403);
    }
    let csrf =
        registration::csrf(request, env)?.ok_or_else(|| failure("missing dashboard session"))?;
    let mut response = registration::html(policy::page(&login, &csrf))?;
    response.headers_mut().set(
        "Content-Security-Policy",
        &format!(
            "{}; style-src 'self'",
            crate::registration::CONTENT_SECURITY_POLICY
        ),
    )?;
    Ok(response)
}

fn denied(status: u16) -> Result<Response> {
    registration::secured(
        Response::error("Organization membership required", status)?,
        "text/plain; charset=utf-8",
    )
}

async fn authenticated(request: &Request, env: &Env) -> Result<Option<(Github, String)>> {
    // Explicit bearer credentials never fall back to a cookie on failure.
    let login = if let Some(header) = request.headers().get("Authorization")? {
        let Some(token) = header.strip_prefix("Bearer ").filter(|t| {
            !t.is_empty() && t.len() <= 4096 && t.bytes().all(|b| b.is_ascii_graphic())
        }) else {
            return Ok(None);
        };
        registration::user_login(token.to_owned()).await?
    } else {
        registration::signed_login(request, env)?
    };
    let Some(login) = login else {
        return Ok(None);
    };
    let github = Github::installation(env).await?;
    if !github.member(&login).await? {
        return Ok(None);
    }
    Ok(Some((github, login)))
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct OnboardingRequest {
    repository: String,
}

pub(super) async fn onboarding(request: &mut Request, env: &Env) -> Result<Response> {
    if request.headers().get("Authorization")?.is_none()
        && registration::signed_login(request, env)?.is_none()
    {
        return registration::secured(
            Response::error("Sign in at /admin first", 401)?,
            "text/plain; charset=utf-8",
        );
    }
    let Some((github, login)) = authenticated(request, env).await? else {
        return denied(403);
    };
    if request.headers().get("Authorization")?.is_none()
        && registration::write_login(request, env)?.as_deref() != Some(login.as_str())
    {
        return denied(403);
    }
    if request.headers().get("Content-Type")?.as_deref() != Some("application/json") {
        return registration::secured(
            Response::error("JSON request required", 415)?,
            "text/plain; charset=utf-8",
        );
    }
    let declared = request.headers().get("Content-Length")?;
    if declared.is_some_and(|length| length.parse::<usize>().map_or(true, |length| length > 512)) {
        return registration::secured(
            Response::error("Request too large", 413)?,
            "text/plain; charset=utf-8",
        );
    }
    let Ok(body) = super::bounded(super::read_stream(request.stream()?, 512), 5000).await else {
        return registration::secured(
            Response::error("Request too large", 413)?,
            "text/plain; charset=utf-8",
        );
    };
    let Ok(input) = serde_json::from_slice::<OnboardingRequest>(&body) else {
        return registration::secured(
            Response::error("Invalid onboarding request", 400)?,
            "text/plain; charset=utf-8",
        );
    };
    if !valid_repository(&input.repository) {
        return registration::secured(
            Response::error("Invalid repository", 400)?,
            "text/plain; charset=utf-8",
        );
    }
    let Some(info) = github.repository(&input.repository).await? else {
        return registration::secured(
            Response::error("Repository unavailable", 404)?,
            "text/plain; charset=utf-8",
        );
    };
    let permission = github
        .get(&format!(
            "/repos/{}/collaborators/{login}/permission",
            input.repository
        ))
        .await?;
    if !matches!(
        permission.get("permission").and_then(Value::as_str),
        Some("admin" | "write")
    ) {
        return denied(403);
    }
    let (status, result) = super::onboard(&github, &input.repository, &info).await?;
    Ok(registration::json_answer(&result)?.with_status(status))
}

/// Fixed numeric cursors, bound to a minute-rounded window. No caller-controlled API paths.
fn cursor(request: &Request) -> Option<(u64, u64, Option<String>)> {
    let url = request.url().ok()?;
    if url.query().is_some_and(|q| q.len() > 512) {
        return None;
    }
    let now = worker::Date::now().as_millis() / 60_000 * 60;
    let (mut page, mut as_of, mut after) = (None, None, None);
    for (key, value) in url.query_pairs() {
        if key == "after" {
            if !valid_cursor(&value) || after.replace(value.into_owned()).is_some() {
                return None;
            }
            continue;
        }
        let slot = match key.as_ref() {
            "page" => &mut page,
            "as_of" => &mut as_of,
            _ => return None,
        };
        if slot.replace(value.parse::<u64>().ok()?).is_some() {
            return None;
        }
    }
    let page = page.unwrap_or(1);
    let as_of = as_of.unwrap_or(now);
    if page == 0
        || page > policy::PAGE_MAX
        || as_of < policy::WINDOW_SECONDS
        || as_of > now
        || now - as_of > 3600
        || as_of % 60 != 0
    {
        return None;
    }
    Some((page, as_of, after))
}

pub(super) async fn data(request: &Request, env: &Env, section: &str) -> Result<Response> {
    let Some((github, _)) = authenticated(request, env).await? else {
        return denied(403);
    };
    let Some((page, as_of, after)) =
        cursor(request).filter(|(_, _, after)| section == "undispatched" || after.is_none())
    else {
        return registration::secured(
            Response::error("Invalid or expired pagination; reload dashboard", 400)?,
            "text/plain; charset=utf-8",
        );
    };
    let value = match section {
        "runs" => runs(&github, page, as_of).await?,
        "undispatched" => undispatched(&github, page, as_of, after.as_deref()).await?,
        "seats" => seats(&github).await?,
        "repositories" => repositories(&github, page).await?,
        _ => return Err(failure("invalid admin section")),
    };
    registration::json_answer(&value)
}

fn valid_cursor(value: &str) -> bool {
    !value.is_empty() && value.len() <= 256 && value.bytes().all(|b| b.is_ascii_graphic())
}

// GitHub's RefConnection paginates the sanitized namespace, including no-run stops.
// Only the immediate annotated tag's message is selected; never the target request's message.
async fn undispatched(
    github: &Github,
    page: u64,
    as_of: u64,
    after: Option<&str>,
) -> Result<Value> {
    let query = r#"query($after: String) { repository(owner: "isp-insoft-gmbh", name: "isp-golem-xiv-ops") { refs(refPrefix: "refs/tags/golem-admin/", first: 10, after: $after) { nodes { name target { __typename oid ... on Tag { name message target { __typename oid } } } } pageInfo { hasNextPage endCursor } } } }"#;
    let result = github
        .post(
            "/graphql",
            json!({"query":query,"variables":{"after":after}}),
        )
        .await?;
    if result
        .get("errors")
        .is_some_and(|e| !e.as_array().is_some_and(Vec::is_empty))
    {
        return Err(failure("audit listing unavailable"));
    }
    let refs = result
        .get("data")
        .and_then(|v| v.get("repository"))
        .and_then(|v| v.get("refs"))
        .ok_or_else(|| failure("audit refs unavailable"))?;
    let nodes = refs
        .get("nodes")
        .and_then(Value::as_array)
        .filter(|n| n.len() <= 10)
        .ok_or_else(|| failure("invalid audit refs"))?;
    let info = refs
        .get("pageInfo")
        .ok_or_else(|| failure("invalid audit pagination"))?;
    let more = info
        .get("hasNextPage")
        .and_then(Value::as_bool)
        .ok_or_else(|| failure("invalid audit pagination"))?;
    let next = if more {
        Some(
            info.get("endCursor")
                .and_then(Value::as_str)
                .filter(|s| valid_cursor(s) && Some(*s) != after)
                .ok_or_else(|| failure("invalid audit cursor"))?,
        )
    } else {
        None
    };
    let mut records = Vec::new();
    let mut malformed = 0;
    for node in nodes {
        let Some(tag) = graph_tag(node) else {
            malformed += 1;
            continue;
        };
        let Some(sha) = tag.get("sha").and_then(Value::as_str) else {
            malformed += 1;
            continue;
        };
        // Dispatched audits are served through the Actions identity-checked path, not duplicated.
        let message = tag
            .get("message")
            .and_then(Value::as_str)
            .filter(|s| s.len() <= 4096)
            .and_then(|s| serde_json::from_str::<Value>(s).ok());
        if message
            .as_ref()
            .and_then(|v| v.get("run_id"))
            .is_some_and(Value::is_u64)
        {
            continue;
        }
        let Some(audit) = policy::Audit::from_undispatched_tag(&tag, sha) else {
            malformed += 1;
            continue;
        };
        let Some(completed) = policy::timestamp(&audit.completed_at) else {
            malformed += 1;
            continue;
        };
        if completed < as_of - policy::WINDOW_SECONDS || completed > as_of {
            continue;
        }
        records.push(json!({"request_id":audit.request_id,"repo":audit.repository,"golem":audit.golem,"trigger":audit.trigger,"seat":audit.seat,"outcome":audit.outcome,"model":null,"duration_s":null,"log":null,"run_id":null,"completed_at":audit.completed_at}));
    }
    Ok(
        json!({"runs":records,"page":page,"as_of":as_of,"next_page":(more && page < policy::PAGE_MAX).then_some(page+1),"after":if page < policy::PAGE_MAX {next} else {None},"listing_complete":!more,"malformed_audits":malformed,"coverage_note":"Undispatched requests by terminal check time; no agent execution or Actions log. Traverses the sanitized audit namespace, ten tags per request, at most 100 pages; partial until fully traversed. Historical missing audits need verified backfill."}),
    )
}

fn graph_tag(node: &Value) -> Option<Value> {
    let target = node.get("target")?;
    if target.get("__typename")?.as_str()? != "Tag" {
        return None;
    }
    let name = target.get("name")?.as_str()?;
    if node.get("name")?.as_str()? != name.strip_prefix("golem-admin/")?
        && node.get("name")?.as_str()? != name
    {
        return None;
    }
    Some(
        json!({"sha":target.get("oid")?,"tag":name,"message":target.get("message")?,"object":{"type":if target.get("target")?.get("__typename")?.as_str()? == "Tag" {"tag"} else {"invalid"},"sha":target.get("target")?.get("oid")?}}),
    )
}

fn workflow_conclusion(run: &Value, as_of: u64) -> Result<Option<&str>> {
    let created = run
        .get("created_at")
        .and_then(Value::as_str)
        .and_then(policy::timestamp)
        .ok_or_else(|| failure("invalid run date"))?;
    if created < as_of - policy::WINDOW_SECONDS || created > as_of {
        return Err(failure("run outside requested window"));
    }
    if run.get("event").and_then(Value::as_str) != Some("workflow_dispatch")
        || run.get("path").and_then(Value::as_str) != Some(".github/workflows/runner.yml")
        || run
            .get("repository")
            .and_then(|v| v.get("full_name"))
            .and_then(Value::as_str)
            != Some(OPS)
    {
        return Err(failure("invalid workflow identity"));
    }
    let status = run
        .get("status")
        .and_then(Value::as_str)
        .filter(|s| {
            matches!(
                *s,
                "completed" | "queued" | "in_progress" | "waiting" | "pending" | "requested"
            )
        })
        .ok_or_else(|| failure("invalid workflow status"))?;
    if status != "completed" {
        return Ok(None);
    }
    let conclusion = run
        .get("conclusion")
        .and_then(Value::as_str)
        .filter(|c| {
            matches!(
                *c,
                "success"
                    | "failure"
                    | "cancelled"
                    | "timed_out"
                    | "action_required"
                    | "neutral"
                    | "skipped"
                    | "startup_failure"
                    | "stale"
            )
        })
        .ok_or_else(|| failure("invalid workflow conclusion"))?;
    Ok(Some(conclusion))
}

async fn runs(github: &Github, page: u64, as_of: u64) -> Result<Value> {
    let from = crate::utc_minute(as_of - policy::WINDOW_SECONDS).replace('Z', ":00Z");
    let to = crate::utc_minute(as_of).replace('Z', ":00Z");
    let list = github.get(&format!("/repos/{OPS}/actions/workflows/runner.yml/runs?per_page={}&page={page}&created={from}..{to}", policy::PAGE_SIZE)).await?;
    let entries = list
        .get("workflow_runs")
        .and_then(Value::as_array)
        .ok_or_else(|| failure("invalid run listing"))?;
    let total = list
        .get("total_count")
        .and_then(Value::as_u64)
        .ok_or_else(|| failure("invalid run total"))?;
    if entries.len() > usize::try_from(policy::PAGE_SIZE).map_err(|_| failure("invalid bound"))?
        || total < entries.len() as u64
    {
        return Err(failure("invalid run listing"));
    }
    let mut records = Vec::new();
    let mut workflow_health = Vec::new();
    let (mut missing, mut malformed, mut pending) = (0, 0, 0);
    for run in entries {
        let id = run
            .get("id")
            .and_then(Value::as_u64)
            .filter(|id| *id > 0)
            .ok_or_else(|| failure("invalid run id"))?;
        let conclusion = workflow_conclusion(run, as_of)?;
        workflow_health.push(json!({"run_id":id,"conclusion":conclusion}));
        if conclusion.is_none() {
            pending += 1;
            continue;
        }
        let (status, reference) = github
            .call(
                Method::Get,
                &format!("/repos/{OPS}/git/ref/tags/golem-run/{id}"),
                None,
            )
            .await?;
        if status == 404 {
            missing += 1;
            continue;
        }
        if status != 200 {
            return Err(failure("audit reference unavailable"));
        }
        let Some(sha) = reference
            .get("object")
            .and_then(|v| v.get("sha"))
            .and_then(Value::as_str)
            .filter(|s| policy::sha(s))
        else {
            malformed += 1;
            continue;
        };
        if reference.get("ref").and_then(Value::as_str)
            != Some(format!("refs/tags/golem-run/{id}").as_str())
            || reference
                .get("object")
                .and_then(|v| v.get("type"))
                .and_then(Value::as_str)
                != Some("tag")
        {
            malformed += 1;
            continue;
        }
        let tag = github.get(&format!("/repos/{OPS}/git/tags/{sha}")).await?;
        let Some(audit) = policy::Audit::from_tag(&tag, sha, run) else {
            malformed += 1;
            continue;
        };
        records.push(json!({"repo":audit.repository,"golem":audit.golem,"trigger":audit.trigger,"seat":audit.seat,"model":audit.model,"outcome":audit.outcome,"duration_s":audit.duration_s,"log":audit.log,"run_id":audit.run_id,"completed_at":audit.completed_at}));
    }
    let traversed = page * policy::PAGE_SIZE;
    let next = (traversed < total && page < policy::PAGE_MAX).then_some(page + 1);
    Ok(
        json!({"runs":records,"workflow_health":workflow_health,"page":page,"as_of":as_of,"from":from,"to":to,"total_workflow_runs":total,"next_page":next,"listing_complete":traversed >= total,"page_complete":missing == 0 && malformed == 0 && pending == 0,"missing_audits":missing,"malformed_audits":malformed,"pending":pending,"history_complete":page == 1 && traversed >= total && missing == 0 && malformed == 0 && pending == 0,"coverage_note":"Actions runs created in this window only. Previously retired runs without sanitized audits need verified backfill; no history is fabricated. Undispatched requests have a separate paginated audit listing at /v1/admin/undispatched. GitHub search caps traversal at 1000 runs; changing upstream data can shift pages."}),
    )
}

async fn seats(github: &Github) -> Result<Value> {
    // Names only. Never individual secret endpoints, values, timestamps, or credential parsing.
    let listed = github
        .get(&format!("/repos/{OPS}/actions/secrets?per_page=100"))
        .await?;
    let secrets = listed
        .get("secrets")
        .and_then(Value::as_array)
        .ok_or_else(|| failure("invalid seats"))?;
    let total = listed
        .get("total_count")
        .and_then(Value::as_u64)
        .ok_or_else(|| failure("invalid seats total"))?;
    if total > 100 || total != secrets.len() as u64 {
        return Err(failure("seat listing incomplete"));
    }
    let mut names = Vec::new();
    for secret in secrets {
        names.push(
            secret
                .get("name")
                .and_then(Value::as_str)
                .filter(|n| {
                    n.len() <= 100
                        && n.bytes()
                            .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit() || b == b'_')
                })
                .ok_or_else(|| failure("invalid secret name"))?,
        );
    }
    let mut members = Vec::new();
    let mut complete = false;
    for page in 1..=3 {
        let list = github
            .get(&format!(
                "/orgs/{ORGANIZATION}/members?per_page=100&page={page}"
            ))
            .await?;
        let list = list
            .as_array()
            .filter(|v| v.len() <= 100)
            .ok_or_else(|| failure("invalid members"))?;
        for member in list {
            members.push(
                member
                    .get("login")
                    .and_then(Value::as_str)
                    .filter(|m| crate::valid_login(m))
                    .ok_or_else(|| failure("invalid member"))?
                    .to_owned(),
            );
        }
        if list.len() < 100 {
            complete = true;
            break;
        }
    }
    let refs: Vec<_> = members.iter().map(String::as_str).collect();
    let holders = crate::seat_holders(&names, &refs);
    let mut rows = Vec::new();
    for (member, entries) in [(true, holders.registered), (false, holders.departed)] {
        for (login, providers) in entries {
            for provider in providers {
                rows.push(json!({"person":login,"provider":provider,"membership":if member {"active"} else if complete {"left organization"} else {"unknown"},"identity":if member {"GitHub login"} else {"normalized secret-name identifier; not reversible"},"refreshing":provider == "openai-codex","expires_at":null}));
            }
        }
    }
    Ok(
        json!({"seats":rows,"complete":complete,"expiry_note":"Expiry and refresh health unknown: secret names contain no expiry metadata. Update time is not expiry."}),
    )
}

async fn repositories(github: &Github, page: u64) -> Result<Value> {
    let list = github
        .get(&format!(
            "/installation/repositories?per_page=1&page={page}"
        ))
        .await?;
    let entries = list
        .get("repositories")
        .and_then(Value::as_array)
        .filter(|e| e.len() <= 1)
        .ok_or_else(|| failure("invalid installations"))?;
    let total = list
        .get("total_count")
        .and_then(Value::as_u64)
        .ok_or_else(|| failure("invalid installation total"))?;
    let mut rows = Vec::new();
    for repository in entries {
        let name = repository
            .get("full_name")
            .and_then(Value::as_str)
            .filter(|r| valid_repository(r))
            .ok_or_else(|| failure("invalid repository"))?;
        let id = repository
            .get("id")
            .and_then(Value::as_u64)
            .filter(|id| *id > 0)
            .ok_or_else(|| failure("invalid repository id"))?;
        // App installation access is sufficient; `.golems/` determines whether a repo is enabled.
        let metadata = async {
            let head = github.head(name, repository).await?;
            if !policy::sha(&head) {
                return Err(failure("invalid repository head"));
            }
            let (status, _) = github
                .call(
                    Method::Get,
                    &format!("/repos/{name}/contents/.golems?ref={head}"),
                    None,
                )
                .await?;
            if !matches!(status, 200 | 404) {
                return Err(failure("repository index unavailable"));
            }
            let golems = if status == 200 {
                github.index(name, &head).await?
            } else {
                Vec::new()
            };
            Ok::<_, worker::Error>((status == 200, golems))
        }
        .await;
        let (enabled, golems, metadata_status) = match metadata {
            Ok((enabled, golems)) => (Some(enabled), golems, "available"),
            Err(_) => (None, Vec::new(), "unavailable"),
        };
        let golems: Vec<_> = golems.into_iter().map(|g| json!({"name":if crate::valid_golem_name(&g.name) {g.name} else {"invalid filename".into()},"enabled":g.errors.is_empty(),"reason":if g.errors.is_empty() {None} else {Some("invalid definition; inspect .golems/ in GitHub")}})).collect();
        rows.push(json!({"repo":name,"repository_id":id,"enabled":enabled,"metadata_status":metadata_status,"golems":golems}));
    }
    Ok(
        json!({"repositories":rows,"page":page,"total_repositories":total,"next_page":(page < total && page < policy::PAGE_MAX).then_some(page+1),"complete":page >= total,"coverage_note":"One installed repository per request; at most 100 pages. No mutations or dispatches."}),
    )
}
