// Fixtures fail loudly; production paths never unwrap untrusted input.
#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

use super::*;
use serde_json::{Value, json};
use std::{
    io::{Read, Write},
    net::TcpListener,
    thread,
    time::{Duration, Instant},
};

type ObservedRequest = (String, Value);

#[tokio::test]
async fn unconfirmed_destruction_prevents_publish_mint_publication_and_queue_completion() {
    use std::cell::RefCell;
    for phase in ["prepare", "agent", "verify"] {
        let events = RefCell::new(Vec::new());
        let result = after_destruction(
            async {
                events.borrow_mut().push(phase);
                Err::<Result<WritableResult>, _>(
                    "container destruction unconfirmed; publication prohibited",
                )
            },
            async |_| {
                events.borrow_mut().push("mint publish token");
                events.borrow_mut().push("publish");
                events.borrow_mut().push("queue complete");
                Ok(())
            },
        )
        .await;
        assert_eq!(
            result,
            Err("container destruction unconfirmed; publication prohibited")
        );
        assert_eq!(*events.borrow(), [phase]);
    }
    let events = RefCell::new(Vec::new());
    after_destruction(
        async {
            events.borrow_mut().push("destruction confirmed");
            Ok(())
        },
        async |()| {
            events
                .borrow_mut()
                .extend(["mint publish token", "publish", "queue complete"]);
            Ok(())
        },
    )
    .await
    .unwrap();
    assert_eq!(
        *events.borrow(),
        [
            "destruction confirmed",
            "mint publish token",
            "publish",
            "queue complete"
        ]
    );
}

#[tokio::test]
async fn writable_no_changes_answer_humans_but_unattended_runs_only_record_reason() {
    for (human, timed_out, outcome) in [
        (true, false, "replied"),
        (false, false, "no_change"),
        (true, true, "incomplete"),
    ] {
        let mut request = request();
        request.version = 2;
        request.trigger = Some(if human { "mention" } else { "event" }.into());
        let mut responses = Vec::new();
        if human {
            responses.push((201, json!({})));
        }
        responses.push((200, json!({})));
        let (github, observed) = server(responses);
        let context = RunContext {
            github: &github,
            request: &request,
            private_key: "unused",
            run_id: "99",
            attempt: "1",
        };
        context
            .publish_writable(
                "test-write-token",
                Ok(WritableResult {
                    base_sha: request.head_sha.clone(),
                    changes: vec![],
                    output: AgentOutput {
                        text: "No changes needed: existing behavior meets task.".into(),
                        model: "observed-model".into(),
                    },
                    timed_out,
                }),
                &OutputGuard::default(),
            )
            .await
            .unwrap()
            .unwrap();
        let observed = observed.join().unwrap().unwrap();
        assert_eq!(observed.len(), if human { 2 } else { 1 });
        if human {
            assert!(observed[0].0.contains("/issues/7/comments "));
        }
        let check = observed.last().unwrap();
        assert!(check.0.contains("/check-runs/55 "));
        let summary = check.1["output"]["summary"].as_str().unwrap();
        assert!(summary.contains(&format!("outcome: {outcome}")));
        assert!(summary.contains("model: observed-model"));
        if !human {
            assert!(summary.contains("reason: No changes needed:"));
        }
    }
}

#[tokio::test]
async fn timed_out_partial_changes_publish_incomplete_with_observed_model_only() {
    let (github, observed) = server(vec![
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
        (201, json!({})),
        (200, json!({})),
    ]);
    let mut request = request();
    request.version = 2;
    request.trigger = Some("mention".into());
    request.model = Some("requested-not-observed".into());
    let context = RunContext {
        github: &github,
        request: &request,
        private_key: "unused",
        run_id: "99",
        attempt: "1",
    };
    let guard = OutputGuard::default();
    context
        .publish_writable(
            "test-write-token",
            Ok(WritableResult {
                base_sha: request.head_sha.clone(),
                changes: vec![golem_runner::changes::Change {
                    path: "file.txt".into(),
                    file: Some(golem_runner::changes::FileState {
                        bytes: b"partial change".to_vec(),
                        executable: false,
                    }),
                }],
                output: golem_runner::partial_agent_result(b"", b"", &guard).unwrap(),
                timed_out: true,
            }),
            &guard,
        )
        .await
        .unwrap()
        .unwrap();
    let observed = observed.join().unwrap().unwrap();
    let pull_body = observed[9].1["body"].as_str().unwrap();
    assert!(pull_body.contains("Run incomplete: agent wall-clock budget exhausted."));
    assert!(pull_body.contains("model: unobserved"));
    assert!(!pull_body.contains("requested-not-observed"));
    assert!(
        observed[10].1["body"]
            .as_str()
            .unwrap()
            .contains("Pull request:")
    );
    let summary = observed[11].1["output"]["summary"].as_str().unwrap();
    assert!(summary.contains("outcome: incomplete"));
    assert!(summary.contains("model: unobserved"));
    assert_eq!(observed[11].1["conclusion"], "neutral");
}

