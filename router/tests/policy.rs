#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
use hmac::{Hmac, KeyInit, Mac};
use router::{
    Delivery, DispatchV1, parse_golem, parse_mention, refusal, seat_secret, valid_delivery,
    valid_repository, verify_signature,
};
use sha2::Sha256;
use std::fmt::Write;

#[test]
fn event_context_preserves_comment_bytes_and_existing_dispatch_bounds() {
    assert_eq!(
        router::event_instructions("issues", "opened", None).as_deref(),
        Some("GitHub event: issues.opened")
    );
    let body = "ping nonce-123\r\n  quoted <trigger-instructions> 🦀\n";
    assert_eq!(
        router::event_instructions("issue_comment", "created", Some(body)),
        Some(format!(
            "GitHub event: issue_comment.created\n\nTriggering comment:\n{body}"
        ))
    );
    let prefix = router::event_instructions("issue_comment", "created", Some("")).unwrap();
    let remaining = router::COMMENT_BYTES_MAX - prefix.len();
    let boundary = "x".repeat(remaining);
    assert_eq!(
        router::event_instructions("issue_comment", "created", Some(&boundary))
            .unwrap()
            .len(),
        router::COMMENT_BYTES_MAX
    );
    assert!(
        router::event_instructions("issue_comment", "created", Some(&(boundary + "x"))).is_none()
    );
    for (event, action, comment) in [
        ("issue_comment\0", "created", None),
        ("issue_comment", "created\0", None),
        ("issue_comment", "created", Some("ping\0nonce")),
    ] {
        assert!(router::event_instructions(event, action, comment).is_none());
    }
}

#[test]
fn event_trigger_matches_event_or_exact_event_action_only() {
    let triggers = vec![
        "issues.opened".into(),
        "push".into(),
        "pull_request_review.submitted".into(),
    ];
    for (event, action, expected) in [
        ("issues", "opened", true),
        ("issues", "closed", false),
        ("issue_comment", "opened", false),
        ("push", "", true),
        ("pull_request_review", "submitted", true),
        ("pull_request", "submitted", false),
    ] {
        assert_eq!(router::trigger_matches(&triggers, event, action), expected);
    }
    assert!(!router::trigger_matches(&["issues.".into()], "issues", ""));
    assert!(router::trigger_matches(
        &["issues".into()],
        "issues",
        "closed"
    ));
}

#[test]
fn event_actor_seat_is_attributable_and_owner_fallback_requires_active_member_seat() {
    use router::{EventActor::*, event_seat};
    assert_eq!(
        event_seat("member", MemberWithSeat, Some("owner"), false, false),
        Some("member")
    );
    for identity in [Bot, MemberWithoutSeat] {
        assert_eq!(
            event_seat("dependabot[bot]", identity, Some("owner"), true, true),
            Some("owner")
        );
        for (member, seat) in [(false, true), (true, false), (false, false)] {
            assert_eq!(
                event_seat("member", identity, Some("owner"), member, seat),
                None
            );
        }
        assert_eq!(event_seat("member", identity, None, true, true), None);
        assert_eq!(
            event_seat("member", identity, Some("../owner"), true, true),
            None
        );
    }
    assert_eq!(
        event_seat("outsider", Outsider, Some("owner"), true, true),
        None
    );
}

#[test]
fn schedules_choose_next_minute_on_matching_utc_day_without_midnight_catchup() {
    use router::{Schedule, next_schedule_minute, schedule_matches};
    let daily = Schedule::Daily("daily".into());
    assert_eq!(next_schedule_minute(&daily, 0), Some(1));
    assert_eq!(next_schedule_minute(&daily, 59_999), Some(1));
    assert_eq!(next_schedule_minute(&daily, 60_000), Some(2));
    assert_eq!(next_schedule_minute(&daily, 86_340_000), None);
    assert_eq!(next_schedule_minute(&daily, 86_400_000), Some(1441));
    // 1970-01-01 was Thursday, an independent calendar oracle.
    for (day, name) in [
        (0, "thu"),
        (1, "fri"),
        (2, "sat"),
        (3, "sun"),
        (4, "mon"),
        (5, "tue"),
        (6, "wed"),
    ] {
        let schedule = Schedule::Days(vec![name.into()]);
        assert!(schedule_matches(&schedule, day));
        assert!(!schedule_matches(&schedule, day + 1));
        assert_eq!(
            next_schedule_minute(&schedule, day * 86_400_000),
            Some(day * 1440 + 1)
        );
    }
    assert!(!schedule_matches(&Schedule::Daily("weekly".into()), 0));
    assert!(schedule_matches(&daily, u64::MAX));
    let id = router::occurrence_id("schedule-v2/99/hello/20000");
    assert!(valid_delivery(&id));
    assert_eq!(id, router::occurrence_id("schedule-v2/99/hello/20000"));
    assert_ne!(id, router::occurrence_id("schedule-v2/99/hello/20001"));
    assert_ne!(id, router::occurrence_id("schedule-v2/99/other/20000"));
}

