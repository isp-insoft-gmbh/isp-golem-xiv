#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
use hmac::{Hmac, KeyInit, Mac};
use router::{
    Delivery, DispatchV1, parse_golem, parse_mention, refusal, seat_secret, valid_delivery,
    valid_repository, verify_signature,
};
use sha2::Sha256;
use std::fmt::Write;

#[test]
fn seat_choice_prefers_golem_provider_then_preferred_then_order_and_names_fallback() {
    use router::{SeatChoice, choose_seat, harness_works};
    let plain = parse_golem("hello", "---\n---\nTask");
    let foreign = parse_golem(
        "foreign",
        "---\nharness: codex\nprovider: openai-codex\nmodel: gpt-x\n---\nTask",
    );
    assert_eq!(
        choose_seat(&plain.frontmatter, &["anthropic"], None),
        Some(SeatChoice {
            provider: "anthropic".into(),
            harness: "claude".into(),
            model: None,
            fallback: None
        })
    );
    let fell = choose_seat(&foreign.frontmatter, &["anthropic"], None).unwrap();
    assert_eq!(
        (fell.provider.as_str(), fell.harness.as_str(), fell.model),
        ("anthropic", "pi", None)
    );
    assert_eq!(
        fell.fallback.as_deref(),
        Some(
            "asked for openai-codex/codex/gpt-x, used anthropic/pi with its default model, because there is no openai-codex seat"
        )
    );
    // Harness-only fallback: the seat at the golem's provider stays, the harness becomes pi.
    let mismatched = parse_golem(
        "mismatched",
        "---\nharness: codex\nprovider: anthropic\nmodel: gpt-x\n---\nTask",
    );
    assert_eq!(
        choose_seat(
            &mismatched.frontmatter,
            &["anthropic", "openai-codex"],
            Some("openai-codex")
        ),
        Some(SeatChoice {
            provider: "anthropic".into(),
            harness: "pi".into(),
            model: None,
            fallback: Some(
                "asked for anthropic/codex/gpt-x, used anthropic/pi with its default model, because codex does not work with anthropic".into()
            ),
        }),
        "provider kept, never moved to a seat the golem did not ask for"
    );
    let roaming = parse_golem(
        "roaming",
        "---\nharness: pi\nprovider: openai-codex\n---\nTask",
    );
    let kept = choose_seat(&roaming.frontmatter, &["anthropic"], None).unwrap();
    assert_eq!(
        (kept.provider.as_str(), kept.harness.as_str()),
        ("anthropic", "pi"),
        "harness kept where it works"
    );
    assert!(
        kept.fallback
            .unwrap()
            .ends_with("because there is no openai-codex seat")
    );
    // With two providers a valid golem's missing provider leaves one seat; an unknown provider
    // still shows that the preferred seat wins over provider order.
    let unknown = parse_golem("unknown", "---\nharness: codex\nprovider: other\n---\nTask");
    let preferred = choose_seat(
        &unknown.frontmatter,
        &["anthropic", "openai-codex"],
        Some("openai-codex"),
    )
    .unwrap();
    assert_eq!(
        (preferred.provider.as_str(), preferred.harness.as_str()),
        ("openai-codex", "codex")
    );
    let ignored = choose_seat(&foreign.frontmatter, &["anthropic"], Some("openai-codex")).unwrap();
    assert_eq!(
        ignored.provider, "anthropic",
        "a preferred provider without a seat is ignored"
    );
    assert_eq!(
        choose_seat(&plain.frontmatter, &[], Some("anthropic")),
        None
    );
    assert!(
        harness_works("openai-codex", "pi")
            && !harness_works("openai-codex", "claude")
            && !harness_works("anthropic", "codex")
            && !harness_works("other", "pi")
    );
}

