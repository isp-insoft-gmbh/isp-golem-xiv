#![allow(clippy::unwrap_used, clippy::indexing_slicing)] // Malformed fixtures should fail loudly.
use super::*;
use std::{
    io::{Read, Write},
    net::TcpListener,
    thread,
};

type Observed = (String, Value);

fn server(responses: Vec<(u16, Value)>) -> (Github, thread::JoinHandle<Vec<Observed>>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    listener.set_nonblocking(true).unwrap();
    let handle = thread::spawn(move || {
        let mut observed = Vec::new();
        for (status, response) in responses {
            let deadline = std::time::Instant::now() + Duration::from_secs(4);
            let mut stream = loop {
                if let Ok((stream, _)) = listener.accept() {
                    break stream;
                }
                assert!(
                    std::time::Instant::now() < deadline,
                    "expected client request"
                );
                thread::sleep(Duration::from_millis(2));
            };
            stream.set_nonblocking(false).unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(3)))
                .unwrap();
            let mut bytes = Vec::new();
            let mut header_end = None;
            let mut length = 0;
            loop {
                let mut chunk = [0; 2048];
                let size = stream.read(&mut chunk).unwrap();
                assert!(size > 0 && bytes.len() + size < 65536);
                bytes.extend_from_slice(&chunk[..size]);
                if header_end.is_none()
                    && let Some(index) = bytes.windows(4).position(|value| value == b"\r\n\r\n")
                {
                    header_end = Some(index + 4);
                    let headers = String::from_utf8_lossy(&bytes[..index]);
                    length = headers
                        .lines()
                        .find_map(|line| {
                            line.to_ascii_lowercase()
                                .strip_prefix("content-length:")
                                .map(|value| value.trim().parse::<usize>().unwrap())
                        })
                        .unwrap_or(0);
                }
                if header_end.is_some_and(|end| bytes.len() >= end + length) {
                    break;
                }
            }
            let end = header_end.unwrap();
            let headers = String::from_utf8_lossy(&bytes[..end]);
            observed.push((
                headers.lines().next().unwrap().to_owned(),
                if length == 0 {
                    Value::Null
                } else {
                    serde_json::from_slice(&bytes[end..end + length]).unwrap()
                },
            ));
            let response = response.to_string();
            write!(stream, "HTTP/1.1 {status} Test\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{response}", response.len()).unwrap();
        }
        observed
    });
    (Github::for_test(url).unwrap(), handle)
}

fn request() -> DispatchV1 {
    let mut request = crate::tests::request();
    request.version = 2;
    request.trigger = Some("mention".into());
    request.branch = Some("golems/hello".into());
    request.queue_generation = Some("23456789-1234-1234-1234-123456789abc".into());
    request
}

fn reference(sha: &str) -> Value {
    json!({"ref":"refs/heads/golems/hello", "object":{"type":"commit", "sha":sha}})
}

fn pull(request: &DispatchV1, sha: &str) -> Value {
    json!({"number":1, "body":Github::ownership_marker(request), "user":{"login":"isp-golem-xiv[bot]"}, "head":{"ref":"golems/hello", "sha":sha, "repo":{"id":123}}})
}

#[tokio::test]
async fn snapshot_selects_only_an_owned_real_branch_and_preserves_dispatch_head() {
    let request = request();
    let sha = "b".repeat(40);
    let (github, seen) = server(vec![
        (200, reference(&sha)),
        (200, json!([pull(&request, &sha)])),
    ]);
    assert_eq!(
        github
            .branch_snapshot(&request, "read-token")
            .await
            .unwrap(),
        sha
    );
    assert_eq!(request.head_sha, "a".repeat(40));
    assert_eq!(seen.join().unwrap().len(), 2);
    for responses in [
        vec![(200, reference("bad"))],
        vec![(200, reference(&sha)), (200, json!([]))],
        vec![
            (200, reference(&sha)),
            (200, json!([pull(&request, &"c".repeat(40))])),
        ],
    ] {
        let (github, seen) = server(responses);
        assert!(
            github
                .branch_snapshot(&request, "read-token")
                .await
                .is_err()
        );
        seen.join().unwrap();
    }
    let (github, seen) = server(vec![(404, json!({}))]);
    assert_eq!(
        github
            .branch_snapshot(&request, "read-token")
            .await
            .unwrap(),
        request.head_sha
    );
    seen.join().unwrap();
}