fn queue_signature(key: &str, body: &[u8]) -> String {
    let mut mac = Hmac::<Sha256>::new_from_slice(key.as_bytes()).unwrap();
    mac.update(body);
    let mut signature = String::from("sha256=");
    for byte in mac.finalize().into_bytes() {
        write!(&mut signature, "{byte:02x}").unwrap();
    }
    signature
}

#[test]
fn queue_auth_binds_exact_body_normalized_pem_action_and_fresh_timestamp() {
    use router::verify_queue_signature;
    let body = br#"{"action":"bind","timestamp_ms":100000}"#;
    let signature = queue_signature("fixture\nPEM", body);
    assert!(verify_queue_signature(
        " \r\nfixture\r\nPEM\r\n",
        &signature,
        body,
        100_000
    ));
    for now in [40_000, 160_000] {
        assert!(verify_queue_signature(
            "fixture\nPEM",
            &signature,
            body,
            now
        ));
    }
    for now in [39_999, 160_001] {
        assert!(!verify_queue_signature(
            "fixture\nPEM",
            &signature,
            body,
            now
        ));
    }
    assert!(!verify_queue_signature("wrong", &signature, body, 100_000));
    assert!(!verify_queue_signature(
        "fixture\nPEM",
        &signature,
        br#"{ "action":"bind","timestamp_ms":100000}"#,
        100_000
    ));
    for body in [
        r#"{"action":"stop","timestamp_ms":100000}"#,
        r#"{"action":"enqueue","timestamp_ms":100000}"#,
        r#"{"action":"bind","timestamp_ms":-1}"#,
        r#"{"action":"bind","timestamp_ms":"100000"}"#,
        r#"{"action":"complete","timestamp_ms":100000}"#,
        r#"{"action":"complete","timestamp_ms":100000,"containers_destroyed":true,"publication_complete":false}"#,
    ] {
        assert!(!verify_queue_signature(
            "fixture",
            &queue_signature("fixture", body.as_bytes()),
            body.as_bytes(),
            100_000
        ));
    }
    let body = br#"{"action":"complete","timestamp_ms":100000,"containers_destroyed":true,"publication_complete":true}"#;
    assert!(verify_queue_signature(
        "fixture",
        &queue_signature("fixture", body),
        body,
        100_000
    ));
}

#[test]
fn unattended_payload_accepts_other_github_bots_but_rejects_app_and_outside_installation() {
    // Representative GitHub webhook schema, synthetic IDs; live captured bot proof remains external.
    let mut value = serde_json::json!({"action":"opened","sender":{"id":49_699_333,"login":"dependabot[bot]","type":"Bot"},"repository":{"id":99,"full_name":"isp-insoft-gmbh/isp-golem-xiv-test-fixture"},"installation":{"id":153_964_219},"issue":{"number":1}});
    assert!(
        serde_json::from_value::<router::EventDelivery>(value.clone())
            .unwrap()
            .accepted()
    );
    *value.pointer_mut("/sender/login").unwrap() = serde_json::json!(router::BOT);
    assert!(
        !serde_json::from_value::<router::EventDelivery>(value.clone())
            .unwrap()
            .accepted()
    );
    *value.pointer_mut("/sender/login").unwrap() = serde_json::json!("github-actions[bot]");
    assert!(
        serde_json::from_value::<router::EventDelivery>(value.clone())
            .unwrap()
            .accepted()
    );
    *value.pointer_mut("/installation/id").unwrap() = serde_json::json!(1);
    assert!(
        !serde_json::from_value::<router::EventDelivery>(value)
            .unwrap()
            .accepted()
    );
    let golem = parse_golem(
        "hello",
        &document("branch: golems/retained\nverify: cargo test\nowner: member\n"),
    );
    assert_eq!(golem.branch(), "golems/retained");
    assert_eq!(golem.frontmatter.verify.as_deref(), Some("cargo test"));
    assert_eq!(parse_golem("hello", &document("")).branch(), "golems/hello");
}

