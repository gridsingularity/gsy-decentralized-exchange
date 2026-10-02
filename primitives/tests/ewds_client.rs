use anyhow::anyhow;
use primitives::ewds::dto::{EwdsEventEnvelope, EwdsSendMessageDto};
use primitives::ewds::{
    EwdsClient, EwdsClientConfig, EwdsEventTopicConfig, EwdsEventType, EwdsTopicConfig,
};
use serde_json::{json, Value};
use std::env;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn client(server: &MockServer) -> EwdsClient {
    EwdsClient::new(EwdsClientConfig {
        gateway_base: server.uri(),
        request_fqcn: "gsy.requests.pub".to_string(),
        response_fqcn: "gsy.responses.sub".to_string(),
        topic_owner: "test.owner".to_string(),
        topic_version: "1.0.0".to_string(),
        consumer_client_id: "testclient".to_string(),
        timeout_ms: 1_000,
        poll_interval_ms: 10,
        empty_response_grace_ms: 0,
        topics: EwdsTopicConfig::default(),
        event_publish_fqcn: "gsy.events.pub".to_string(),
        event_subscribe_fqcn: "gsy.events.sub".to_string(),
        event_batch_size: 50,
        event_poll_interval_ms: 10,
        event_topics: EwdsEventTopicConfig::default(),
    })
}

fn delivered() -> ResponseTemplate {
    ResponseTemplate::new(200).set_body_json(json!({
        "recipients": {"sent": 1, "failed": 0, "total": 1}
    }))
}

#[tokio::test]
async fn publish_posts_the_message_on_the_given_channel_and_topic() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/v2/messages"))
        .respond_with(delivered())
        .expect(1)
        .mount(&server)
        .await;

    client(&server)
        .publish(
            "gsy.intelligent.events.pub",
            "facilitySubmitted",
            "event-1",
            r#"{"eventId":"event-1"}"#.to_string(),
        )
        .await
        .unwrap();

    let requests = server.received_requests().await.unwrap();
    let sent: EwdsSendMessageDto = serde_json::from_slice(&requests[0].body).unwrap();
    assert_eq!(sent.fqcn, "gsy.intelligent.events.pub");
    assert_eq!(sent.topic_name, "facilitySubmitted");
    assert_eq!(sent.topic_owner, "test.owner");
    assert_eq!(sent.topic_version, "1.0.0");
    assert_eq!(sent.transaction_id, "event-1");
    assert_eq!(sent.payload, r#"{"eventId":"event-1"}"#);
    assert!(sent.anonymous_recipient.is_empty());
}

#[tokio::test]
async fn publish_retries_when_rate_limited() {
    env::set_var("EWDS_RATE_LIMIT_BACKOFF_MS", "10");
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/v2/messages"))
        .respond_with(ResponseTemplate::new(429))
        .up_to_n_times(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/api/v2/messages"))
        .respond_with(delivered())
        .mount(&server)
        .await;

    client(&server)
        .publish(
            "gsy.intelligent.events.pub",
            "siteSubmitted",
            "event-2",
            "{}".to_string(),
        )
        .await
        .unwrap();

    assert_eq!(server.received_requests().await.unwrap().len(), 2);
}

#[tokio::test]
async fn publish_fails_when_the_gateway_rejects_the_message() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/v2/messages"))
        .respond_with(ResponseTemplate::new(400).set_body_string("unknown topic"))
        .expect(1)
        .mount(&server)
        .await;

    let error = client(&server)
        .publish(
            "gsy.intelligent.events.pub",
            "facilitySubmitted",
            "event-3",
            "{}".to_string(),
        )
        .await
        .unwrap_err()
        .to_string();

    assert!(error.contains("facilitySubmitted message"), "{error}");
    assert!(error.contains("HTTP 400"), "{error}");
    assert!(error.contains("unknown topic"), "{error}");
}

