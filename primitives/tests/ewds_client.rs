use anyhow::anyhow;
use primitives::ewds::dto::{EwdsEventEnvelope, EwdsSendMessageDto};
use primitives::ewds::{
    invalid_event, EwdsClient, EwdsClientConfig, EwdsEventTopicConfig, EwdsEventType,
    EwdsTopicConfig,
};
use serde_json::{json, Value};
use std::env;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use wiremock::matchers::{method, path, query_param, query_param_is_missing};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn client(server: &MockServer) -> EwdsClient {
    EwdsClient::new(client_config(server))
}

fn client_config(server: &MockServer) -> EwdsClientConfig {
    EwdsClientConfig {
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
        event_handle_attempts: 3,
        event_retry_delay_ms: 10,
        event_topics: EwdsEventTopicConfig::default(),
    }
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
            "facility",
            "event-1",
            r#"{"eventId":"event-1"}"#.to_string(),
        )
        .await
        .unwrap();

    let requests = server.received_requests().await.unwrap();
    let sent: EwdsSendMessageDto = serde_json::from_slice(&requests[0].body).unwrap();
    assert_eq!(sent.fqcn, "gsy.intelligent.events.pub");
    assert_eq!(sent.topic_name, "facility");
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
            "site",
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
            "facility",
            "event-3",
            "{}".to_string(),
        )
        .await
        .unwrap_err()
        .to_string();

    assert!(error.contains("facility message"), "{error}");
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
    assert_eq!(sent.topic_name, "community");
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
/// gives it a few more polls and returns the IDs of all handled events, one per attempt.
/// `outcome` gets the event ID and how often the event was handled before and decides the
/// handler's result.
async fn run_order_event_worker(
    server: &MockServer,
    expected: usize,
    outcome: fn(&str, usize) -> anyhow::Result<()>,
) -> Vec<String> {
    let handled = Arc::new(Mutex::new(Vec::<String>::new()));
    let worker_client = client(server);
    let worker_handled = handled.clone();
    let worker = tokio::spawn(async move {
        worker_client
            .run_event_subscriber(&[EwdsEventType::OrderSubmitted], |envelope| {
                let handled = worker_handled.clone();
                async move {
                    let mut handled = handled.lock().unwrap();
                    let earlier_attempts = handled
                        .iter()
                        .filter(|id| **id == envelope.event_id)
                        .count();
                    handled.push(envelope.event_id.clone());
                    outcome(envelope.event_id.as_str(), earlier_attempts)
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

fn succeeds(_: &str, _: usize) -> anyhow::Result<()> {
    Ok(())
}

/// Serves `events` as order events on every poll.
async fn serve_order_events(server: &MockServer, events: &[&str]) {
    let messages = events
        .iter()
        .map(|event_id| json!({"payload": serde_json::to_string(&order_event(event_id)).unwrap()}))
        .collect::<Vec<_>>();
    Mock::given(method("GET"))
        .and(path("/api/v2/messages"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!(messages)))
        .mount(server)
        .await;
}

#[tokio::test]
async fn event_subscriber_polls_the_whole_events_channel() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v2/messages"))
        .and(query_param("fqcn", "gsy.events.sub"))
        .and(query_param_is_missing("topicName"))
        .and(query_param_is_missing("topicOwner"))
        .and(query_param("clientId", "testclientevents"))
        .and(query_param("amount", "50"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([
            {
                "topicName": "order",
                "topicOwner": "test.owner",
                "payload": serde_json::to_string(&order_event("order-event")).unwrap(),
            },
            {
                "topicName": "trade",
                "topicOwner": "test.owner",
                "payload": serde_json::to_string(&EwdsEventEnvelope {
                    event_type: EwdsEventType::TradeCreated,
                    ..order_event("trade-event")
                }).unwrap(),
            },
            {
                "topicName": "order",
                "topicOwner": "other.owner",
                "payload": serde_json::to_string(&order_event("foreign-event")).unwrap(),
            },
        ])))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/v2/messages"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([])))
        .with_priority(10)
        .mount(&server)
        .await;

    let handled = run_order_event_worker(&server, 1, succeeds).await;

    assert_eq!(handled, vec!["order-event"]);
}

#[tokio::test]
async fn event_subscriber_skips_bad_and_seen_messages() {
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
            {"payload": serde_json::to_string(&order_event("order-event")).unwrap()},
            {"payload": serde_json::to_string(&order_event("order-event")).unwrap()},
        ])))
        .mount(&server)
        .await;

    let handled = run_order_event_worker(&server, 1, succeeds).await;

    assert_eq!(handled, vec!["order-event"]);
}

#[tokio::test]
async fn event_subscriber_retries_a_failed_event_and_keeps_later_events_waiting() {
    let server = MockServer::start().await;
    serve_order_events(&server, &["flaky-event", "order-event"]).await;

    let handled = run_order_event_worker(&server, 3, |event_id, earlier_attempts| {
        if event_id == "flaky-event" && earlier_attempts < 1 {
            return Err(anyhow!("database unavailable"));
        }
        Ok(())
    })
    .await;

    assert_eq!(handled, vec!["flaky-event", "flaky-event", "order-event"]);
}

#[tokio::test]
async fn event_subscriber_drops_an_event_after_its_last_attempt() {
    let server = MockServer::start().await;
    serve_order_events(&server, &["failing-event", "order-event"]).await;

    let handled = run_order_event_worker(&server, 4, |event_id, _| {
        if event_id == "failing-event" {
            return Err(anyhow!("database unavailable"));
        }
        Ok(())
    })
    .await;

    // The client allows 3 attempts; the dropped event is not handled again on later polls.
    assert_eq!(
        handled,
        vec![
            "failing-event",
            "failing-event",
            "failing-event",
            "order-event"
        ]
    );
}

#[tokio::test]
async fn event_subscriber_drops_an_invalid_event_without_retrying_it() {
    let server = MockServer::start().await;
    serve_order_events(&server, &["invalid-event", "order-event"]).await;

    let handled = run_order_event_worker(&server, 2, |event_id, _| {
        if event_id == "invalid-event" {
            return Err(invalid_event(anyhow!("quantity must be greater than 0")));
        }
        Ok(())
    })
    .await;

    assert_eq!(handled, vec!["invalid-event", "order-event"]);
}

#[tokio::test]
async fn event_subscriber_keeps_polling_after_a_failed_poll() {
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

    let handled = run_order_event_worker(&server, 1, succeeds).await;

    assert_eq!(handled, vec!["order-event"]);
}

#[tokio::test]
async fn event_subscriber_keeps_other_topics_going_while_one_waits_for_a_retry() {
    let server = MockServer::start().await;
    let site_event = EwdsEventEnvelope {
        event_type: EwdsEventType::SiteSubmitted,
        ..order_event("site-event")
    };
    Mock::given(method("GET"))
        .and(path("/api/v2/messages"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([
            {"topicName": "order", "payload": serde_json::to_string(&order_event("flaky-order")).unwrap()},
            {"topicName": "site", "payload": serde_json::to_string(&site_event).unwrap()},
        ])))
        .up_to_n_times(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/v2/messages"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([])))
        .with_priority(10)
        .mount(&server)
        .await;

    let handled = Arc::new(Mutex::new(Vec::<String>::new()));
    let subscriber_client = EwdsClient::new(EwdsClientConfig {
        // Long enough that the site event can only be handled before the order event's retry
        // if it does not wait behind it.
        event_retry_delay_ms: 2_000,
        ..client_config(&server)
    });
    let subscriber_handled = handled.clone();
    let subscriber = tokio::spawn(async move {
        subscriber_client
            .run_event_subscriber(
                &[EwdsEventType::OrderSubmitted, EwdsEventType::SiteSubmitted],
                |envelope| {
                    let handled = subscriber_handled.clone();
                    async move {
                        handled.lock().unwrap().push(envelope.event_id.clone());
                        if envelope.event_id == "flaky-order" {
                            return Err(anyhow!("database unavailable"));
                        }
                        Ok(())
                    }
                },
            )
            .await
    });

    for _ in 0..50 {
        if handled.lock().unwrap().len() >= 2 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    subscriber.abort();

    let mut handled = handled.lock().unwrap().clone();
    handled.sort();
    assert_eq!(handled, vec!["flaky-order", "site-event"]);
}