fn document(fields: &str) -> String {
    format!("---\n{fields}---\n\nRead the repository and reply.\n")
}

#[test]
fn mention_authority_is_first_line_with_exact_prefix_and_boundary() {
    let command = parse_mention("@isp-golem-xiv hello say 🦀\n@isp-golem-xiv other").unwrap();
    assert_eq!(command.golem, "hello");
    assert_eq!(command.instructions, "say 🦀\n@isp-golem-xiv other");
    assert_eq!(parse_mention("@isp-golem-xiv\r\ntext").unwrap().golem, "");
    for body in [
        "text\n@isp-golem-xiv hello",
        " @isp-golem-xiv hello",
        "@isp-golem-xiv-other hello",
        "@isp-golem-xivhello",
        "```\n@isp-golem-xiv hello",
        "@isp-golem-xiv hello bad\0text",
    ] {
        assert!(parse_mention(body).is_none(), "{body}");
    }
    assert!(parse_mention(&format!("@isp-golem-xiv hello {}", "x".repeat(16_384))).is_none());
}

#[test]
fn mention_context_preserves_multiline_bytes_without_promoting_later_commands() {
    for (body, golem, instructions) in [
        ("@isp-golem-xiv hello", "hello", ""),
        (
            "@isp-golem-xiv hello say 🦀\n\n  keep indentation\nlast \n",
            "hello",
            "say 🦀\n\n  keep indentation\nlast \n",
        ),
        (
            "@isp-golem-xiv hello\n  next line\n",
            "hello",
            "\n  next line\n",
        ),
        (
            "@isp-golem-xiv\thello\t say this\r\n\r\n  next\r\n",
            "hello",
            "say this\r\n\r\n  next\r\n",
        ),
        ("@isp-golem-xiv hello\r\nnext", "hello", "\r\nnext"),
        (
            "@isp-golem-xiv hello\n@isp-golem-xiv stop",
            "hello",
            "\n@isp-golem-xiv stop",
        ),
        ("@isp-golem-xiv", "", ""),
        ("@isp-golem-xiv\nhello say this", "", "\nhello say this"),
        ("@isp-golem-xiv \t \r\nhello", "", "\r\nhello"),
    ] {
        let mention = parse_mention(body).unwrap();
        assert_eq!(mention.golem, golem, "{body:?}");
        assert_eq!(mention.instructions, instructions, "{body:?}");
    }
}

#[test]
fn signed_bytes_are_verified_and_every_tampering_is_rejected() {
    let body = br#"{"action":"created"}"#;
    let mut mac = Hmac::<Sha256>::new_from_slice(b"test-only-secret").unwrap();
    mac.update(body);
    let mut hex = String::new();
    for byte in mac.finalize().into_bytes() {
        write!(&mut hex, "{byte:02x}").unwrap();
    }
    let signature = format!("sha256={hex}");
    assert!(verify_signature(b"test-only-secret", &signature, body));
    assert!(!verify_signature(b"wrong-secret", &signature, body));
    assert!(!verify_signature(
        b"test-only-secret",
        &signature,
        b"changed"
    ));
    for invalid in [
        "",
        "sha1=123",
        "sha256=bad",
        &format!("sha256={}", "z".repeat(64)),
    ] {
        assert!(!verify_signature(b"test-only-secret", invalid, body));
    }
    // RFC 4231 case 1, independent oracle for algorithm/hex handling.
    assert!(verify_signature(
        &[0x0b; 20],
        "sha256=b0344c61d8db38535ca8afceaf0bf12b881dc200c9833da726e9376c2e32cff7",
        b"Hi There"
    ));
}