#[tokio::test]
async fn publication_failure_retains_queue_claim_even_when_blocked_feedback_succeeds() {
    let (github, observed) = server(vec![
        (503, json!({})),
        (200, json!([])),
        (201, json!({})),
        (200, json!({})),
    ]);
    let mut request = request();
    request.version = 2;
    request.trigger = Some("mention".into());
    let context = RunContext {
        github: &github,
        request: &request,
        private_key: "unused",
        run_id: "99",
        attempt: "1",
    };
    let result = context
        .publish_writable(
            "test-write-token",
            Ok(WritableResult {
                base_sha: request.head_sha.clone(),
                changes: vec![golem_runner::changes::Change {
                    path: "file.txt".into(),
                    file: Some(golem_runner::changes::FileState {
                        bytes: b"change".to_vec(),
                        executable: false,
                    }),
                }],
                output: AgentOutput {
                    text: "Made change".into(),
                    model: "observed-model".into(),
                },
                timed_out: true,
            }),
            &OutputGuard::default(),
        )
        .await;
    // The supervisor's outcome? must fail before its complete(true, true) call.
    assert_eq!(result, Err("GitHub rejected request"));
    let observed = observed.join().unwrap().unwrap();
    assert_eq!(observed.len(), 4);
    assert!(observed[0].0.starts_with("GET /repos/"));
    assert!(observed[1].0.contains("/pulls?"));
    assert!(observed[2].0.contains("/issues/7/comments "));
    assert!(
        observed[2].1["body"]
            .as_str()
            .unwrap()
            .contains("Run blocked:")
    );
    assert_eq!(observed[3].1["conclusion"], "failure");
}

#[tokio::test]
async fn safe_writable_errors_attempt_feedback_and_check_and_require_both_for_completion() {
    for reply_status in [201, 503] {
        let (github, observed) = server(vec![
            (200, json!([])),
            (reply_status, json!({})),
            (200, json!({})),
        ]);
        let mut request = request();
        request.version = 2;
        request.trigger = Some("mention".into());
        request.model = Some("requested-but-unobserved".into());
        let context = RunContext {
            github: &github,
            request: &request,
            private_key: "unused",
            run_id: "99",
            attempt: "1",
        };
        let result = context
            .publish_writable(
                "test-write-token",
                Err("verification mutated workspace; publication blocked"),
                &OutputGuard::default(),
            )
            .await;
        if reply_status == 201 {
            assert_eq!(
                result,
                Ok(Err("verification mutated workspace; publication blocked"))
            );
        } else {
            assert!(result.is_err());
        }
        let observed = observed.join().unwrap().unwrap();
        assert!(observed[1].0.contains("/comments "));
        assert_eq!(observed[2].1["conclusion"], "failure");
        let summary = observed[2].1["output"]["summary"].as_str().unwrap();
        assert!(summary.contains("model: unobserved"));
        assert!(!summary.contains("requested-but-unobserved"));
        for proofs in [(false, true), (true, false), (false, false)] {
            assert_eq!(
                context.complete(proofs.0, proofs.1).await,
                Err("queue completion requires destruction and publication proofs")
            );
        }
    }
}

