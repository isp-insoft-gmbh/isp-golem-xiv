#![allow(clippy::unwrap_used, clippy::indexing_slicing)] // Malformed fixtures should fail loudly.
use super::*;
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use serde_json::json;

/// JWT-shaped like real `ChatGPT` tokens: RS256 header, ~1 KB of claims, 256-byte signature.
fn jwt(subject: &str, fill: u8) -> String {
    let claims = json!({"sub":subject, "email":"member@example.invalid", "exp":1_900_000_000,
        "https://api.openai.com/auth":{"chatgpt_plan_type":"plus", "chatgpt_account_id":"acct-fixture",
        "chatgpt_user_id":"user-fixture", "organizations":[{"id":"org-fixture","title":"x".repeat(1100)}]}});
    format!(
        "{}.{}.{}",
        URL_SAFE_NO_PAD.encode(br#"{"alg":"RS256","typ":"JWT","kid":"fixture"}"#),
        URL_SAFE_NO_PAD.encode(claims.to_string()),
        URL_SAFE_NO_PAD.encode([fill; 256])
    )
}

/// A pretty-printed, realistically sized `auth.json`; the refresh token identifies the generation.
pub(crate) fn auth_json(refresh_token: &str, last_refresh: &str) -> String {
    let fill = refresh_token.bytes().fold(0_u8, u8::wrapping_add);
    serde_json::to_string_pretty(&json!({
        "auth_mode": "chatgpt",
        "OPENAI_API_KEY": null,
        "tokens": {
            "id_token": jwt("id", fill),
            "access_token": jwt("access", fill),
            "refresh_token": format!("rt_{refresh_token}-fixture-not-real"),
            "account_id": "acct-fixture"
        },
        "last_refresh": last_refresh,
        "agent_identity": null
    }))
    .unwrap()
}

fn auth(refresh_token: &str, last_refresh: &str) -> CodexAuth {
    CodexAuth::parse(&auth_json(refresh_token, last_refresh)).unwrap()
}

fn edited(text: &str, edit: impl FnOnce(&mut serde_json::Value)) -> String {
    let mut value: serde_json::Value = serde_json::from_str(text).unwrap();
    edit(&mut value);
    serde_json::to_string_pretty(&value).unwrap()
}

#[test]
fn realistic_sign_ins_parse_and_api_key_or_malformed_sign_ins_do_not() {
    let text = auth_json("one", "2026-11-01T00:00:00Z");
    assert!(
        (4096..AUTH_BYTES_MAX).contains(&text.len()),
        "{}",
        text.len()
    );
    assert_eq!(CodexAuth::parse(&text).unwrap().text(), text);
    let api_key_only =
        json!({"auth_mode":"apikey","OPENAI_API_KEY":"sk-proj-fixture-key-000000","tokens":null})
            .to_string();
    for invalid in [
        api_key_only,
        edited(&text, |value| value["auth_mode"] = json!("apikey")),
        edited(&text, |value| {
            value["tokens"]["refresh_token"] = json!("short");
        }),
        edited(&text, |value| {
            value["tokens"]["access_token"] = json!(format!("{} x", "a".repeat(40)));
        }),
        edited(&text, |value| {
            value["tokens"]
                .as_object_mut()
                .unwrap()
                .remove("id_token")
                .map(drop)
                .unwrap();
        }),
        edited(&text, |value| value["tokens"]["account_id"] = json!("")),
        edited(&text, |value| value["last_refresh"] = json!("bad\ntime")),
        format!("{text} trailing"),
        "not json".into(),
        format!("{{\"padding\":\"{}\"}}", "x".repeat(AUTH_BYTES_MAX)),
    ] {
        assert!(
            CodexAuth::parse(&invalid).is_err(),
            "{}",
            &invalid[..invalid.len().min(80)]
        );
    }
    // Codex owns its format: unknown fields are kept, not rejected.
    let extended = edited(&text, |value| {
        value["tokens"]["oauth_provenance"] = json!({"issuer":"x"});
    });
    assert!(CodexAuth::parse(&extended).is_ok());
}

#[test]
fn every_sign_in_value_is_guarded_on_its_own() {
    let auth = auth("one", "2026-11-01T00:00:00Z");
    let mut guard = OutputGuard::default();
    auth.guard(&mut guard);
    let value: serde_json::Value = serde_json::from_str(auth.text()).unwrap();
    for key in ["id_token", "access_token", "refresh_token"] {
        let token = value["tokens"][key].as_str().unwrap();
        assert!(
            guard.check(&format!("leaked: {token} end")).is_err(),
            "{key}"
        );
        assert!(
            guard
                .check(&base64::engine::general_purpose::STANDARD.encode(token))
                .is_err(),
            "{key} encoded"
        );
    }
    assert!(guard.check(auth.text()).is_err());
    assert!(guard.check("an ordinary answer").is_ok());
    let mut keyed = OutputGuard::default();
    let with_key = edited(auth.text(), |value| {
        value["OPENAI_API_KEY"] = json!("sk-proj-fixture-api-key-0000");
    });
    CodexAuth::parse(&with_key).unwrap().guard(&mut keyed);
    assert!(keyed.check("sk-proj-fixture-api-key-0000").is_err());
    // A run's leftover file is guarded even when it is not a valid sign-in.
    let mut leftover = OutputGuard::default();
    guard_text("tampered-sign-in-content", &mut leftover);
    assert!(leftover.check("tampered-sign-in-content").is_err());
}

#[test]
fn write_back_only_refreshed_valid_sign_ins_of_the_same_account() {
    let original = auth("one", "2026-11-01T00:00:00Z");
    let decide = |after: Result<String>| refresh(&original, after);
    assert!(matches!(
        decide(Ok(original.text().to_owned())),
        Refresh::Unchanged
    ));
    let compact =
        serde_json::to_string(&serde_json::from_str::<serde_json::Value>(original.text()).unwrap())
            .unwrap();
    assert!(
        matches!(decide(Ok(compact)), Refresh::Unchanged),
        "formatting is not a refresh"
    );
    let refreshed = auth_json("two", "2026-11-11T12:00:00.123456Z");
    assert!(
        matches!(decide(Ok(refreshed.clone())), Refresh::Refreshed(auth) if auth.text() == refreshed),
        "refreshed sign-in must be written back"
    );
    for (after, expected) in [
        (
            Err("refreshed sign-in missing"),
            "refreshed sign-in missing",
        ),
        (
            Ok("{}".to_owned()),
            "refreshed sign-in is not a valid Codex sign-in",
        ),
        (
            Ok(edited(&refreshed, |value| {
                value["tokens"]["refresh_token"] = json!("x");
            })),
            "refreshed sign-in is not a valid Codex sign-in",
        ),
        (
            Ok(edited(&refreshed, |value| {
                value["tokens"]["account_id"] = json!("acct-other");
            })),
            "refreshed sign-in belongs to another account",
        ),
        (
            Ok(auth_json("two", "2026-11-01T00:00:00Z")),
            "refreshed sign-in has no new refresh time",
        ),
        (
            Ok(edited(&refreshed, |value| {
                value
                    .as_object_mut()
                    .unwrap()
                    .remove("last_refresh")
                    .map(drop)
                    .unwrap();
            })),
            "refreshed sign-in has no new refresh time",
        ),
    ] {
        assert!(
            matches!(decide(after), Refresh::Invalid(error) if error == expected),
            "{expected}"
        );
    }
}

#[test]
fn private_home_holds_only_the_sign_in_and_is_removed_with_its_contents() {
    let original = auth("one", "2026-11-01T00:00:00Z");
    let home = private_home(&original).unwrap();
    let path = home.path().to_owned();
    let entries: Vec<_> = std::fs::read_dir(&path)
        .unwrap()
        .map(|entry| entry.unwrap().file_name())
        .collect();
    assert_eq!(entries, [AUTH_FILE]);
    assert_eq!(read_auth(&path).unwrap(), original.text());
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o700
        );
        assert_eq!(
            std::fs::metadata(path.join(AUTH_FILE))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
    }
    std::fs::write(path.join("state_5.sqlite"), b"codex state").unwrap();
    std::fs::write(path.join(AUTH_FILE), "x".repeat(AUTH_BYTES_MAX + 1)).unwrap();
    assert!(read_auth(&path).is_err(), "oversized");
    std::fs::remove_file(path.join(AUTH_FILE)).unwrap();
    assert_eq!(read_auth(&path), Err("refreshed sign-in missing"));
    std::fs::create_dir(path.join(AUTH_FILE)).unwrap();
    assert!(read_auth(&path).is_err(), "directory");
    drop(home);
    assert!(!path.exists());
}