#[test]
fn strict_frontmatter_preserves_valid_metadata_and_reports_disabled_entries() {
    let valid = parse_golem(
        "hello",
        &document(
            "harness: claude\nprovider: anthropic\nmodel: sonnet\ntimeout: 5h\nbranch: golems/hello\nverify: 'test -s file'\n",
        ),
    );
    assert!(valid.errors.is_empty(), "{:?}", valid.errors);
    assert_eq!(valid.timeout_seconds(), Some(18_000));
    assert_eq!(valid.frontmatter.model.as_deref(), Some("sonnet"));
    let empty = parse_golem("hi", &document(""));
    assert!(empty.errors.is_empty(), "{:?}", empty.errors);
    assert_eq!(empty.timeout_seconds(), Some(1800));
    for fields in [
        "unknown: value\n",
        "harness: claude\nharness: pi\n",
        "timeout: 30\n",
        "timeout: 6h\n",
        "timeout: 0m\n",
        "timeout: 9999999999999999h\n",
        "schedule: someday\n",
        "schedule: daily\n",
        "owner: person\nschedule: [mon, mon]\n",
        "owner: person\ntriggers: [issue_comment.created.extra]\n",
        "owner: person\ntriggers: [unknown.opened]\n",
        "harness: codex\nprovider: anthropic\n",
        "branch: ../escape\n",
        "branch: a.lock/b\n",
        "model: ''\n",
        "model: [a, b]\n",
        "verify: false\n",
        "model: '-unsafe'\n",
        "model: 'provider/model'\n",
        "owner: &x someone\nmodel: *x\n",
        "<<: {harness: claude}\n",
        "model: [unterminated\n",
    ] {
        let invalid = parse_golem("broken", &document(fields));
        assert!(!invalid.errors.is_empty(), "incorrectly enabled {fields}");
    }
    for name in ["stop", "seat", "help", "_golems", "../escape", "Hello", ""] {
        assert!(
            !parse_golem(name, &document("")).errors.is_empty(),
            "{name}"
        );
    }
    for invalid in [
        "Task without frontmatter".to_owned(),
        "---\nharness: claude\nTask".into(),
        "---\n---\n".into(),
        document(&"x".repeat(65_536)),
    ] {
        assert_ne!(parse_golem("hello", &invalid).errors, Vec::<String>::new());
    }
    assert_eq!(
        parse_golem(
            "scheduled",
            &document("owner: someone\nschedule: [mon, thu]\ntriggers: [issues.opened]\n")
        )
        .errors,
        Vec::<String>::new()
    );
    assert_eq!(
        parse_golem("hello", &document("").replace('\n', "\r\n")).errors,
        Vec::<String>::new()
    );
    assert_eq!(
        parse_golem(
            "hello",
            "---\n---\nDescribe the repository.\n\n---\nInclude a summary.\n"
        )
        .errors,
        Vec::<String>::new()
    );
    let prose = parse_golem("hello", "---\n---\ntimeout: 5h\n---\nTask\n");
    assert_eq!(prose.errors, Vec::<String>::new());
    assert_eq!(prose.timeout_seconds(), Some(1800));
}

#[test]
fn index_serialization_contains_names_and_validation_errors_without_task_content() {
    let entries = [
        parse_golem("hello", &document("")),
        parse_golem("stop", &document("")),
    ];
    let value = serde_json::to_value(&entries).unwrap();
    assert_eq!(value.pointer("/0/name"), Some(&serde_json::json!("hello")));
    assert_eq!(value.pointer("/0/errors"), Some(&serde_json::json!([])));
    assert_ne!(
        value.pointer("/1/errors").unwrap().as_array().unwrap(),
        &Vec::<serde_json::Value>::new()
    );
    assert!(!value.to_string().contains("Read the repository"));
}