#[tokio::test]
async fn blocked_label_failure_does_not_suppress_human_reply_or_check() {
    let mut request = request();
    request.version = 2;
    request.trigger = Some("mention".into());
    let pull = json!({"number":1, "body":format!("<!-- isp-golem-xiv repository={} golem={} -->", request.repository_id, request.golem), "user":{"login":"isp-golem-xiv[bot]"}, "head":{"ref":request.branch_name(), "repo":{"id":request.repository_id}}});
    for reply_status in [201, 503] {
        let (github, observed) = server(vec![
            (200, json!([pull])),
            (503, json!({})),
            (reply_status, json!({})),
            (200, json!({})),
        ]);
        let context = RunContext {
            github: &github,
            request: &request,
            private_key: "unused",
            run_id: "99",
            attempt: "1",
        };
        assert_eq!(
            context
                .report_blocked(
                    "token",
                    "verification failed",
                    "model",
                    &OutputGuard::default()
                )
                .await,
            Err("GitHub rejected request")
        );
        let observed = observed.join().unwrap().unwrap();
        assert!(observed[1].0.contains("/labels/blocked "));
        assert!(observed[2].0.contains("/issues/7/comments "));
        assert_eq!(observed[3].1["conclusion"], "failure");
    }
}

fn server(
    responses: Vec<(u16, Value)>,
) -> (
    github::Github,
    thread::JoinHandle<std::io::Result<Vec<ObservedRequest>>>,
) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    listener.set_nonblocking(true).unwrap();
    let handle = thread::spawn(move || {
        let mut observed = Vec::new();
        for (status, body) in responses {
            let deadline = Instant::now() + Duration::from_secs(5);
            let mut stream = loop {
                match listener.accept() {
                    Ok((stream, _)) => break stream,
                    Err(error)
                        if error.kind() == std::io::ErrorKind::WouldBlock
                            && Instant::now() < deadline =>
                    {
                        thread::sleep(Duration::from_millis(2));
                    }
                    Err(error) => return Err(error),
                }
                if Instant::now() >= deadline {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::TimedOut,
                        "expected request was not received",
                    ));
                }
            };
            stream.set_nonblocking(false).unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            let mut bytes = Vec::new();
            let headers_end = loop {
                let mut chunk = [0_u8; 2048];
                let size = stream.read(&mut chunk)?;
                if size == 0 || bytes.len() + size > 65_536 {
                    return Err(std::io::Error::other("invalid test request"));
                }
                bytes.extend_from_slice(&chunk[..size]);
                if let Some(offset) = bytes.windows(4).position(|window| window == b"\r\n\r\n") {
                    break offset + 4;
                }
            };
            let headers = String::from_utf8(bytes[..headers_end].to_vec()).unwrap();
            let content_length: usize = headers
                .lines()
                .find_map(|line| {
                    line.to_ascii_lowercase()
                        .strip_prefix("content-length:")
                        .map(str::trim)
                        .map(str::to_owned)
                })
                .unwrap_or_default()
                .parse()
                .unwrap_or(0);
            while bytes.len() < headers_end + content_length {
                let mut chunk = [0_u8; 2048];
                let size = stream.read(&mut chunk)?;
                if size == 0 || bytes.len() + size > 65_536 {
                    return Err(std::io::Error::other("invalid test body"));
                }
                bytes.extend_from_slice(&chunk[..size]);
            }
            let payload = if content_length == 0 {
                Value::Null
            } else {
                serde_json::from_slice(&bytes[headers_end..headers_end + content_length]).unwrap()
            };
            observed.push((headers.lines().next().unwrap().to_owned(), payload));
            let body = if status == 204 {
                String::new()
            } else {
                body.to_string()
            };
            write!(
                stream,
                "HTTP/1.1 {status} Test\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            )?;
        }
        Ok::<_, std::io::Error>(observed)
    });
    (
        github::Github::for_test(format!("http://{address}")).unwrap(),
        handle,
    )
}

pub(super) fn request() -> DispatchV1 {
    let repository = "isp-insoft-gmbh/isp-golem-xiv-test-example";
    DispatchV1::parse(
        &json!({
            "version": 1, "delivery_id": "12345678-1234-1234-1234-123456789abc",
            "repository_id": 123, "repository": repository, "head_sha": "a".repeat(40),
            "golem": "hello", "issue_number": 7, "comment_id": 9, "actor_id": 10,
            "actor_login": "member", "seat_login": "member", "provider": "anthropic",
            "harness": "claude", "model": null, "timeout_seconds": 60,
            "instructions": "report findings", "check_run_id": 55
        })
        .to_string(),
        repository,
        "hello",
    )
    .unwrap()
}

