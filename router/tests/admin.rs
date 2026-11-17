#![allow(clippy::unwrap_used, clippy::indexing_slicing)]
use router::admin::{Audit, page, timestamp};
use serde_json::{Value, json};

fn record() -> (Value, Value) {
    let run = json!({"id":43_409_045_310_u64,"run_attempt":1,"status":"completed","event":"workflow_dispatch","repository":{"full_name":"isp-insoft-gmbh/isp-golem-xiv-ops"},"path":".github/workflows/runner.yml","display_title":"isp-insoft-gmbh/isp-golem-xiv-test-nativeconfig-20261113090534 claudeconv","created_at":"2026-11-13T01:00:00Z","run_started_at":"2026-11-13T01:00:00Z"});
    // Native final-check vocabulary, projected into the coordinator's closed v3 schema.
    // IDs/name/model from the retained dashboard proof; fixture timestamps are deterministic.
    let audit = json!({"version":3,"request_id":"11111111-1111-4111-8111-111111111111","request_tag":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","generation":"22222222-2222-4222-8222-222222222222","repository":"isp-insoft-gmbh/isp-golem-xiv-test-nativeconfig-20261113090534","repository_id":99,"golem":"claudeconv","trigger":"dashboard","check_run_id":131_769_403_103_u64,"run_id":43_409_045_310_u64,"attempt":1,"seat":"bugabinga/anthropic","outcome":"replied","model":"claude-sonnet-5-5","duration_s":42,"completed_at":"2026-11-13T01:00:42Z","log":"https://github.com/isp-insoft-gmbh/isp-golem-xiv-ops/actions/runs/43409045310"});
    (audit, run)
}

fn tag(audit: &Value) -> Value {
    json!({"sha":"bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb","tag":"golem-admin/11111111-1111-4111-8111-111111111111","object":{"type":"tag","sha":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"},"message":audit.to_string()})
}

fn accepts(audit: &Value, run: &Value) -> bool {
    Audit::from_tag(&tag(audit), "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb", run).is_some()
}

#[test]
fn final_metadata_survives_retirement_without_request_or_queue_access() {
    let (mut audit, run) = record();
    assert!(accepts(&audit, &run));
    for outcome in [
        "changed",
        "replied",
        "no_change",
        "incomplete",
        "blocked",
        "stopped",
    ] {
        audit["outcome"] = json!(outcome);
        assert!(accepts(&audit, &run));
    }
    audit["outcome"] = json!("replied");
    audit["model"] = json!("default");
    assert_eq!(
        Audit::from_tag(
            &tag(&audit),
            "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
            &run
        )
        .unwrap()
        .model,
        None,
        "historical default means an unobserved model, not a model identifier"
    );
    audit["model"] = Value::Null;
    audit["outcome"] = Value::Null;
    audit["duration_s"] = Value::Null;
    assert!(accepts(&audit, &run), "unknown must remain representable");
}

#[test]
fn malformed_audits_and_wrong_identity_never_become_dashboard_data() {
    let (audit, run) = record();
    for (field, value) in [
        ("instructions", json!("do not expose raw request")),
        ("version", json!(2)),
        ("repository", json!("another-org/repo")),
        ("request_id", json!("not-a-uuid")),
        ("request_tag", json!("not-a-sha")),
        ("seat", json!("someone/unknown")),
        ("golem", json!("<script>")),
        ("trigger", json!("secret")),
        ("outcome", json!("success")),
        ("model", json!("requested model\ninstructions")),
        ("model", json!("unobserved")),
        ("duration_s", json!(-1)),
        ("duration_s", json!(43)),
        ("log", json!("https://attacker.invalid/")),
        ("attempt", Value::Null),
        ("completed_at", json!("2026-02-30T01:00:42Z")),
        ("completed_at", json!("2026-11-12T01:00:42Z")),
    ] {
        let mut changed = audit.clone();
        changed[field] = value;
        assert!(!accepts(&changed, &run), "invalid {field}");
    }
    for field in ["model", "outcome", "duration_s", "attempt"] {
        let mut changed = audit.clone();
        changed.as_object_mut().unwrap().remove(field);
        assert!(
            !accepts(&changed, &run),
            "missing {field} is malformed, not unknown"
        );
    }
    for (field, value) in [
        ("id", json!(1)),
        ("run_attempt", json!(2)),
        ("status", json!("in_progress")),
        ("event", json!("push")),
        ("path", json!("another.yml")),
        ("display_title", json!("another repository")),
    ] {
        let mut changed = run.clone();
        changed[field] = value;
        assert!(!accepts(&audit, &changed), "wrong Actions {field}");
    }
    let mut changed = tag(&audit);
    changed["object"]["sha"] = json!("cccccccccccccccccccccccccccccccccccccccc");
    assert!(Audit::from_tag(&changed, "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb", &run).is_none());
}

#[test]
fn undispatched_audits_require_absent_execution_metadata_and_preserve_unknown_outcome() {
    let (mut audit, run) = record();
    for field in ["run_id", "attempt", "log", "model", "duration_s"] {
        audit[field] = Value::Null;
    }
    audit["outcome"] = json!("stopped");
    let parse = |value: &Value| {
        Audit::from_undispatched_tag(&tag(value), "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb")
    };
    assert!(parse(&audit).is_some());
    assert!(
        !accepts(&audit, &run),
        "undispatched audits cannot masquerade as Actions runs"
    );
    audit["outcome"] = json!("blocked");
    assert!(parse(&audit).is_some());
    audit["outcome"] = Value::Null;
    assert_eq!(
        parse(&audit).unwrap().outcome,
        None,
        "null means no verified outcome, never inferred success or stopped"
    );
    for (field, value) in [
        ("run_id", json!(1)),
        ("attempt", json!(1)),
        (
            "log",
            json!("https://github.com/isp-insoft-gmbh/isp-golem-xiv-ops/actions/runs/1"),
        ),
        ("model", json!("requested-model")),
        ("duration_s", json!(0)),
    ] {
        let mut changed = audit.clone();
        changed[field] = value;
        assert!(
            parse(&changed).is_none(),
            "undispatched {field} is forbidden, including zero duration"
        );
    }
    for outcome in ["changed", "replied", "no_change", "incomplete", "success"] {
        audit["outcome"] = json!(outcome);
        assert!(
            parse(&audit).is_none(),
            "dispatched outcome {outcome} is impossible without execution"
        );
    }
}

#[test]
fn utc_dates_are_strict_and_page_escapes_untrusted_text() {
    assert_eq!(timestamp("1970-01-01T00:00:00Z"), Some(0));
    assert_eq!(timestamp("2024-02-29T12:30:01.001Z"), Some(1_709_209_801));
    for date in [
        "2026-02-29T00:00:00Z",
        "2024-04-31T00:00:00Z",
        "2024-01-01T24:00:00Z",
        "2024-01-01T00:60:00Z",
        "2024-01-01T00:00:60Z",
        "2024-01-01T00:00:00+00:00",
        "<script>",
    ] {
        assert_eq!(timestamp(date), None);
    }
    let page = page("<script>\"&'");
    assert!(!page.contains("<script>\""));
    assert!(page.contains("&lt;script&gt;&quot;&amp;&#39;"));
    for heading in ["Runs", "Usage", "Health", "Seats", "Repositories"] {
        assert!(page.contains(&format!(">{heading}</h2>")));
    }
}