#[test]
fn seat_names_are_safe_and_refusals_supply_required_markers_and_remediation() {
    assert_eq!(
        seat_secret("a-member", "anthropic").as_deref(),
        Some("SEAT_A_MEMBER__ANTHROPIC")
    );
    assert_eq!(
        seat_secret("a-member", "openai-codex").as_deref(),
        Some("SEAT_A_MEMBER__OPENAI_CODEX")
    );
    for login in [
        "../secrets",
        "member_bad",
        "-member",
        "member-",
        "a--b",
        "",
        "ü",
    ] {
        assert!(seat_secret(login, "anthropic").is_none());
    }
    assert!(seat_secret("member", "unknown").is_none());
    let golems = [
        parse_golem("hello", &document("")),
        parse_golem("disabled", &document("bad: value\n")),
    ];
    let unknown = refusal("unknown-golem", &golems);
    assert!(unknown.starts_with("<!-- isp-golem-xiv kind=refused reason=unknown-golem -->\n"));
    assert!(unknown.contains("hello"));
    assert!(!unknown.contains("disabled,"));
    assert!(refusal("no-seat", &[]).contains(router::SEAT_URL));
    assert!(refusal("not-member", &[]).contains("organization admin"));
}

#[test]
fn webhook_identity_prevents_app_loops_bots_and_mismatched_comment_authors() {
    let value = serde_json::json!({"action":"created","sender":{"id":42,"login":"member","type":"User"},"repository":{"id":99,"full_name":"isp-insoft-gmbh/isp-golem-xiv-test-unit"},"installation":{"id":153_964_219},"issue":{"number":1},"comment":{"id":100,"body":"@isp-golem-xiv hello","user":{"id":42,"login":"member","type":"User"}}});
    assert!(
        serde_json::from_value::<Delivery>(value.clone())
            .unwrap()
            .accepted()
    );
    for (pointer, replacement) in [
        ("/sender/login", serde_json::json!(router::BOT)),
        ("/sender/type", serde_json::json!("Bot")),
        ("/comment/user/id", serde_json::json!(7)),
        ("/action", serde_json::json!("edited")),
        ("/installation/id", serde_json::json!(1)),
        ("/repository/full_name", serde_json::json!("other-org/repo")),
        (
            "/comment/body",
            serde_json::json!("not a command\n@isp-golem-xiv hello"),
        ),
    ] {
        let mut invalid = value.clone();
        *invalid.pointer_mut(pointer).unwrap() = replacement;
        assert!(
            !serde_json::from_value::<Delivery>(invalid)
                .unwrap()
                .accepted(),
            "{pointer}"
        );
    }
}

#[test]
fn routing_identifiers_are_bounded_and_cannot_escape_api_paths() {
    assert!(valid_repository("isp-insoft-gmbh/project.name"));
    for repo in [
        "elsewhere/repo",
        "isp-insoft-gmbh/../ops",
        "isp-insoft-gmbh/repo?x=y",
        "isp-insoft-gmbh/..",
    ] {
        assert!(!valid_repository(repo));
    }
    assert!(valid_delivery("12345678-1234-1234-1234-123456789abc"));
    for delivery in [
        "",
        "../head",
        "tag.lock",
        "a/b",
        "a b",
        "not-a-uuid",
        "12345678-1234-1234-1234-123456789abz",
    ] {
        assert!(!valid_delivery(delivery));
    }
}

#[test]
fn dispatch_serialization_matches_version_one_runner_protocol() {
    let request = DispatchV1 {
        version: 1,
        delivery_id: "delivery-1",
        repository_id: 99,
        repository: "isp-insoft-gmbh/test",
        head_sha: "0123456789012345678901234567890123456789",
        golem: "hello",
        issue_number: 1,
        comment_id: 2,
        actor_id: 3,
        actor_login: "member",
        seat_login: "member",
        provider: "anthropic",
        harness: "claude",
        model: None,
        timeout_seconds: 1800,
        instructions: "say this",
        check_run_id: 4,
    };
    let value = serde_json::to_value(request).unwrap();
    assert_eq!(
        value,
        serde_json::json!({"version":1,"delivery_id":"delivery-1","repository_id":99,"repository":"isp-insoft-gmbh/test","head_sha":"0123456789012345678901234567890123456789","golem":"hello","issue_number":1,"comment_id":2,"actor_id":3,"actor_login":"member","seat_login":"member","provider":"anthropic","harness":"claude","model":null,"timeout_seconds":1800,"instructions":"say this","check_run_id":4})
    );
}