#[test]
fn fallback_dispatch_runs_foreign_golems_on_an_anthropic_seat_with_pi_and_says_so() {
    use router::{choose_seat, queued_summary, runner_supports};
    let foreign = parse_golem(
        "foreign",
        "---\nharness: codex\nprovider: anthropic\n---\nReply with one short greeting.",
    );
    assert!(foreign.errors.is_empty(), "{:?}", foreign.errors);
    let choice = choose_seat(&foreign.frontmatter, &["anthropic"], None).unwrap();
    assert!(
        choice.runnable(),
        "anthropic-only payer runs foreign golems with pi"
    );
    let summary = queued_summary("member", &choice, 1800);
    assert!(summary.starts_with("outcome: queued\nseat: member/anthropic\nharness: pi\nmodel: default\nbudget: 1800s\nlog: https://github.com/isp-insoft-gmbh/isp-golem-xiv-ops/actions\n"));
    // contract check_run.fallback: `fallback: {any}`, one or more non-space characters.
    let fallback = summary
        .lines()
        .find_map(|line| line.strip_prefix("fallback: "))
        .unwrap();
    assert!(fallback.starts_with(|c: char| !c.is_whitespace()));
    assert_eq!(
        fallback,
        "asked for anthropic/codex, used anthropic/pi with its default model, because codex does not work with anthropic"
    );
    assert!(summary.ends_with("\nStop: @isp-golem-xiv stop"));
    let plain = parse_golem("hello", "---\n---\nTask");
    let own = choose_seat(&plain.frontmatter, &["anthropic"], None).unwrap();
    assert!(own.runnable());
    assert!(!queued_summary("member", &own, 1800).contains("fallback"));
    // The runner executes Anthropic (claude, pi) and OpenAI Codex (codex) seats; other choices are refused, never rerouted.
    let codex = choose_seat(&foreign.frontmatter, &["openai-codex"], None).unwrap();
    assert_eq!(
        (codex.provider.as_str(), codex.harness.as_str()),
        ("openai-codex", "codex")
    );
    assert!(codex.runnable());
    let roaming = parse_golem("roaming", "---\nharness: pi\n---\nTask");
    assert!(
        !choose_seat(&roaming.frontmatter, &["openai-codex"], None)
            .unwrap()
            .runnable()
    );
    assert!(
        choose_seat(&foreign.frontmatter, &[], None).is_none(),
        "no seat, no run"
    );
    assert!(runner_supports(
        "anthropic",
        "claude",
        Some("claude-sonnet-5-5")
    ));
    assert!(runner_supports("anthropic", "pi", None));
    for (provider, harness, model) in [
        ("anthropic", "codex", None),
        ("openai-codex", "pi", None),
        ("anthropic", "pi", Some("anthropic/claude:high")),
        ("anthropic", "pi", Some("--injected")),
        ("anthropic", "pi", Some("")),
    ] {
        assert!(
            !runner_supports(provider, harness, model),
            "{harness} {model:?}"
        );
    }
}

#[test]
fn seat_holders_map_secret_names_to_members_and_report_people_who_left() {
    let names = [
        "APP_PRIVATE_KEY",
        "SEAT_MEMBER__ANTHROPIC",
        "SEAT_MEMBER__OPENAI_CODEX",
        "SEAT_JANE_DOE__ANTHROPIC",
        "SEAT_GONE_PERSON__OPENAI_CODEX",
        "SEAT_BAD__UNKNOWN",
        "SEAT___ANTHROPIC",
    ];
    let holders = router::seat_holders(&names, &["member", "jane-doe", "other"]);
    assert_eq!(
        holders.registered,
        vec![
            ("jane-doe".to_owned(), vec!["anthropic"]),
            ("member".to_owned(), vec!["anthropic", "openai-codex"]),
        ]
    );
    assert_eq!(
        holders.departed,
        vec![("GONE_PERSON".to_owned(), vec!["openai-codex"])]
    );
}

#[test]
fn dashboard_lists_sections_golems_disabled_and_seats_privately_only() {
    let golems = vec![
        parse_golem("hello", "---\n---\n\n  Report what you see.\nMore."),
        parse_golem(
            "nightly",
            "---\nschedule: daily\nowner: lead\ntriggers: [issues.opened]\n---\nTidy.",
        ),
        parse_golem("broken", "---\nschedule: someday\n---\nTask"),
    ];
    let holders = router::seat_holders(
        &["SEAT_MEMBER__ANTHROPIC", "SEAT_GONE__ANTHROPIC"],
        &["member"],
    );
    let private = router::dashboard_body(&golems, Some(&holders), &[]);
    assert!(private.starts_with(router::DASHBOARD_MARKER));
    for heading in ["## Golems", "## Disabled", "## Talk to golems", "## Seats"] {
        assert!(private.lines().any(|line| line == heading), "{heading}");
    }
    assert!(private.contains("- `hello`: Report what you see. Wakes: mention. Pays: whoever starts it. Start: comment `@isp-golem-xiv hello` or tick its run box.\n  <!-- golem-last golem=hello -->\n  Last: never run\n  - [ ] Run `hello` now <!-- golem-run golem=hello -->\n"));
    assert!(private.contains("- `nightly`: Tidy. Wakes: mention; events `issues.opened`; daily. Pays: whoever starts it; unattended runs @lead."));
    assert!(
        !private.contains("golem=broken"),
        "disabled golems get no run box"
    );
    assert!(
        private.contains("## Disabled\n\n- `broken`: ") && !private.contains("- `broken`: Task")
    );
    assert!(
        private.contains("- @member: `anthropic`")
            && private.contains("Seats of people who left the organization:\n- GONE: `anthropic`")
    );
    let public = router::dashboard_body(&golems, None, &[]);
    assert!(
        !public.contains("@member")
            && !public.contains("GONE")
            && public.contains("`@isp-golem-xiv seat`")
    );
    let empty = router::dashboard_body(&[], None, &[]);
    assert!(empty.contains("## Golems\n\nNone yet") && empty.contains("## Disabled\n\nNone."));
}

