#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
use hmac::{Hmac, KeyInit, Mac};
use router::{
    Delivery, DispatchV1, parse_golem, parse_mention, refusal, seat_secret, valid_delivery,
    valid_repository, verify_signature,
};
use sha2::Sha256;
use std::fmt::Write;

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
