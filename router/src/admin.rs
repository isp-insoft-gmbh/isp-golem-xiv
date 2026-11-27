//! Sanitized coordinator audit consumer. Request tags and check summaries never cross this boundary.
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{
    OPS, OUTCOMES, PROVIDERS, valid_delivery, valid_golem_name, valid_login, valid_repository,
};

pub const PAGE_SIZE: u64 = 10;
pub const PAGE_MAX: u64 = 100;
pub const WINDOW_SECONDS: u64 = 30 * 86_400;
/// Creation time of the first runner run with a sanitized audit (2026-11-13T12:51:43Z, run
/// 43428751065); every earlier run predates audits, every later one has one.
pub const AUDITED_SINCE: u64 = 1_794_574_303;

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Audit {
    pub version: u8,
    pub request_id: String,
    pub request_tag: String,
    pub generation: String,
    pub repository: String,
    pub repository_id: u64,
    pub golem: String,
    pub trigger: String,
    pub check_run_id: u64,
    pub run_id: Option<u64>,
    pub attempt: Option<u64>,
    pub seat: String,
    pub outcome: Option<String>,
    pub model: Option<String>,
    pub duration_s: Option<f64>,
    pub completed_at: String,
    pub log: Option<String>,
}

#[must_use]
pub fn sha(value: &str) -> bool {
    value.len() == 40
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

#[must_use]
pub fn model(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && !matches!(value, "unobserved" | "default")
        && value
            .as_bytes()
            .first()
            .is_some_and(u8::is_ascii_alphanumeric)
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"._:/+@-".contains(&b))
}

/// Strict UTC timestamps, including leap-year and day validation; no locale or runtime clock.
#[must_use]
pub fn timestamp(value: &str) -> Option<u64> {
    let bytes = value.as_bytes();
    if !matches!(bytes.len(), 20 | 24) {
        return None;
    }
    for (i, b) in bytes.iter().enumerate() {
        let valid = match i {
            4 | 7 => *b == b'-',
            10 => *b == b'T',
            13 | 16 => *b == b':',
            19 if bytes.len() == 24 => *b == b'.',
            19 | 23 => *b == b'Z',
            _ => b.is_ascii_digit(),
        };
        if !valid {
            return None;
        }
    }
    let number = |start, end| value.get(start..end)?.parse::<u64>().ok();
    let (year, month, day) = (number(0, 4)?, number(5, 7)?, number(8, 10)?);
    if !(1970..=9999).contains(&year) || !(1..=12).contains(&month) {
        return None;
    }
    let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let days = match month {
        2 => {
            if leap {
                29
            } else {
                28
            }
        }
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    };
    let (hour, minute, second) = (number(11, 13)?, number(14, 16)?, number(17, 19)?);
    if day == 0 || day > days || hour > 23 || minute > 59 || second > 59 {
        return None;
    }
    let y = year - u64::from(month <= 2);
    let era = y / 400;
    let yoe = y - era * 400;
    let mp = if month > 2 { month - 3 } else { month + 9 };
    let doy = (153 * mp + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    Some((era * 146_097 + doe - 719_468) * 86_400 + hour * 3600 + minute * 60 + second)
}

impl Audit {
    fn parse_tag(tag: &Value, expected_sha: &str) -> Option<Self> {
        let message = tag.get("message")?.as_str()?;
        if message.len() > 4096 || !sha(expected_sha) || tag.get("sha")?.as_str()? != expected_sha {
            return None;
        }
        let fields: Value = serde_json::from_str(message).ok()?;
        if fields.as_object()?.len() != 17 {
            return None;
        }
        let mut audit: Self = serde_json::from_value(fields).ok()?;
        // Early v3 audits copied the runner's placeholder; preserve immutable history
        // while making its lack of a model observation explicit to consumers.
        if audit.model.as_deref() == Some("default") {
            audit.model = None;
        }
        let (login, provider) = audit.seat.split_once('/')?;
        if audit.version != 3
            || !valid_delivery(&audit.request_id)
            || !valid_delivery(&audit.generation)
            || !sha(&audit.request_tag)
            || !valid_repository(&audit.repository)
            || !valid_golem_name(&audit.golem)
            || audit.repository_id == 0
            || audit.check_run_id == 0
            || !matches!(
                audit.trigger.as_str(),
                "mention" | "dashboard" | "event" | "schedule"
            )
            || !valid_login(login)
            || login != login.to_ascii_lowercase()
            || !PROVIDERS.contains(&provider)
            || audit
                .outcome
                .as_deref()
                .is_some_and(|o| !OUTCOMES.contains(&o))
            || audit.model.as_deref().is_some_and(|m| !model(m))
            || audit
                .duration_s
                .is_some_and(|d| !d.is_finite() || !(0.0..=2_678_400.0).contains(&d))
            || timestamp(&audit.completed_at).is_none()
            || tag.get("tag")?.as_str()? != format!("golem-admin/{}", audit.request_id)
            || tag.get("object")?.get("type")?.as_str()? != "tag"
            || tag.get("object")?.get("sha")?.as_str()? != audit.request_tag
        {
            return None;
        }
        Some(audit)
    }

    /// A ref and its annotated tag must agree with the bound Actions metadata.
    #[must_use]
    pub fn from_tag(tag: &Value, expected_sha: &str, run: &Value) -> Option<Self> {
        let audit = Self::parse_tag(tag, expected_sha)?;
        let id = audit.run_id?;
        let log = format!("https://github.com/{OPS}/actions/runs/{id}");
        if audit.log.as_deref() != Some(log.as_str()) {
            return None;
        }
        if id == 0
            || run.get("id")?.as_u64()? != id
            || audit.attempt.is_none_or(|a| a == 0)
            || audit.attempt != run.get("run_attempt").and_then(Value::as_u64)
            || run.get("status")?.as_str()? != "completed"
            || run.get("event")?.as_str()? != "workflow_dispatch"
            || run.get("repository")?.get("full_name")?.as_str()? != OPS
            || run.get("path")?.as_str()? != ".github/workflows/runner.yml"
            || run.get("display_title")?.as_str()?
                != format!("{} {}", audit.repository, audit.golem)
        {
            return None;
        }
        let created = timestamp(run.get("created_at")?.as_str()?)?;
        let completed = timestamp(&audit.completed_at)?;
        if completed < created {
            return None;
        }
        if let Some(duration) = audit.duration_s {
            let start = timestamp(run.get("run_started_at")?.as_str()?)?;
            // Millisecond check timestamps can add a fraction of a second.
            if start < created
                || completed < start
                || (duration - f64::from(u32::try_from(completed - start).ok()?)).abs() >= 1.0
            {
                return None;
            }
        }
        Some(audit)
    }

    /// Undispatched requests have no observed model, duration, attempt or Actions log.
    #[must_use]
    pub fn from_undispatched_tag(tag: &Value, expected_sha: &str) -> Option<Self> {
        let audit = Self::parse_tag(tag, expected_sha)?;
        if audit.run_id.is_some()
            || audit.attempt.is_some()
            || audit.log.is_some()
            || audit.model.is_some()
            || audit.duration_s.is_some()
            || audit
                .outcome
                .as_deref()
                .is_some_and(|o| !matches!(o, "blocked" | "stopped"))
        {
            return None;
        }
        Some(audit)
    }
}

#[must_use]
pub fn page(login: &str, csrf: &str) -> String {
    let login = escape(login);
    let csrf = escape(csrf);
    format!(include_str!("admin.html"), login = login, csrf = csrf)
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}