fn last(outcome: &str, model: &str, run_id: u64) -> router::LastResult {
    router::LastResult {
        outcome: outcome.into(),
        login: "member".into(),
        provider: "anthropic".into(),
        harness: "claude".into(),
        model: model.into(),
        at: "2026-11-13T10:04Z".into(),
        run_id,
    }
}

#[test]
fn utc_minute_formats_civil_dates_including_leap_days_and_year_ends() {
    assert_eq!(router::utc_minute(0), "1970-01-01T00:00Z");
    assert_eq!(router::utc_minute(951_868_740), "2000-02-29T23:59Z");
    assert_eq!(router::utc_minute(1_735_689_540), "2024-12-31T23:59Z");
    assert_eq!(router::utc_minute(1_794_564_299), "2026-11-13T10:04Z");
}

#[test]
fn last_results_render_escaped_survive_rerender_and_replace_only_their_golem() {
    let golems = vec![
        parse_golem("hello", "---\n---\nSay hi <!-- hide -->."),
        parse_golem("other", "---\n---\nOther."),
    ];
    let replied = last("replied", "claude-opus-5-5", 123);
    let body = router::dashboard_body(&golems, None, &[("hello".into(), replied.clone())]);
    assert!(
        body.contains("- `hello`: Say hi &lt;!-- hide -->. Wakes:"),
        "purpose cannot open a comment"
    );
    assert!(body.contains("\n  <!-- golem-last golem=hello outcome=replied seat=member/anthropic harness=claude model=claude-opus-5-5 at=2026-11-13T10:04Z run=123 -->\n  Last: replied on member/anthropic, claude, model claude-opus-5-5, 2026-11-13 10:04 UTC, [log](https://github.com/isp-insoft-gmbh/isp-golem-xiv-ops/actions/runs/123)\n"));
    assert!(body.contains("<!-- golem-last golem=other -->\n  Last: never run\n"));
    assert_eq!(
        router::parse_last_results(&body),
        vec![("hello".to_owned(), replied.clone())]
    );
    // A push re-renders from parsed results, so the result survives unchanged.
    assert_eq!(
        router::dashboard_body(&golems, None, &router::parse_last_results(&body)),
        body
    );

    let blocked = last("blocked", "unobserved", 456);
    let crlf = body.replace('\n', "\r\n");
    let updated = router::dashboard_with_last(&crlf, "other", &blocked).unwrap();
    let expected = crlf.replace(
        "  <!-- golem-last golem=other -->\r\n  Last: never run\r\n",
        "  <!-- golem-last golem=other outcome=blocked seat=member/anthropic harness=claude model=unobserved at=2026-11-13T10:04Z run=456 -->\r\n  Last: blocked on member/anthropic, claude, model unobserved, 2026-11-13 10:04 UTC, [log](https://github.com/isp-insoft-gmbh/isp-golem-xiv-ops/actions/runs/456)\r\n",
    );
    assert_eq!(
        updated, expected,
        "only that golem's two lines change; line endings kept"
    );
    let replaced = router::dashboard_with_last(&updated, "hello", &blocked).unwrap();
    assert!(replaced.contains("golem=hello outcome=blocked") && !replaced.contains("replied"));
    assert_eq!(router::parse_last_results(&replaced).len(), 2);

    assert_eq!(
        router::dashboard_with_last(&body, "missing", &blocked),
        None
    );
    assert_eq!(
        router::dashboard_with_last(
            &body.replacen(router::DASHBOARD_MARKER, "", 1),
            "hello",
            &blocked
        ),
        None
    );
    assert_eq!(
        router::dashboard_with_last(
            &body.replace("  Last: never run", "  edited"),
            "other",
            &blocked
        ),
        None
    );
    for invalid in [
        last("hacked", "m", 1),
        last("replied", "model -->", 1),
        last("replied", "-flag", 1),
        last("replied", "m", 0),
        router::LastResult {
            login: "a b".into(),
            ..last("replied", "m", 1)
        },
        router::LastResult {
            provider: "github-copilot".into(),
            ..last("replied", "m", 1)
        },
        router::LastResult {
            harness: "bash".into(),
            ..last("replied", "m", 1)
        },
        router::LastResult {
            at: "yesterday".into(),
            ..last("replied", "m", 1)
        },
    ] {
        assert!(!invalid.valid());
        assert_eq!(router::dashboard_with_last(&body, "other", &invalid), None);
        let rendered = router::dashboard_body(&golems, None, &[("other".into(), invalid)]);
        assert!(
            rendered.contains("golem=other -->\n  Last: never run"),
            "invalid results never render"
        );
    }
    // Hand-edited markers are not carried over, but remain replaceable by the next run.
    let tampered = body.replace("outcome=replied", "outcome=replied extra=1");
    assert!(
        router::parse_last_results(&tampered).is_empty(),
        "tampered marker not carried over"
    );
    assert!(
        router::dashboard_with_last(&tampered, "hello", &blocked)
            .unwrap()
            .contains("golem=hello outcome=blocked")
    );
}