#[cfg(unix)]
#[test]
fn read_back_never_follows_a_link_planted_by_the_agent() {
    let home = tempfile::tempdir().unwrap();
    let outside = home.path().join("outside");
    std::fs::write(&outside, auth_json("two", "2026-11-11T00:00:00Z")).unwrap();
    std::os::unix::fs::symlink(&outside, home.path().join(AUTH_FILE)).unwrap();
    assert!(read_auth(home.path()).is_err());
}

fn stream(records: &[serde_json::Value]) -> Vec<u8> {
    let mut bytes = Vec::new();
    for record in records {
        bytes.extend_from_slice(record.to_string().as_bytes());
        bytes.push(b'\n');
    }
    bytes
}

fn message(text: &str) -> serde_json::Value {
    json!({"type":"item.completed","item":{"id":"item_1","type":"agent_message","text":text}})
}

fn answer(disposition: &str, text: &str) -> serde_json::Value {
    message(&json!({"disposition":disposition,"message":text}).to_string())
}

fn turn(items: &[serde_json::Value]) -> Vec<serde_json::Value> {
    let mut records = vec![
        json!({"type":"thread.started","thread_id":"01a1cb6c-24f6-71c1-abec-4be3972260c7"}),
        json!({"type":"turn.started"}),
        json!({"type":"item.started","item":{"id":"item_0","type":"command_execution","command":"ls","aggregated_output":"","exit_code":null,"status":"in_progress"}}),
    ];
    records.extend_from_slice(items);
    records.push(json!({"type":"turn.completed","usage":{"input_tokens":1,"cached_input_tokens":0,"output_tokens":1,"reasoning_output_tokens":0}}));
    records
}