#[tokio::test]
async fn blocked_label_requires_owned_open_pull() {
    let request = request();
    let sha = request.head_sha.clone();
    for (responses, creates) in [
        (vec![(200, json!({})), (200, json!([]))], false),
        (
            vec![(404, json!({})), (201, json!({})), (200, json!([]))],
            true,
        ),
        (
            vec![
                (404, json!({})),
                (422, json!({})),
                (200, json!({})),
                (200, json!([])),
            ],
            true,
        ),
    ] {
        let mut all = vec![(200, json!([pull(&request, &sha)]))];
        all.extend(responses);
        let (github, seen) = server(all);
        github.mark_blocked(&request, "token").await.unwrap();
        let seen = seen.join().unwrap();
        assert!(seen[0].0.contains("/pulls?state=open"));
        assert!(seen[1].0.contains("/labels/blocked "));
        if creates {
            assert!(seen[2].0.starts_with("POST "));
            assert!(seen[2].0.contains("/labels "));
            assert_eq!(seen[2].1, json!({"name":"blocked", "color":"d73a4a"}));
        }
        let last = seen.last().unwrap();
        assert!(last.0.contains("/issues/1/labels "));
        assert_eq!(last.1, json!({"labels":["blocked"]}));
    }
    for responses in [
        vec![(503, json!({}))],
        vec![(404, json!({})), (503, json!({}))],
        vec![(404, json!({})), (422, json!({})), (404, json!({}))],
    ] {
        let mut all = vec![(200, json!([pull(&request, &sha)]))];
        all.extend(responses);
        let (github, seen) = server(all);
        assert!(github.mark_blocked(&request, "token").await.is_err());
        assert!(
            seen.join()
                .unwrap()
                .iter()
                .all(|entry| !entry.0.contains("/issues/1/labels "))
        );
    }
    let (github, seen) = server(vec![(200, json!([]))]);
    github.mark_blocked(&request, "token").await.unwrap();
    assert_eq!(seen.join().unwrap().len(), 1);
    let mut unowned = pull(&request, &sha);
    unowned["user"]["login"] = json!("someone-else");
    let (github, seen) = server(vec![(200, json!([unowned]))]);
    assert_eq!(
        github.mark_blocked(&request, "token").await,
        Err("branch ownership mismatch")
    );
    assert_eq!(seen.join().unwrap().len(), 1);
}

#[tokio::test]
async fn all_terminal_outcomes_have_semantic_checks_and_no_change_requires_reason() {
    for (outcome, conclusion) in [
        ("changed", "success"),
        ("replied", "success"),
        ("no_change", "success"),
        ("incomplete", "neutral"),
        ("blocked", "failure"),
        ("stopped", "cancelled"),
    ] {
        let (github, seen) = server(vec![(200, json!({}))]);
        github
            .check_reason(
                &request(),
                "token",
                outcome,
                "observed-model",
                "99",
                Some("nothing pending"),
                &OutputGuard::default(),
            )
            .await
            .unwrap();
        let seen = seen.join().unwrap();
        assert_eq!(seen[0].1["conclusion"], conclusion);
        let summary = seen[0].1["output"]["summary"].as_str().unwrap();
        assert!(summary.contains(&format!("outcome: {outcome}")));
        assert!(summary.contains("model: observed-model"));
        if outcome == "no_change" {
            assert!(summary.contains("reason: nothing pending"));
        }
    }
    let github = Github::for_test("http://127.0.0.1:1".into()).unwrap();
    for reason in [None, Some("  ")] {
        assert!(
            github
                .check_reason(
                    &request(),
                    "token",
                    "no_change",
                    "model",
                    "99",
                    reason,
                    &OutputGuard::default()
                )
                .await
                .is_err()
        );
    }
}