#[test]
fn verifier_preserves_inputs_and_excludes_disposable_outputs() {
    let workspace = tempfile::tempdir().unwrap();
    let path = workspace.path().join("file");
    std::fs::write(&path, "original").unwrap();
    let before = golem_runner::changes::scan(workspace.path()).unwrap();
    std::fs::write(&path, "agent change").unwrap();
    let after_agent = golem_runner::changes::scan(workspace.path()).unwrap();
    let guard = OutputGuard::default();
    let changes = verified_changes(workspace.path(), &before, &after_agent, &guard).unwrap();
    assert_eq!(changes.len(), 1);
    assert_eq!(changes[0].file.as_ref().unwrap().bytes, b"agent change");
    for mutation in ["verifier change", "original"] {
        std::fs::write(&path, mutation).unwrap();
        assert_eq!(
            verified_changes(workspace.path(), &before, &after_agent, &guard),
            Err("verification mutated workspace; publication blocked")
        );
    }
    std::fs::write(&path, "agent change").unwrap();
    std::fs::write(workspace.path().join("new-verifier-file"), "generated").unwrap();
    let published = verified_changes(workspace.path(), &before, &after_agent, &guard).unwrap();
    assert_eq!(published, changes);
    std::fs::write(workspace.path().join(".mcp.json"), "{}").unwrap();
    assert!(verified_changes(workspace.path(), &before, &after_agent, &guard).is_err());
    std::fs::remove_file(workspace.path().join(".mcp.json")).unwrap();
    std::fs::remove_file(&path).unwrap();
    assert!(verified_changes(workspace.path(), &before, &after_agent, &guard).is_err());
}

#[test]
fn writable_prompt_uses_pinned_definition_and_house_rules_not_continuation_config() {
    let workspace = tempfile::tempdir().unwrap();
    let configuration = tempfile::tempdir().unwrap();
    for root in [workspace.path(), configuration.path()] {
        std::fs::create_dir(root.join(".golems")).unwrap();
    }
    std::fs::write(
        workspace.path().join(".golems/hello.md"),
        "stale branch task",
    )
    .unwrap();
    std::fs::write(
        workspace.path().join(".golems/_golems.md"),
        "stale branch rules",
    )
    .unwrap();
    std::fs::write(
        configuration.path().join(".golems/hello.md"),
        "---\n---\nPinned task sentinel",
    )
    .unwrap();
    std::fs::write(
        configuration.path().join(".golems/_golems.md"),
        "Pinned rules sentinel",
    )
    .unwrap();
    std::fs::write(
        workspace.path().join("CLAUDE.md"),
        "Continuation instructions sentinel",
    )
    .unwrap();
    let mut request = request();
    request.version = 2;
    let base_sha = "b".repeat(40);
    let prompt =
        snapshot::writable_prompt(workspace.path(), configuration.path(), &request, &base_sha)
            .unwrap();
    for required in [
        "Pinned task sentinel",
        "Pinned rules sentinel",
        "Continuation instructions sentinel",
        "writable repository snapshot",
        base_sha.as_str(),
    ] {
        assert!(prompt.contains(required), "missing {required}");
    }
    for forbidden in [
        "stale branch task",
        "stale branch rules",
        "Slice 1 permits replies only",
        "read-only repository snapshot",
    ] {
        assert!(!prompt.contains(forbidden), "unexpected {forbidden}");
    }
}

#[tokio::test]
async fn target_requires_the_allowlist_topic_not_a_repository_name_prefix() {
    let request = request();
    let (github, observed) = server(vec![(
        200,
        json!({"id":123, "full_name":request.repository, "topics":["unrelated"]}),
    )]);
    assert_eq!(
        github.validate_target(&request, "test-read-token").await,
        Err("target identity or allowlist mismatch")
    );
    assert_eq!(observed.join().unwrap().unwrap().len(), 1);
}

