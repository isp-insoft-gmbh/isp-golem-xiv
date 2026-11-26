// Missing fixture fields should fail the test at the point of use.
#![allow(clippy::unwrap_used)]

use router::{Delivery, EventDelivery, event_instructions, parse_mention, trigger_matches};

#[test]
fn captured_push_without_an_action_matches_the_bare_event() {
    let delivery: EventDelivery =
        serde_json::from_slice(include_bytes!("fixtures/webhooks/push.json")).unwrap();

    assert_eq!(delivery.action, "");
    assert!(delivery.issue.is_none() && delivery.pull_request.is_none());
    assert!(delivery.comment.is_none());
    assert!(trigger_matches(&["push".into()], "push", &delivery.action));
    assert!(!trigger_matches(
        &["push.opened".into()],
        "push",
        &delivery.action
    ));
}

#[test]
fn captured_pull_request_opened_matches_its_action_and_supplies_the_pr_number() {
    let delivery: EventDelivery =
        serde_json::from_slice(include_bytes!("fixtures/webhooks/pull-request-opened.json"))
            .unwrap();

    assert_eq!(delivery.pull_request.unwrap().number, 402);
    assert!(delivery.issue.is_none());
    assert!(trigger_matches(
        &["pull_request.opened".into()],
        "pull_request",
        &delivery.action
    ));
    assert!(!trigger_matches(
        &["pull_request.closed".into()],
        "pull_request",
        &delivery.action
    ));
}

#[test]
fn captured_bot_comment_decodes_as_event_context_not_a_mention() {
    let body = include_bytes!("fixtures/webhooks/issue-comment-created.json");
    let event: EventDelivery = serde_json::from_slice(body).unwrap();
    let comment_delivery: Delivery = serde_json::from_slice(body).unwrap();
    let comment = event.comment.unwrap();

    assert_eq!(event.issue.unwrap().number, 401);
    assert_eq!(comment_delivery.issue.number, 401);
    assert_eq!(comment.id, 501);
    assert_eq!(comment_delivery.comment.id, 501);
    assert_eq!(comment_delivery.action, "created");
    assert_eq!(event.sender.kind, "Bot");
    assert_eq!(comment.user.kind, "Bot");
    assert_eq!(event.sender.id, comment.user.id);
    assert_eq!(event.sender.login, comment.user.login);
    assert!(parse_mention(&comment_delivery.comment.body).is_none());
    assert_eq!(
        event_instructions("issue_comment", &event.action, Some(&comment.body)).as_deref(),
        Some("GitHub event: issue_comment.created\n\nTriggering comment:\nFixture comment.")
    );
}