#[tokio::test]
async fn schedule_validates_seat_check_and_pinned_definition_without_comment_metadata() {
    let mut request = request();
    request.trigger = Some("schedule".into());
    request.owner = Some("member".into());
    request.actor_login = None;
    request.actor_id = None;
    request.issue_number = None;
    request.comment_id = None;
    for (branch, valid) in [("golems/hello", true), ("other", false)] {
        let definition = format!(
            "---\nowner: member\nschedule: daily\ntimeout: 1m\nbranch: {branch}\n---\nInspect repository\n"
        );
        let (github, seen) = server(vec![
            (
                200,
                json!({"id":123,"full_name":request.repository,"topics":["isp-golem-xiv-test"]}),
            ),
            (204, Value::Null),
            (
                200,
                json!({"head_sha":request.head_sha,"name":"golem/hello","app":{"id":APP_ID}}),
            ),
            (
                200,
                json!({"type":"file","encoding":"base64","content":general_purpose::STANDARD.encode(definition)}),
            ),
        ]);
        assert_eq!(
            github.validate_target(&request, "token").await.is_ok(),
            valid
        );
        let seen = seen.join().unwrap();
        assert!(seen[1].0.contains("/members/member "));
        assert!(seen[3].0.contains(&format!("?ref={} ", request.head_sha)));
    }
}

#[test]
fn queue_signs_exact_bytes_normalizes_pem_and_requires_explicit_grant() {
    assert_eq!(
        queue_signature(" key\r\nlines\r\n", b"body").unwrap(),
        queue_signature("key\nlines", b"body").unwrap()
    );
    assert_ne!(
        queue_signature("key", b"body").unwrap(),
        queue_signature("key", b"body ").unwrap()
    );
    // RFC 4231 HMAC-SHA256 test case 2.
    assert_eq!(
        queue_signature("Jefe", b"what do ya want for nothing?").unwrap(),
        "sha256=5bdcc146bf60754e6a042426089575c75a003f089d2739839dec58b964ec3843"
    );
    assert!(validate_queue_response("bind", &json!({"state":"granted"})).is_ok());
    for value in [
        json!({}),
        json!({"state":"pending"}),
        json!({"state":"stop_requested"}),
        json!({"granted":true}),
        json!({"state":"terminal"}),
    ] {
        assert!(validate_queue_response("bind", &value).is_err());
    }
    for action in ["stop", "enqueue", "", "complete "] {
        assert!(queue_body(&request(), "99", "1", action, 1000).is_err());
    }
    for (run, attempt) in [
        ("", "1"),
        ("0", "1"),
        ("99", "2"),
        ("-1", "1"),
        ("9007199254740992", "1"),
    ] {
        assert!(queue_body(&request(), run, attempt, "bind", 1000).is_err());
    }
    let body: Value =
        serde_json::from_slice(&queue_body(&request(), "99", "1", "complete", 1000).unwrap())
            .unwrap();
    assert_eq!(body["request_id"], request().delivery_id);
    assert_eq!(body["generation"], request().queue_generation.unwrap());
    assert_eq!(body["run_id"], 99);
    assert_eq!(body["containers_destroyed"], true);
    assert_eq!(body["publication_complete"], true);
}

#[tokio::test]
async fn publishing_uses_git_objects_nonforce_ref_and_reuses_owned_pull() {
    let request = request();
    let sha = request.head_sha.clone();
    let new_sha = "c".repeat(40);
    let (github, seen) = server(vec![
        (200, json!({"default_branch":"trunk"})),
        (200, reference(&sha)),
        (200, json!([pull(&request, &sha)])),
        (200, json!({"tree":{"sha":"b".repeat(40)}})),
        (201, json!({"sha":"d".repeat(40)})),
        (201, json!({"sha":"e".repeat(40)})),
        (201, json!({"sha":new_sha})),
        (200, reference(&sha)),
        (200, json!({})),
        (200, json!({"html_url":"https://github.com/example/pull/1"})),
    ]);
    let changes = [
        golem_runner::changes::Change {
            path: "src/file".into(),
            file: Some(golem_runner::changes::FileState {
                bytes: vec![0, 255],
                executable: true,
            }),
        },
        golem_runner::changes::Change {
            path: "deleted".into(),
            file: None,
        },
    ];
    let output = AgentOutput {
        text: "implemented".into(),
        model: "observed-model".into(),
        disposition: Some(golem_runner::AgentDisposition::Completed),
    };
    github
        .publish_changes(
            &request,
            "write-token",
            &sha,
            &changes,
            &output,
            &OutputGuard::default(),
        )
        .await
        .unwrap();
    let seen = seen.join().unwrap();
    assert_eq!(seen[4].1["content"], "AP8=");
    assert_eq!(seen[5].1["tree"][0]["mode"], "100755");
    assert!(seen[5].1["tree"][1]["sha"].is_null());
    assert_eq!(seen[6].1["parents"], json!([sha]));
    assert_eq!(seen[8].1["force"], false);
    assert!(seen[9].0.starts_with("PATCH "));
    assert!(
        seen[9].1["body"]
            .as_str()
            .unwrap()
            .contains("seat: member/anthropic\nmodel: observed-model")
    );
}