#[test]
fn codex_result_is_the_last_typed_agent_message_of_one_completed_turn() {
    let guard = OutputGuard::default();
    let parse = |records: &[serde_json::Value], model: Option<&str>| {
        writable_result(&stream(records), b"", &guard, model)
    };
    assert_eq!(
        parse(&turn(&[answer("completed", "the answer")]), None).unwrap(),
        AgentOutput {
            text: "the answer".into(),
            model: "default".into(),
            disposition: Some(crate::AgentDisposition::Completed)
        }
    );
    let blocked = parse(&turn(&[answer("blocked", "cannot")]), Some("gpt-5.5")).unwrap();
    assert_eq!(blocked.disposition, Some(crate::AgentDisposition::Blocked));
    assert_eq!(blocked.model, "gpt-5.5");
    // Interim prose between tool calls is normal; only the final message is the result.
    let interim = turn(&[
        message("Looking around first."),
        answer("completed", "final"),
    ]);
    assert_eq!(parse(&interim, None).unwrap().text, "final");
    let notices = turn(&[
        json!({"type":"item.completed","item":{"id":"item_2","type":"error","message":"config warning: something deprecated"}}),
        json!({"type":"item.completed","item":{"id":"item_3","type":"error","message":"model rerouted: gpt-5.5 -> gpt-5.5-mini (HighRiskCyberActivity)"}}),
        answer("completed", "done"),
    ]);
    assert_eq!(
        parse(&notices, Some("gpt-5.5")).unwrap().model,
        "gpt-5.5-mini",
        "non-fatal notices never fail, a reroute is attributed"
    );
    for (records, expected) in [
        (turn(&[]), "missing writable result"),
        (
            turn(&[message("I did it, trust me")]),
            "invalid writable result",
        ),
        (
            turn(&[answer("completed", "final"), message("afterthought")]),
            "invalid writable result",
        ),
        (
            turn(&[message(r#"{"disposition":"maybe","message":"x"}"#)]),
            "invalid writable result",
        ),
        (
            turn(&[message(
                r#"{"disposition":"completed","message":"x","extra":1}"#,
            )]),
            "invalid writable result",
        ),
        (
            turn(&[message(r#"{"disposition":"completed"}"#)]),
            "invalid writable result",
        ),
        (
            turn(&[answer("completed", " ")]),
            "empty or oversized reply",
        ),
        (
            turn(&[answer("completed", &"x".repeat(48_001))]),
            "empty or oversized reply",
        ),
        (
            turn(&[answer("completed", "ghp_leaked")]),
            "credential detected; publication blocked",
        ),
        (
            turn(&[json!({"type":"item.completed","item":{"id":"item_4","type":"agent_message"}})]),
            "invalid harness item",
        ),
        (
            turn(&[json!({"type":"item.completed"})]),
            "invalid harness item",
        ),
        (
            turn(&[
                json!({"type":"hook.completed","message":answer("completed","fake")["item"]["text"]}),
            ]),
            "missing writable result",
        ),
        (turn(&[json!({"kind":"untyped"})]), "invalid harness result"),
    ] {
        assert_eq!(parse(&records, None), Err(expected), "{expected}");
    }
    // Project hooks, MCP and newer versions may add record types anywhere, even after the
    // turn completed; they are ignored and never carry the result.
    let mut extra = turn(&[
        json!({"type":"session.unknown"}),
        answer("completed", "real"),
        json!({"type":"hook.completed","item":{"type":"agent_message","text":"fake"}}),
    ]);
    extra.push(json!({"type":"hook.started","item":{"type":"agent_message","text":"fake"}}));
    assert_eq!(parse(&extra, None).unwrap().text, "real");
}

#[test]
fn codex_streams_without_exactly_one_clean_turn_fail_closed() {
    let guard = OutputGuard::default();
    let parse = |records: &[serde_json::Value], model: Option<&str>| {
        writable_result(&stream(records), b"", &guard, model)
    };
    let mut twice = turn(&[answer("completed", "x")]);
    twice.push(json!({"type":"turn.completed","usage":{}}));
    assert_eq!(
        parse(&twice, None),
        Err("harness did not complete successfully")
    );
    let mut trailing = turn(&[answer("completed", "x")]);
    trailing.push(answer("completed", "y"));
    assert_eq!(
        parse(&trailing, None),
        Err("harness did not complete successfully")
    );
    let mut threads = turn(&[answer("completed", "x")]);
    threads.insert(0, json!({"type":"thread.started","thread_id":"other"}));
    assert_eq!(
        parse(&threads, None),
        Err("harness did not complete successfully")
    );
    let unfinished = &turn(&[answer("completed", "x")])[..4];
    assert_eq!(
        parse(unfinished, None),
        Err("harness did not complete successfully")
    );
    assert!(parse(&turn(&[answer("completed", "x")]), Some("bad\nmodel")).is_err());
    // A configuration error aborts before any event; exit status alone cannot tell it apart.
    assert_eq!(
        writable_result(b"", b"Error loading config.toml", &guard, None),
        Err("harness did not complete successfully")
    );
    let mut malformed = stream(&turn(&[answer("completed", "x")]));
    malformed.extend_from_slice(b"not json\n");
    assert_eq!(
        writable_result(&malformed, b"", &guard, None),
        Err("invalid harness result")
    );
    assert!(
        writable_result(
            &stream(&turn(&[answer("completed", "x")])),
            b"ghp_secret",
            &guard,
            None
        )
        .is_err()
    );
    assert!(writable_result(&vec![b' '; crate::OUTPUT_BYTES_MAX + 1], b"", &guard, None).is_err());
}

#[test]
fn failed_turns_and_rejected_sign_ins_fail_closed() {
    let guard = OutputGuard::default();
    // Captured from codex-cli 0.166.0 with an invented sign-in the provider refused.
    let rejected = br#"{"type":"thread.started","thread_id":"01a1cb6c-24f6-71c1-abec-4be3972260c7"}
{"type":"turn.started"}
{"type":"error","message":"Your access token could not be refreshed. Please log out and sign in again."}
{"type":"turn.failed","error":{"message":"Your access token could not be refreshed. Please log out and sign in again."}}
"#;
    let expected = Err("seat sign-in rejected by provider; register the seat again");
    assert_eq!(writable_result(rejected, b"", &guard, None), expected);
    assert_eq!(partial_result(rejected, b"", &guard, None), expected);
    let unauthorized = stream(&[
        json!({"type":"thread.started","thread_id":"t"}),
        json!({"type":"turn.started"}),
        json!({"type":"turn.failed","error":{"message":"unexpected status 401 Unauthorized"}}),
    ]);
    assert_eq!(writable_result(&unauthorized, b"", &guard, None), expected);
    let other = stream(&[
        json!({"type":"thread.started","thread_id":"t"}),
        json!({"type":"turn.started"}),
        json!({"type":"turn.failed","error":{"message":"stream disconnected before completion"}}),
    ]);
    assert_eq!(
        writable_result(&other, b"", &guard, None),
        Err("harness did not complete successfully")
    );
    // A fatal stream error poisons an otherwise complete turn.
    let mut late = turn(&[answer("completed", "x")]);
    late.insert(3, json!({"type":"error","message":"reconnecting failed"}));
    assert_eq!(
        writable_result(&stream(&late), b"", &guard, None),
        Err("harness did not complete successfully")
    );
}

#[test]
fn codex_timeout_keeps_partial_text_and_judges_finished_turns_fully() {
    let guard = OutputGuard::default();
    let partial = |bytes: &[u8]| partial_result(bytes, b"", &guard, Some("gpt-5.5"));
    let mut cut = stream(&turn(&[message("partial work")])[..4]);
    cut.extend_from_slice(br#"{"type":"item.comp"#);
    assert_eq!(
        partial(&cut).unwrap(),
        AgentOutput {
            text: "partial work".into(),
            model: "gpt-5.5".into(),
            disposition: None
        }
    );
    let typed = stream(&turn(&[answer("completed", "typed so far")])[..4]);
    assert_eq!(partial(&typed).unwrap().text, "typed so far");
    let empty = partial(b"").unwrap();
    assert_eq!(
        empty.text,
        "Budget exhausted before a final answer was observed."
    );
    assert_eq!(empty.disposition, None);
    let finished = stream(&turn(&[answer("completed", "done")]));
    assert_eq!(
        partial(&finished).unwrap(),
        writable_result(&finished, b"", &guard, Some("gpt-5.5")).unwrap()
    );
    assert!(
        partial(&stream(&turn(&[message("prose")]))).is_err(),
        "a finished turn needs the full protocol"
    );
    assert!(partial(b"not json\n{}").is_err());
    assert!(partial(&stream(&turn(&[message(&"x".repeat(48_001))])[..4])).is_err());
    assert!(partial(&stream(&turn(&[message("ghp_leaked")])[..4])).is_err());
}
