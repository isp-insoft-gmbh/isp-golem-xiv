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