#[test]
fn run_boxes_count_only_fresh_ticks_in_the_dashboard_and_clear_without_other_changes() {
    let golems = vec![
        parse_golem(
            "hello",
            "---\n---\nFake:   - [x] Run `other` now <!-- golem-run golem=other -->",
        ),
        parse_golem("other", "---\n---\nOther."),
        parse_golem("third", "---\n---\nThird."),
    ];
    let old = router::dashboard_body(&golems, None, &[]);
    let tick = |body: &str, golem: &str| {
        body.replace(
            &format!("  - [ ] Run `{golem}` now"),
            &format!("  - [x] Run `{golem}` now"),
        )
    };
    assert!(
        router::ticked_run_boxes(&old, &old).is_empty(),
        "unrelated edits tick nothing"
    );
    assert!(
        router::ticked_run_boxes(&old, &old.replace("Other.", "Changed.")).is_empty(),
        "text edits tick nothing"
    );
    let new = tick(&tick(&old, "third"), "hello");
    assert_eq!(router::ticked_run_boxes(&old, &new), ["hello", "third"]);
    assert_eq!(
        router::ticked_run_boxes(&old, &new.replace("[x] Run `third`", "[X] Run `third`")),
        ["hello", "third"]
    );
    assert_eq!(
        router::ticked_run_boxes(&old.replace('\n', "\r\n"), &new.replace('\n', "\r\n")),
        ["hello", "third"]
    );
    assert!(
        router::ticked_run_boxes(&new, &new).is_empty(),
        "a box left ticked is not ticked again"
    );
    assert!(
        router::ticked_run_boxes(&new, &old).is_empty(),
        "unticking starts nothing"
    );
    let stripped = |body: &str| body.replacen(router::DASHBOARD_MARKER, "", 1);
    assert!(
        router::ticked_run_boxes(&stripped(&old), &new).is_empty(),
        "old body must be the dashboard"
    );
    assert!(
        router::ticked_run_boxes(&old, &stripped(&new)).is_empty(),
        "new body must be the dashboard"
    );
    // Boxes are recognized only in their exact rendered form.
    for forged in [
        tick(&old, "other").replace(
            "<!-- golem-run golem=other -->",
            "<!-- golem-run golem=hello -->",
        ),
        tick(&old, "other").replace("  - [x] Run `other`", " - [x] Run `other`"),
        tick(&old, "other").replace(
            "<!-- golem-run golem=other -->",
            "<!-- golem-run golem=other --> extra",
        ),
        old.replace("  - [ ] Run `other`", "  - [y] Run `other`"),
    ] {
        assert!(
            router::ticked_run_boxes(&old, &forged).is_empty(),
            "{forged}"
        );
    }
    assert_eq!(router::dashboard_unticked(&old), None);
    assert_eq!(router::dashboard_unticked(&new).unwrap(), old);
    let crlf = new.replace('\n', "\r\n");
    assert_eq!(
        router::dashboard_unticked(&crlf).unwrap(),
        old.replace('\n', "\r\n")
    );
    assert_eq!(router::dashboard_unticked(&stripped(&new)), None);
    assert!(
        old.contains("Fake:   - [x] Run `other` now"),
        "purpose text stays verbatim"
    );
}