#[tokio::test]
async fn first_publication_creates_ref_and_pull_but_default_branch_is_never_written() {
    let request = request();
    let output = AgentOutput {
        text: "implemented".into(),
        model: "observed".into(),
        disposition: Some(golem_runner::AgentDisposition::Completed),
    };
    let changes = [golem_runner::changes::Change {
        path: "file".into(),
        file: Some(golem_runner::changes::FileState {
            bytes: b"content".to_vec(),
            executable: false,
        }),
    }];
    let (github, seen) = server(vec![
        (200, json!({"default_branch":"trunk"})),
        (404, json!({})),
        (200, json!([])),
        (200, json!({"tree":{"sha":"b".repeat(40)}})),
        (201, json!({"sha":"c".repeat(40)})),
        (201, json!({"sha":"d".repeat(40)})),
        (201, json!({"sha":"e".repeat(40)})),
        (404, json!({})),
        (201, json!({})),
        (201, json!({"html_url":"https://github.com/example/pull/1"})),
    ]);
    github
        .publish_changes(
            &request,
            "token",
            &request.head_sha,
            &changes,
            &output,
            &OutputGuard::default(),
        )
        .await
        .unwrap();
    let seen = seen.join().unwrap();
    assert_eq!(seen[8].1["ref"], "refs/heads/golems/hello");
    assert!(seen[8].0.starts_with("POST "));
    assert_eq!(seen[9].1["base"], "trunk");
    assert_eq!(seen[9].1["head"], "golems/hello");
    let (github, seen) = server(vec![(200, json!({"default_branch":"golems/hello"}))]);
    assert!(
        github
            .publish_changes(
                &request,
                "token",
                &request.head_sha,
                &changes,
                &output,
                &OutputGuard::default()
            )
            .await
            .is_err()
    );
    assert_eq!(seen.join().unwrap().len(), 1);
}

#[tokio::test]
async fn forbidden_edits_and_handled_secrets_never_reach_network() {
    let github = Github::for_test("http://127.0.0.1:1".into()).unwrap();
    let request = request();
    let mut guard = OutputGuard::default();
    guard.add("handled-private-value");
    let output = AgentOutput {
        text: "safe".into(),
        model: "observed".into(),
        disposition: Some(golem_runner::AgentDisposition::Completed),
    };
    for (path, bytes) in [
        (".golems/task.md", b"safe".to_vec()),
        ("src/file", b"handled-private-value".to_vec()),
    ] {
        let changes = [golem_runner::changes::Change {
            path: path.into(),
            file: Some(golem_runner::changes::FileState {
                bytes,
                executable: false,
            }),
        }];
        assert!(
            github
                .publish_changes(
                    &request,
                    "token",
                    &request.head_sha,
                    &changes,
                    &output,
                    &guard
                )
                .await
                .is_err()
        );
    }
}

#[test]
fn queue_body_serializes_run_id_and_attempt_as_json_numbers_within_safe_integer() {
    let mut request = request();
    request.queue_generation = Some("aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee".into());
    // Max safe integer (2^53 - 1) — coordinator's Number.isSafeInteger boundary.
    let max_safe = 9_007_199_254_740_991u64;
    for (action, run_id, expected_attempt) in [("bind", max_safe, 1u64), ("complete", 12345, 1)] {
        let body = queue_body(
            &request,
            &run_id.to_string(),
            &expected_attempt.to_string(),
            action,
            1_700_000_000_000u128,
        )
        .unwrap();
        let value: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(value["action"], action);
        assert_eq!(value["request_id"], request.delivery_id);
        assert_eq!(
            value["generation"].as_str(),
            request.queue_generation.as_deref()
        );
        // run_id must be a JSON number, not a string or float.
        let run_id_val = &value["run_id"];
        assert!(
            run_id_val.is_number(),
            "run_id must be a JSON number, got {run_id_val}"
        );
        assert!(
            run_id_val.as_i64().is_some() || run_id_val.as_u64().is_some(),
            "run_id must fit in i64/u64, got {run_id_val}"
        );
        // attempt must be a JSON number 1.
        let attempt_val = &value["attempt"];
        assert!(
            attempt_val.is_number(),
            "attempt must be a JSON number, got {attempt_val}"
        );
        assert_eq!(attempt_val, &1);
        // timestamp_ms must be a JSON number.
        assert!(value["timestamp_ms"].is_number());
        // complete action includes boolean attestations.
        if action == "complete" {
            assert_eq!(value["containers_destroyed"], true);
            assert_eq!(value["publication_complete"], true);
        }
    }
    // Reject run_id that exceeds safe integer.
    let too_large = (9_007_199_254_740_992u64).to_string();
    assert!(
        queue_body(&request, &too_large, "1", "bind", 1_700_000_000_000u128).is_err(),
        "run_id exceeding 2^53-1 must be rejected"
    );
}