#[tokio::test]
async fn failed_snapshot_after_validated_target_completes_check_and_answers_requester() {
    let request = request();
    let (github, observed) = server(vec![
        (
            200,
            json!({"id":123, "full_name":request.repository, "topics":["isp-golem-xiv-test"]}),
        ),
        (204, Value::Null),
        (
            200,
            json!({"user":{"id":10,"login":"member"}, "issue_url":"https://api.github.com/repos/isp-insoft-gmbh/isp-golem-xiv-test-example/issues/7"}),
        ),
        (
            200,
            json!({"head_sha":request.head_sha, "name":"golem/hello", "app":{"id":golem_runner::APP_ID}}),
        ),
        (500, json!({})),
        (200, json!({})),
        (201, json!({})),
    ]);
    github
        .validate_target(&request, "test-read-token")
        .await
        .unwrap();
    let guard = OutputGuard::default();
    let result = execute_agent(
        &github,
        &request,
        "test-read-token",
        "test-seat",
        "golem-99-1",
        &guard,
    )
    .await
    .unwrap();
    assert_eq!(result, Err("snapshot redirect missing"));
    assert!(
        publish_result(&github, &request, "test-write-token", "99", result, &guard)
            .await
            .is_err()
    );
    let observed = observed.join().unwrap().unwrap();
    assert!(
        observed[4]
            .0
            .contains("/tarball/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")
    );
    assert!(
        observed[5]
            .0
            .starts_with("PATCH /repos/isp-insoft-gmbh/isp-golem-xiv-test-example/check-runs/55 ")
    );
    assert_eq!(observed[5].1["status"], "completed");
    assert_eq!(observed[5].1["conclusion"], "failure");
    assert!(
        observed[5].1["output"]["summary"]
            .as_str()
            .unwrap()
            .contains("outcome: blocked")
    );
    assert!(observed[6].0.contains("/issues/7/comments "));
    assert!(
        observed[6].1["body"]
            .as_str()
            .unwrap()
            .starts_with("<!-- isp-golem-xiv kind=reply golem=hello -->\n\nRun blocked:")
    );
}

#[tokio::test]
async fn exhausted_check_retries_do_not_suppress_requester_failure_reply() {
    let (github, observed) = server(vec![
        (503, json!({})),
        (503, json!({})),
        (503, json!({})),
        (201, json!({})),
    ]);
    let result = publish_result(
        &github,
        &request(),
        "test-write-token",
        "99",
        Err("trusted harness preparation failed"),
        &OutputGuard::default(),
    )
    .await;
    assert!(result.is_err());
    let observed = observed.join().unwrap().unwrap();
    assert_eq!(observed.len(), 4);
    assert!(observed[3].0.contains("/issues/7/comments "));
    assert!(
        observed[3].1["body"]
            .as_str()
            .unwrap()
            .contains("trusted harness preparation failed")
    );
}

#[tokio::test]
async fn successful_agent_result_posts_real_answer_and_retries_idempotent_check_completion() {
    let (github, observed) = server(vec![(201, json!({})), (503, json!({})), (200, json!({}))]);
    let request = request();
    let guard = OutputGuard::default();
    let output = agent_result(br#"{"type":"result","subtype":"success","is_error":false,"result":"README.md contains secret-free repository facts. Requested text: abc123.","modelUsage":{"claude-sonnet-4-6":{}}}"#, b"", &guard);
    publish_result(&github, &request, "test-write-token", "99", output, &guard)
        .await
        .unwrap();
    let observed = observed.join().unwrap().unwrap();
    assert!(observed[0].0.contains("/issues/7/comments "));
    assert!(
        observed[0].1["body"]
            .as_str()
            .unwrap()
            .ends_with("Requested text: abc123.")
    );
    for check in &observed[1..] {
        assert!(check.0.starts_with("PATCH "));
        assert_eq!(check.1["status"], "completed");
        assert_eq!(check.1["conclusion"], "success");
        assert_eq!(
            check.1["output"]["summary"],
            "outcome: replied\nseat: member/anthropic\nharness: claude\nmodel: claude-sonnet-4-6\nbudget: 60s\nlog: https://github.com/isp-insoft-gmbh/isp-golem-xiv-ops/actions/runs/99"
        );
    }
    assert_eq!(observed[1].1, observed[2].1);
}