#[test]
fn dashboard_ticks_need_a_person_editing_the_apps_dashboard_body() {
    let golems = vec![
        parse_golem("a", "---\n---\nA."),
        parse_golem("b", "---\n---\nB."),
        parse_golem("c", "---\n---\nC."),
        parse_golem("d", "---\n---\nD."),
    ];
    let old = router::dashboard_body(&golems, None, &[]);
    let new = old.replace("- [ ]", "- [x]");
    let payload = serde_json::json!({"action":"edited","sender":{"id":42,"login":"member","type":"User"},"repository":{"id":99,"full_name":"isp-insoft-gmbh/isp-golem-xiv-test-fixture"},"installation":{"id":153_964_219},"issue":{"number":5,"title":"Golems","body":new,"user":{"login":router::BOT}},"changes":{"body":{"from":old}}});
    let ticks = |value: &serde_json::Value, event: &str| {
        let delivery: router::EventDelivery = serde_json::from_value(value.clone()).unwrap();
        router::dashboard_ticks(event, &delivery, value.to_string().as_bytes())
    };
    assert_eq!(
        ticks(&payload, "issues"),
        ["a", "b", "c"],
        "bounded per edit"
    );
    assert!(
        ticks(&payload, "issue_comment").is_empty(),
        "only issues events"
    );
    for (pointer, value) in [
        ("/action", serde_json::json!("opened")),
        ("/sender/login", serde_json::json!(router::BOT)),
        (
            "/sender",
            serde_json::json!({"id":7,"login":"dependabot[bot]","type":"Bot"}),
        ),
        ("/issue/title", serde_json::json!("Golems!")),
        ("/issue/user/login", serde_json::json!("member")),
        ("/issue/body", serde_json::Value::Null),
        ("/changes", serde_json::json!({"title":{"from":"Golems"}})),
        (
            "/changes/body/from",
            serde_json::json!(old.clone() + &"x".repeat(router::FILE_BYTES_MAX)),
        ),
        ("/installation/id", serde_json::json!(1)),
    ] {
        let mut changed = payload.clone();
        *changed.pointer_mut(pointer).unwrap() = value;
        assert!(ticks(&changed, "issues").is_empty(), "{pointer}");
    }
}

#[test]
fn dashboard_refusals_answer_the_person_with_marker_and_remedy() {
    let names = ["a".to_owned(), "b".to_owned()];
    let not_member = router::dashboard_refusal("outsider", "not-member", &names);
    assert!(not_member.starts_with("<!-- isp-golem-xiv kind=refused reason=not-member -->\n@outsider, the run box of `a`, `b` did not start a run. Only isp-insoft-gmbh members"));
    let no_seat = router::dashboard_refusal("member", "no-seat", &names[..1]);
    assert!(no_seat.starts_with("<!-- isp-golem-xiv kind=refused reason=no-seat -->\n@member, the run box of `a` did not start a run.") && no_seat.contains("/seats"));
    assert!(
        router::dashboard_refusal("member", "unknown-golem", &names[..1])
            .contains("no longer an enabled golem")
    );
}

#[test]
fn onboarding_adds_exactly_house_rules_and_hello_and_explains_toolchain_and_seat() {
    let files = router::onboarding_files();
    assert_eq!(
        files.each_ref().map(|(path, _)| *path),
        [".golems/_golems.md", ".golems/hello.md"]
    );
    assert_eq!(files[0].1.trim(), router::HOUSE_RULES);
    let hello = parse_golem("hello", &files[1].1);
    assert!(hello.errors.is_empty(), "{:?}", hello.errors);
    assert!(router::onboarding_body(Some("mise.toml")).contains("`mise.toml`"));
    let none = router::onboarding_body(None);
    assert!(
        none.contains("No toolchain description found")
            && none.contains("`@isp-golem-xiv seat`")
            && none.contains(router::SEAT_URL)
    );
}

#[test]
fn validation_summary_fails_on_any_invalid_golem_and_names_reasons() {
    let valid = parse_golem("hello", "---\n---\nTask");
    let broken = parse_golem("broken", "---\nschedule: someday\n---\nTask");
    assert!(router::validation_summary(&[]).0);
    let (ok, summary) = router::validation_summary(std::slice::from_ref(&valid));
    assert!(ok && summary.contains("- `hello`: valid"));
    let (ok, summary) = router::validation_summary(&[valid, broken]);
    assert!(!ok);
    assert!(summary.starts_with("Invalid golems are disabled after merge until fixed."));
    assert!(summary.contains("- `broken`: ") && summary.contains("schedule"));
}

#[test]
fn sign_in_location_uses_public_client_encoded_callback_and_uuid_state() {
    let state = "0f6e1c2a-1b2c-4d3e-8f90-123456789abc";
    assert_eq!(
        router::sign_in_location(state).as_deref(),
        Some(
            "https://github.com/login/oauth/authorize?client_id=Iv23liEnfLEVCSS2aUdR&redirect_uri=https%3A%2F%2Fisp-golem-xiv.isp-insoft-gmbh.workers.dev%2Fauth%2Fgithub%2Fcallback&state=0f6e1c2a-1b2c-4d3e-8f90-123456789abc"
        )
    );
    for invalid in [
        "",
        "x&redirect_uri=evil",
        "0f6e1c2a-1b2c-4d3e-8f90-123456789abc&x",
    ] {
        assert_eq!(router::sign_in_location(invalid), None, "{invalid}");
    }
}

mod registration {
    use router::registration::{
        SESSION_SECONDS, SeatRegistration, admin_page, base64_length, cookie, csrf_token,
        csrf_valid, oauth_access_token, oauth_exchange_body, preferred_registration, public_key,
        seat_registration, seats_page, session_cookie, session_key, session_login, session_value,
        state_matches,
    };
    use serde_json::json;

    const NOW: u64 = 1_800_000_000;