#[test]
fn seat_write_back_token_is_scoped_to_ops_secrets_only() {
    assert_eq!(
        seat_write_token_request(),
        json!({"repositories":["isp-golem-xiv-ops"], "permissions":{"secrets":"write"}})
    );
    let request = request();
    for publish in [false, true] {
        let target = target_token_request(&request, publish);
        assert_eq!(target["repository_ids"], json!([request.repository_id]));
        assert!(target.get("repositories").is_none());
        assert!(target["permissions"].get("secrets").is_none());
    }
}

// libsodium 1.0.20 keypair from the 32-byte seed 0x07; an invented test key.
const OPS_PUBLIC: &str = "dh2I7IMEE5Gd/p1NHVbxfmU8jJlAgt9bE3uQoK5u33Q=";
const OPS_SECRET: &str = "L605/v1/o+IAqcYm7vWZ5hotBVxIqCiKTn5MS8o5KPg=";

fn open_sealed(sealed: &str) -> Vec<u8> {
    let key = |text: &str| -> [u8; 32] {
        general_purpose::STANDARD
            .decode(text)
            .unwrap()
            .try_into()
            .unwrap()
    };
    let sealed = general_purpose::STANDARD.decode(sealed).unwrap();
    let mut message = vec![0; sealed.len() - dryoc::constants::CRYPTO_BOX_SEALBYTES];
    dryoc::classic::crypto_box::crypto_box_seal_open(
        &mut message,
        &sealed,
        &key(OPS_PUBLIC),
        &key(OPS_SECRET),
    )
    .unwrap();
    message
}

#[tokio::test]
async fn seat_write_back_puts_only_the_sealed_sign_in_to_the_named_ops_secret() {
    let sign_in = r#"{"tokens":{"refresh_token":"rt_fixture-refreshed"}}"#;
    for status in [201, 204] {
        let (github, seen) = server(vec![
            (
                200,
                json!({"key_id":"568250167242549743", "key":OPS_PUBLIC}),
            ),
            (status, json!({})),
        ]);
        github
            .put_seat_secret("ops-secret-token", "SEAT_MEMBER__OPENAI_CODEX", sign_in)
            .await
            .unwrap();
        let seen = seen.join().unwrap();
        assert_eq!(
            seen[0].0,
            "GET /repos/isp-insoft-gmbh/isp-golem-xiv-ops/actions/secrets/public-key HTTP/1.1"
        );
        assert_eq!(
            seen[1].0,
            "PUT /repos/isp-insoft-gmbh/isp-golem-xiv-ops/actions/secrets/SEAT_MEMBER__OPENAI_CODEX HTTP/1.1"
        );
        assert_eq!(seen[1].1["key_id"], "568250167242549743");
        assert_eq!(seen[1].1.as_object().unwrap().len(), 2);
        assert!(!seen[1].1.to_string().contains("rt_fixture-refreshed"));
        assert_eq!(
            open_sealed(seen[1].1["encrypted_value"].as_str().unwrap()),
            sign_in.as_bytes()
        );
    }
    let (github, seen) = server(vec![
        (200, json!({"key_id":"1", "key":OPS_PUBLIC})),
        (422, json!({})),
    ]);
    assert_eq!(
        github
            .put_seat_secret("token", "SEAT_MEMBER__OPENAI_CODEX", sign_in)
            .await,
        Err("seat write-back rejected")
    );
    assert_eq!(seen.join().unwrap().len(), 2);
    for key in [
        json!({"key_id":"1", "key":"AAAA"}),
        json!({"key":OPS_PUBLIC}),
        json!({"key_id":"../x", "key":OPS_PUBLIC}),
    ] {
        let (github, seen) = server(vec![(200, key)]);
        assert!(
            github
                .put_seat_secret("token", "SEAT_MEMBER__OPENAI_CODEX", sign_in)
                .await
                .is_err()
        );
        assert_eq!(
            seen.join().unwrap().len(),
            1,
            "never PUT without a valid key"
        );
    }
    let (github, seen) = server(vec![]);
    assert_eq!(
        github
            .put_seat_secret("token", "APP_PRIVATE_KEY", sign_in)
            .await,
        Err("invalid seat secret name")
    );
    assert_eq!(
        seen.join().unwrap(),
        [] as [(std::string::String, serde_json::Value); 0]
    );
}