#[tokio::test]
async fn publish_event_uses_the_events_channel_and_the_topic_of_its_type() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/v2/messages"))
        .respond_with(delivered())
        .expect(1)
        .mount(&server)
        .await;
    let event = EwdsEventEnvelope {
        event_id: "event-4".to_string(),
        event_type: EwdsEventType::CommunitySubmitted,
        occurred_at: "2026-09-29T10:16:02+00:00".to_string(),
        data: json!({"communityId": "community-1"}),
    };

    client(&server).publish_event(&event).await.unwrap();

    let requests = server.received_requests().await.unwrap();
    let sent: EwdsSendMessageDto = serde_json::from_slice(&requests[0].body).unwrap();
    assert_eq!(sent.fqcn, "gsy.events.pub");
    assert_eq!(sent.topic_name, "communitySubmitted");
    assert_eq!(sent.transaction_id, "event-4");
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&sent.payload).unwrap(),
        serde_json::to_value(&event).unwrap()
    );
}

fn order_event(event_id: &str) -> EwdsEventEnvelope<Value> {
    EwdsEventEnvelope {
        event_id: event_id.to_string(),
        event_type: EwdsEventType::OrderSubmitted,
        occurred_at: "2026-09-30T09:40:13Z".to_string(),
        data: json!([]),
    }
}

/// Runs the order event worker against `server` until `expected` events were handled, then
/// gives it a few more polls and returns the IDs of all handled events. `handle` fails for
/// `failing_event_id`.
async fn run_order_event_worker(
    server: &MockServer,
    expected: usize,
    failing_event_id: &str,
) -> Vec<String> {
    let handled = Arc::new(Mutex::new(Vec::new()));
    let worker_client = client(server);
    let worker_handled = handled.clone();
    let failing_event_id = failing_event_id.to_string();
    let worker = tokio::spawn(async move {
        worker_client
            .run_event_worker(EwdsEventType::OrderSubmitted, |envelope| {
                let handled = worker_handled.clone();
                let failing_event_id = failing_event_id.clone();
                async move {
                    handled.lock().unwrap().push(envelope.event_id.clone());
                    if envelope.event_id == failing_event_id {
                        return Err(anyhow!("cannot handle {}", envelope.event_id));
                    }
                    Ok(())
                }
            })
            .await
    });

    for _ in 0..100 {
        if handled.lock().unwrap().len() >= expected {
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    // The gateway keeps returning the same messages, so later polls must not handle them again.
    tokio::time::sleep(Duration::from_millis(100)).await;
    worker.abort();

    let handled = handled.lock().unwrap().clone();
    handled
}

#[tokio::test]
async fn event_worker_polls_the_topic_of_its_type_on_the_events_channel() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v2/messages"))
        .and(query_param("fqcn", "gsy.events.sub"))
        .and(query_param("topicName", "orderSubmitted"))
        .and(query_param("topicOwner", "test.owner"))
        .and(query_param("clientId", "testclientorderSubmitted"))
        .and(query_param("amount", "50"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([
            {"payload": serde_json::to_string(&order_event("order-event")).unwrap()},
        ])))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/v2/messages"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([])))
        .with_priority(10)
        .mount(&server)
        .await;

    let handled = run_order_event_worker(&server, 1, "").await;

    assert_eq!(handled, vec!["order-event"]);
}

#[tokio::test]
async fn event_worker_skips_bad_and_seen_messages_and_does_not_retry_failed_events() {
    let server = MockServer::start().await;
    let wrong_type_event = EwdsEventEnvelope {
        event_type: EwdsEventType::SiteSubmitted,
        ..order_event("site-event")
    };
    Mock::given(method("GET"))
        .and(path("/api/v2/messages"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([
            {"payload": "not an event"},
            {"payload": serde_json::to_string(&wrong_type_event).unwrap()},
            {"payload": serde_json::to_string(&order_event("failing-event")).unwrap()},
            {"payload": serde_json::to_string(&order_event("order-event")).unwrap()},
            {"payload": serde_json::to_string(&order_event("order-event")).unwrap()},
        ])))
        .mount(&server)
        .await;

    let handled = run_order_event_worker(&server, 2, "failing-event").await;

    assert_eq!(handled, vec!["failing-event", "order-event"]);
}

#[tokio::test]
async fn event_worker_keeps_polling_after_a_failed_poll() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v2/messages"))
        .respond_with(ResponseTemplate::new(400).set_body_string("channel not found"))
        .up_to_n_times(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/v2/messages"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([
            {"payload": serde_json::to_string(&order_event("order-event")).unwrap()},
        ])))
        .mount(&server)
        .await;

    let handled = run_order_event_worker(&server, 1, "").await;

    assert_eq!(handled, vec!["order-event"]);
}