    fn key() -> [u8; 32] {
        session_key("fixture-client-secret").unwrap()
    }

    /// Reference encoder, independent of the validator under test.
    fn base64(bytes: &[u8]) -> String {
        const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let mut text = String::new();
        for chunk in bytes.chunks(3) {
            let mut block = [0u8; 3];
            block.get_mut(..chunk.len()).unwrap().copy_from_slice(chunk);
            let n = (u32::from(block[0]) << 16) | (u32::from(block[1]) << 8) | u32::from(block[2]);
            for index in 0..4 {
                if index <= chunk.len() {
                    let digit = (n >> (18 - 6 * index)) & 63;
                    text.push(char::from(*ALPHABET.get(digit as usize).unwrap()));
                } else {
                    text.push('=');
                }
            }
        }
        text
    }

    #[test]
    fn sessions_verify_only_untampered_unexpired_values_under_the_same_key() {
        let key = key();
        assert_ne!(key, session_key("other-client-secret").unwrap());
        assert_eq!(session_key(""), None);
        let value = session_value(&key, "some-member", NOW).unwrap();
        assert!(value.starts_with(&format!("some-member.{}.", NOW + SESSION_SECONDS)));
        assert_eq!(session_login(&key, &value, NOW), Some("some-member"));
        assert_eq!(
            session_login(&key, &value, NOW + SESSION_SECONDS - 1),
            Some("some-member")
        );
        assert_eq!(
            session_login(&key, &value, NOW + SESSION_SECONDS),
            None,
            "expired"
        );
        assert_eq!(
            session_login(&key, &value, NOW - 30),
            Some("some-member"),
            "small skew"
        );
        assert_eq!(
            session_login(&key, &value, NOW - 120),
            None,
            "expiry beyond a session length"
        );
        let other = session_key("other-client-secret").unwrap();
        assert_eq!(session_login(&other, &value, NOW), None, "other key");
        let (signed, tag) = value.rsplit_once('.').unwrap();
        let (login, expires) = signed.split_once('.').unwrap();
        let later = (expires.parse::<u64>().unwrap() + 1).to_string();
        let flipped = if tag.starts_with('0') { "1" } else { "0" };
        for tampered in [
            format!("other-member.{expires}.{tag}"),
            format!("{login}.{later}.{tag}"),
            format!("{signed}.{flipped}{}", &tag[1..]),
            format!("{signed}.{}", &tag[1..]),
            format!("{value}.x"),
            format!("{login}.{tag}"),
            format!("{login}.-1.{tag}"),
            format!("-bad.{expires}.{tag}"),
            String::new(),
            "x".repeat(200),
        ] {
            assert_eq!(session_login(&key, &tampered, NOW), None, "{tampered}");
        }
        assert_eq!(session_value(&key, "bad login", NOW), None);
        assert_eq!(session_value(&key, "member", u64::MAX), None);
        assert_eq!(
            session_cookie(&value),
            format!("golem_session={value}; Path=/; Max-Age=3600; HttpOnly; Secure; SameSite=Lax")
        );
    }

    #[test]
    fn csrf_tokens_derive_from_the_exact_session_and_key() {
        let key = key();
        let session = session_value(&key, "member", NOW).unwrap();
        let another = session_value(&key, "other", NOW).unwrap();
        let token = csrf_token(&key, &session).unwrap();
        assert_eq!(token.len(), 64);
        assert_ne!(token, csrf_token(&key, &another).unwrap());
        assert!(csrf_valid(&key, &session, &token));
        assert!(csrf_valid(&key, &session, &token.to_ascii_uppercase()));
        assert!(
            !csrf_valid(&key, &another, &token),
            "token of another session"
        );
        assert!(!csrf_valid(&session_key("x").unwrap(), &session, &token));
        for bad in ["", "00", &token[1..], &format!("{token}0"), &"g".repeat(64)] {
            assert!(!csrf_valid(&key, &session, bad), "{bad}");
        }
    }

    #[test]
    fn cookies_parse_exact_names_and_refuse_duplicates_and_oversized_headers() {
        assert_eq!(cookie("golem_session=abc", "golem_session"), Some("abc"));
        assert_eq!(
            cookie("a=1; golem_session=abc; b=2", "golem_session"),
            Some("abc")
        );
        assert_eq!(
            cookie("xgolem_session=abc; golem_session_x=1", "golem_session"),
            None
        );
        assert_eq!(
            cookie("golem_session=a; golem_session=b", "golem_session"),
            None,
            "tossed duplicate"
        );
        assert_eq!(cookie("", "golem_session"), None);
        let oversized = format!("golem_session=abc; pad={}", "x".repeat(8192));
        assert_eq!(cookie(&oversized, "golem_session"), None);
    }

