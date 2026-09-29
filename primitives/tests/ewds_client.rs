use primitives::ewds::dto::{EwdsEventEnvelope, EwdsSendMessageDto};
use primitives::ewds::{
    EwdsClient, EwdsClientConfig, EwdsEventTopicConfig, EwdsEventType, EwdsTopicConfig,
};
use serde_json::json;
use std::env;
use wiremock::matchers::{method, path};
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