fn dashboard(body: &str) -> Value {
    json!({"number":12, "title":"Golems", "user":{"login":"isp-golem-xiv[bot]"}, "body":body})
}

fn dashboard_body() -> String {
    router::dashboard_body(&[router::parse_golem("hello", "---\n---\nHi.")], None, &[])
}

#[tokio::test]
async fn dashboard_last_result_rereads_the_apps_dashboard_and_patches_only_its_golem() {
    let request = request();
    let body = dashboard_body();
    let decoy = json!({"number":3, "title":"Golems", "user":{"login":"member"}, "body":body});
    let (github, seen) = server(vec![
        (200, json!([decoy, dashboard(&body)])),
        (200, dashboard(&body)),
        (200, json!({})),
    ]);
    let status = github
        .update_dashboard(
            &request,
            "publish-token",
            ("replied", "claude-opus-5-5"),
            "4242",
            &OutputGuard::default(),
        )
        .await;
    assert_eq!(status, "updated");
    let seen = seen.join().unwrap();
    assert_eq!(
        seen.iter()
            .map(|(line, _)| line.as_str())
            .collect::<Vec<_>>(),
        [
            "GET /repos/isp-insoft-gmbh/isp-golem-xiv-test-example/issues?state=open&per_page=100 HTTP/1.1",
            "GET /repos/isp-insoft-gmbh/isp-golem-xiv-test-example/issues/12 HTTP/1.1",
            "PATCH /repos/isp-insoft-gmbh/isp-golem-xiv-test-example/issues/12 HTTP/1.1",
        ]
    );
    assert_eq!(
        seen[2].1.as_object().unwrap().len(),
        1,
        "only the body changes"
    );
    let patched = seen[2].1["body"].as_str().unwrap();
    let results = router::parse_last_results(patched);
    assert_eq!(results.len(), 1);
    let (golem, last) = &results[0];
    assert_eq!(golem, "hello");
    assert_eq!(
        (
            last.outcome.as_str(),
            last.login.as_str(),
            last.provider.as_str(),
            last.harness.as_str(),
            last.model.as_str(),
            last.run_id
        ),
        (
            "replied",
            "member",
            "anthropic",
            "claude",
            "claude-opus-5-5",
            4242
        )
    );
    assert!(
        patched.contains(
            "[log](https://github.com/isp-insoft-gmbh/isp-golem-xiv-ops/actions/runs/4242)"
        )
    );
    assert_eq!(
        patched
            .lines()
            .filter(|line| !line.contains("golem-last") && !line.starts_with("  Last: "))
            .collect::<Vec<_>>(),
        body.lines()
            .filter(|line| !line.contains("golem-last") && !line.starts_with("  Last: "))
            .collect::<Vec<_>>(),
        "everything else stays"
    );
    // An invalid observed model is shown as unobserved rather than dropping the result.
    let (github, seen) = server(vec![
        (200, json!([dashboard(&body)])),
        (200, dashboard(&body)),
        (200, json!({})),
    ]);
    assert_eq!(
        github
            .update_dashboard(
                &request,
                "token",
                ("blocked", "model with spaces"),
                "7",
                &OutputGuard::default()
            )
            .await,
        "updated"
    );
    assert!(
        seen.join().unwrap()[2].1["body"]
            .as_str()
            .unwrap()
            .contains("model=unobserved")
    );
}