    #[test]
    fn oauth_state_code_and_token_answers_are_validated() {
        let state = "0f6e1c2a-1b2c-4d3e-8f90-123456789abc";
        assert!(state_matches(state, state));
        assert!(!state_matches(
            state,
            "0f6e1c2a-1b2c-4d3e-8f90-123456789abd"
        ));
        assert!(!state_matches("", state));
        assert!(!state_matches("not-a-uuid", "not-a-uuid"));
        assert_eq!(
            oauth_exchange_body("s&cret=", "abc_DEF-123").as_deref(),
            Some(
                "client_id=Iv23liEnfLEVCSS2aUdR&client_secret=s%26cret%3D&code=abc_DEF-123&redirect_uri=https%3A%2F%2Fisp-golem-xiv.isp-insoft-gmbh.workers.dev%2Fauth%2Fgithub%2Fcallback"
            )
        );
        for code in ["", "a&b=c", "a b", &"a".repeat(257)] {
            assert_eq!(oauth_exchange_body("secret", code), None, "{code}");
        }
        assert_eq!(oauth_exchange_body("", "code"), None);
        assert_eq!(
            oauth_access_token(&json!({"access_token":"ghu_x","token_type":"bearer"})),
            Some("ghu_x")
        );
        for answer in [
            json!({"error":"bad_verification_code"}),
            json!({"access_token":""}),
            json!({"access_token":"a b"}),
            json!({"access_token":7}),
        ] {
            assert_eq!(oauth_access_token(&answer), None, "{answer}");
        }
    }

    #[test]
    fn base64_length_matches_reference_encoding_and_rejects_other_shapes() {
        for length in 1u8..200 {
            let bytes: Vec<u8> = (0..length).map(|i| i.wrapping_mul(37)).collect();
            assert_eq!(
                base64_length(&base64(&bytes)),
                Some(bytes.len()),
                "{length}"
            );
        }
        for bad in ["", "abc", "ab=c", "a===", "ab-_", "abc\n", "ab c", "===="] {
            assert_eq!(base64_length(bad), None, "{bad:?}");
        }
    }

    #[test]
    fn seat_registrations_accept_only_sealed_sized_values_for_known_providers() {
        let sealed = base64(&[7; 49]);
        let body = |provider: &str, key_id: &str, value: &str| {
            json!({"provider":provider,"key_id":key_id,"encrypted_value":value}).to_string()
        };
        assert_eq!(
            seat_registration(body("openai-codex", "568250167242549743", &sealed).as_bytes()),
            Ok(SeatRegistration {
                provider: "openai-codex",
                key_id: "568250167242549743".into(),
                encrypted_value: sealed.clone()
            })
        );
        let largest = base64(&vec![1; 49_152 + 48]);
        assert!(seat_registration(body("anthropic", "1", &largest).as_bytes()).is_ok());
        let cases = [
            (body("ANTHROPIC", "1", &sealed), "unknown provider"),
            (body("anthropic", "", &sealed), "invalid key id"),
            (body("anthropic", "1/../x", &sealed), "invalid key id"),
            (
                body("anthropic", "1", "not base64!"),
                "encrypted value is not base64",
            ),
            (
                body("anthropic", "1", &base64(&[7; 48])),
                "encrypted value size is invalid",
            ),
            (
                body("anthropic", "1", &base64(&vec![1; 49_152 + 49])),
                "encrypted value size is invalid",
            ),
            (
                json!({"provider":"anthropic","key_id":"1","encrypted_value":sealed,"value":"x"})
                    .to_string(),
                "invalid request",
            ),
            (
                json!({"provider":"anthropic","key_id":"1"}).to_string(),
                "invalid request",
            ),
            ("[]".into(), "invalid request"),
            (" ".repeat(66_561), "request too large"),
        ];
        for (body, reason) in cases {
            let refused = seat_registration(body.as_bytes()).unwrap_err();
            assert_eq!(refused, reason);
            assert!(!refused.contains(&sealed), "never echoes the value");
        }
        assert_eq!(
            preferred_registration(br#"{"provider":"openai-codex"}"#),
            Ok("openai-codex")
        );
        assert_eq!(
            preferred_registration(br#"{"provider":"other"}"#),
            Err("unknown provider")
        );
        assert_eq!(
            preferred_registration(br#"{"provider":"anthropic","x":1}"#),
            Err("invalid request")
        );
        assert_eq!(
            preferred_registration(&[b' '; 1025]),
            Err("request too large")
        );
        assert_eq!(
            router::preferred_seat_variable("some-member").as_deref(),
            Some("SEAT_PREFERRED_SOME_MEMBER")
        );
        assert_eq!(router::preferred_seat_variable("bad/login"), None);
        let key = base64(&[9; 32]);
        assert_eq!(
            public_key(&json!({"key_id":"568","key":key})),
            Some(("568", key.as_str()))
        );
        assert_eq!(
            public_key(&json!({"key_id":"568","key":base64(&[9; 31])})),
            None
        );
        assert_eq!(public_key(&json!({"key_id":"a/b","key":key})), None);
    }

    #[test]
    fn seats_page_shows_only_given_seats_with_csrf_and_no_inline_script() {
        let page = seats_page("member", &["openai-codex"], Some("openai-codex"), "ab12");
        assert!(page.contains("<meta name=\"golem-csrf\" content=\"ab12\">"));
        assert!(page.contains("@member"));
        assert!(page.contains("<li data-seat=\"openai-codex\">"));
        assert!(!page.contains("data-seat=\"anthropic\""));
        assert_eq!(page.matches("<form class=\"seat\"").count(), 2);
        for provider in ["anthropic", "openai-codex"] {
            assert!(
                page.contains(&format!(
                    "<form class=\"seat\" data-provider=\"{provider}\">"
                )),
                "{provider}"
            );
        }
        assert!(page.contains("claude setup-token"));
        assert!(page.contains("<option value=\"openai-codex\" selected>"));
        assert!(
            !page.contains("<option value=\"anthropic\""),
            "preferred only among own seats"
        );
        assert_eq!(page.matches("<script").count(), 1);
        assert!(page.contains("<script src=\"/seats.js\" defer></script>"));
        assert!(
            !page.contains("onclick") && !page.contains("onsubmit"),
            "no inline handlers"
        );
        assert!(!page.contains("style"));
        let empty = seats_page("member", &[], None, "ab12");
        assert!(empty.contains("You have no seats yet.") && !empty.contains("id=\"preferred\""));
        let admin = admin_page("member");
        assert!(admin.contains("Slice 4") && !admin.contains("<script"));
        // Refreshing sign-ins rotate: people must create a fresh, golem-only sign-in on any OS.
        assert!(empty.contains("fresh sign-in used only by golems"));
        assert!(empty.contains("Never paste the sign-in you use yourself"));
        assert!(
            empty.contains("export CODEX_HOME=\"$(mktemp -d)\""),
            "macOS/Linux instructions"
        );
        assert!(
            empty.contains("$env:CODEX_HOME = Join-Path $env:TEMP"),
            "Windows instructions"
        );
        assert!(empty.contains("so golems hold the only copy"));
        assert!(!empty.contains("paste the content of <code>~/.codex/auth.json</code>"));
    }
}

#[test]
fn help_and_seat_replies_carry_markers_choices_and_only_the_askers_seats() {
    let golems = vec![
        parse_golem("hello", "---\n---\nTask"),
        parse_golem(
            "foreign",
            "---\nharness: codex\nprovider: anthropic\n---\nTask",
        ),
        parse_golem("broken", "---\nschedule: someday\n---\nTask"),
    ];
    let help = router::help_reply("member", &golems, &["anthropic"], None);
    assert!(help.starts_with("<!-- isp-golem-xiv kind=help -->\n"));
    assert!(
        help.contains("- `hello`: seat `member/anthropic`, harness `claude`, model `default`\n")
    );
    assert!(help.contains("- `foreign`: seat `member/anthropic`, harness `pi`, model `default` (fallback: asked for anthropic/codex, used anthropic/pi with its default model, because codex does not work with anthropic)\n"));
    assert!(help.contains("Disabled: `broken`.") && !help.contains("- `broken`"));
    assert!(help.contains("Your seats: `anthropic`.") && help.contains(router::SEAT_URL));
    let none = router::help_reply("newbie", &golems, &[], None);
    assert!(
        none.contains(&format!(
            "- `hello`: no seat; register one at {}",
            router::SEAT_URL
        )) && none.contains("Your seats: none.")
    );
    let seat = router::seat_reply(
        "member",
        &["anthropic", "openai-codex"],
        Some("openai-codex"),
    );
    assert!(seat.starts_with(
        "<!-- isp-golem-xiv kind=seat -->\n@member, your seats: `anthropic`, `openai-codex`.\n"
    ));
    assert!(
        seat.contains("Preferred seat when a golem's provider is missing: `openai-codex`.")
            && seat.contains(router::SEAT_URL)
    );
    assert!(
        !router::seat_reply("member", &["anthropic"], Some("openai-codex")).contains("Preferred")
    );
}

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
        "harness: other\n",
        "provider: other\n",
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
    // Incompatible pairs are preferences resolved at run time by falling back to pi.
    for fields in [
        "harness: codex\nprovider: anthropic\n",
        "harness: claude\nprovider: openai-codex\n",
        "harness: codex\n",
    ] {
        let preference = parse_golem("preference", &document(fields));
        assert!(
            preference.errors.is_empty(),
            "{fields}: {:?}",
            preference.errors
        );
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