#[tokio::test]
async fn dashboard_last_result_is_best_effort_not_found_or_failed() {
    let request = request();
    let body = dashboard_body();
    let guard = OutputGuard::default();
    for (responses, expected, calls) in [
        (vec![(200, json!([]))], "not found", 1),
        (
            vec![(
                200,
                json!([dashboard(&body.replace(router::DASHBOARD_MARKER, ""))]),
            )],
            "not found",
            1,
        ),
        (
            vec![
                (200, json!([dashboard(&body)])),
                (200, dashboard(&body.replace("golem-last golem=hello", "x"))),
            ],
            "not found",
            2,
        ),
        (
            vec![
                (200, json!([dashboard(&body)])),
                (200, dashboard(&body.replace("hello", "other"))),
            ],
            "not found",
            2,
        ),
        (vec![(500, json!({}))], "failed", 1),
        (vec![(200, json!({"not":"a list"}))], "failed", 1),
        (
            vec![(200, json!([dashboard(&body)])), (404, json!({}))],
            "failed",
            2,
        ),
        (
            vec![
                (200, json!([dashboard(&body)])),
                (200, dashboard(&body)),
                (409, json!({})),
            ],
            "failed",
            3,
        ),
    ] {
        let (github, seen) = server(responses);
        assert_eq!(
            github
                .update_dashboard(&request, "token", ("changed", "m"), "1", &guard)
                .await,
            expected
        );
        assert_eq!(seen.join().unwrap().len(), calls);
    }
    let (github, seen) = server(vec![]);
    assert_eq!(
        github
            .update_dashboard(&request, "token", ("changed", "m"), "not-a-run", &guard)
            .await,
        "failed",
        "invalid inputs never reach GitHub"
    );
    assert!(seen.join().unwrap().is_empty(), "no requests");
}

#[tokio::test]
async fn dashboard_requests_bind_the_reply_to_the_apps_dashboard_instead_of_a_comment() {
    let mut request = request();
    request.trigger = Some("dashboard".into());
    request.comment_id = None;
    let (github, seen) = server(vec![(200, dashboard(&dashboard_body()))]);
    github.validate_human(&request, "read-token").await.unwrap();
    assert_eq!(
        seen.join().unwrap()[0].0,
        "GET /repos/isp-insoft-gmbh/isp-golem-xiv-test-example/issues/7 HTTP/1.1"
    );
    for issue in [
        json!({"number":7, "title":"Golems", "user":{"login":"member"}, "body":dashboard_body()}),
        json!({"number":7, "title":"Other", "user":{"login":"isp-golem-xiv[bot]"}, "body":dashboard_body()}),
        json!({"number":7, "title":"Golems", "user":{"login":"isp-golem-xiv[bot]"}, "body":"no marker"}),
        json!({"number":7, "title":"Golems", "user":{"login":"isp-golem-xiv[bot]"}, "body":dashboard_body(), "pull_request":{}}),
    ] {
        let (github, seen) = server(vec![(200, issue)]);
        assert_eq!(
            github.validate_human(&request, "read-token").await,
            Err("dashboard identity mismatch")
        );
        seen.join().unwrap();
    }
    assert!(
        github_feedback(&request).await,
        "dashboard runs always answer"
    );
}

async fn github_feedback(request: &DispatchV1) -> bool {
    let (github, seen) = server(vec![]);
    let feedback = github.feedback_eligible(request, "token").await.unwrap();
    assert!(seen.join().unwrap().is_empty(), "no comment lookup");
    feedback
}

#[tokio::test]
async fn only_final_check_updates_record_the_published_outcome() {
    let request = request();
    let guard = OutputGuard::default();
    let (github, seen) = server(vec![
        (200, json!({})),
        (500, json!({})),
        (500, json!({})),
        (500, json!({})),
        (200, json!({})),
    ]);
    github
        .check(&request, "token", "running", "default", "1", &guard)
        .await
        .unwrap();
    assert_eq!(request.published.take(), None, "running is not an outcome");
    assert!(
        github
            .check(&request, "token", "replied", "m", "1", &guard)
            .await
            .is_err()
    );
    assert_eq!(
        request.published.take(),
        None,
        "an unpublished check records nothing"
    );
    github
        .check(&request, "token", "blocked", "m", "1", &guard)
        .await
        .unwrap();
    assert_eq!(
        request.published.take(),
        Some(("blocked".to_owned(), "m".to_owned()))
    );
    assert_eq!(seen.join().unwrap().len(), 5);
}
