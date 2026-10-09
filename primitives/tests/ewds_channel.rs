use primitives::ewds::channel::{
    route_event_by_type, route_request_by_operation, EwdsChannelPoller, EwdsChannelPollerConfig,
    EwdsDispatch,
};
use primitives::ewds::dto::{EwdsEventEnvelope, EwdsInboundMessage, EwdsRequestEnvelope};
use primitives::ewds::{EwdsEventTopicConfig, EwdsEventType, EwdsOperation, EwdsTopicConfig};
use serde_json::{json, Value};
use std::time::Duration;
use tokio::sync::mpsc::UnboundedReceiver;
use wiremock::matchers::{method, path, query_param, query_param_is_missing};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn config(gateway_base: &str) -> EwdsChannelPollerConfig {
    EwdsChannelPollerConfig {
        gateway_base: gateway_base.to_string(),
        fqcn: "gsy.events.sub".to_string(),
        client_id: "testclientevents".to_string(),
        topic_owner: "test.owner".to_string(),
        batch_size: 50,
        poll_interval_ms: 10,
    }
}

/// A message in the shape the gateway returns it.
fn gateway_message(id: &str, topic_name: &str, topic_owner: &str) -> Value {
    json!({
        "id": id,
        "topicName": topic_name,
        "topicOwner": topic_owner,
        "topicVersion": "1.0.0",
        "transactionId": id,
        "sender": "did:ethr:volta:0x0000000000000000000000000000000000000001",
        "payload": format!("{{\"id\":\"{}\"}}", id),
    })
}

fn message_without_topic(payload: String) -> EwdsInboundMessage {
    EwdsInboundMessage {
        payload,
        ..Default::default()
    }
}

async fn serve_once(server: &MockServer, messages: Value) {
    Mock::given(method("GET"))
        .and(path("/api/v2/messages"))
        .respond_with(ResponseTemplate::new(200).set_body_json(messages))
        .up_to_n_times(1)
        .mount(server)
        .await;
}

async fn serve_nothing_afterwards(server: &MockServer) {
    Mock::given(method("GET"))
        .and(path("/api/v2/messages"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([])))
        .with_priority(10)
        .mount(server)
        .await;
}

async fn receive_ids(
    receiver: &mut UnboundedReceiver<EwdsInboundMessage>,
    count: usize,
) -> Vec<String> {
    let mut ids = Vec::new();
    for _ in 0..count {
        let message = tokio::time::timeout(Duration::from_secs(2), receiver.recv())
            .await
            .expect("timed out waiting for a routed message")
            .expect("the route was closed");
        ids.push(message.id.unwrap());
    }
    ids
}

#[tokio::test]
async fn poll_reads_the_whole_channel_without_topic_or_owner() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v2/messages"))
        .and(query_param("fqcn", "gsy.events.sub"))
        .and(query_param("amount", "50"))
        .and(query_param("clientId", "testclientevents"))
        .and(query_param_is_missing("topicName"))
        .and(query_param_is_missing("topicOwner"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(json!([gateway_message(
                "m1",
                "order",
                "test.owner"
            ),])),
        )
        .mount(&server)
        .await;

    let messages = EwdsChannelPoller::new(config(&server.uri()))
        .poll_once()
        .await
        .unwrap();

    assert_eq!(messages.len(), 1);
    let message = &messages[0];
    assert_eq!(message.id.as_deref(), Some("m1"));
    assert_eq!(message.topic_name.as_deref(), Some("order"));
    assert_eq!(message.topic_owner.as_deref(), Some("test.owner"));
    assert_eq!(message.topic_version.as_deref(), Some("1.0.0"));
    assert_eq!(message.payload, "{\"id\":\"m1\"}");
}

#[tokio::test]
async fn poll_fails_on_a_body_that_is_no_message_list() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v2/messages"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"messages": []})))
        .mount(&server)
        .await;

    let result = EwdsChannelPoller::new(config(&server.uri()))
        .poll_once()
        .await;

    assert!(result.is_err());
}

#[tokio::test]
async fn poller_routes_each_topic_to_its_own_queue_in_order() {
    let server = MockServer::start().await;
    serve_once(
        &server,
        json!([
            gateway_message("order-1", "order", "test.owner"),
            gateway_message("site-1", "site", "test.owner"),
            gateway_message("trade-1", "trade", "test.owner"),
            gateway_message("order-foreign", "order", "other.owner"),
            gateway_message("order-2", "order", "test.owner"),
            gateway_message("site-2", "site", "test.owner"),
        ]),
    )
    .await;
    serve_nothing_afterwards(&server).await;

    let mut poller = EwdsChannelPoller::new(config(&server.uri()));
    let mut orders = poller.route("order");
    let mut sites = poller.route("site");
    let task = tokio::spawn(poller.run());

    assert_eq!(
        receive_ids(&mut orders, 2).await,
        vec!["order-1", "order-2"]
    );
    assert_eq!(receive_ids(&mut sites, 2).await, vec!["site-1", "site-2"]);
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert!(orders.try_recv().is_err());
    assert!(sites.try_recv().is_err());
    task.abort();
}

#[tokio::test]
async fn poller_fetches_the_next_batch_at_once_when_a_poll_returns_a_full_batch() {
    let server = MockServer::start().await;
    serve_once(
        &server,
        json!([
            gateway_message("order-1", "order", "test.owner"),
            gateway_message("order-2", "order", "test.owner"),
        ]),
    )
    .await;
    Mock::given(method("GET"))
        .and(path("/api/v2/messages"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(json!([gateway_message(
                "order-3",
                "order",
                "test.owner"
            ),])),
        )
        .up_to_n_times(1)
        .with_priority(5)
        .mount(&server)
        .await;
    serve_nothing_afterwards(&server).await;

    let mut poller = EwdsChannelPoller::new(EwdsChannelPollerConfig {
        batch_size: 2,
        poll_interval_ms: 60_000,
        ..config(&server.uri())
    });
    let mut orders = poller.route("order");
    let task = tokio::spawn(poller.run());

    // With a 60 s interval, order-3 only arrives in time if the full batch is followed at once.
    assert_eq!(
        receive_ids(&mut orders, 3).await,
        vec!["order-1", "order-2", "order-3"]
    );
    task.abort();
}

#[tokio::test]
async fn poller_keeps_polling_after_a_failed_poll() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v2/messages"))
        .respond_with(ResponseTemplate::new(400).set_body_string("bad request"))
        .up_to_n_times(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/v2/messages"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(json!([gateway_message(
                "order-1",
                "order",
                "test.owner"
            ),])),
        )
        .up_to_n_times(1)
        .with_priority(5)
        .mount(&server)
        .await;
    serve_nothing_afterwards(&server).await;

    let mut poller = EwdsChannelPoller::new(config(&server.uri()));
    let mut orders = poller.route("order");
    let task = tokio::spawn(poller.run());

    assert_eq!(receive_ids(&mut orders, 1).await, vec!["order-1"]);
    task.abort();
}

#[tokio::test]
async fn poller_stops_when_no_route_is_consumed_any_more() {
    let server = MockServer::start().await;
    serve_nothing_afterwards(&server).await;

    let mut poller = EwdsChannelPoller::new(config(&server.uri()));
    drop(poller.route("order"));
    drop(poller.route("site"));

    tokio::time::timeout(Duration::from_secs(2), poller.run())
        .await
        .expect("the poller kept running without consumers");
}

#[test]
fn dispatch_reports_what_happened_to_a_message() {
    let mut poller = EwdsChannelPoller::new(config("http://unused"));
    let _orders = poller.route("order");
    drop(poller.route("site"));
    let message = |topic: Option<&str>, owner: Option<&str>| EwdsInboundMessage {
        payload: "{}".to_string(),
        topic_name: topic.map(str::to_string),
        topic_owner: owner.map(str::to_string),
        ..Default::default()
    };

    assert_eq!(
        poller.dispatch(message(Some("order"), Some("test.owner"))),
        EwdsDispatch::Routed("order".to_string())
    );
    // A message without an owner is accepted; only a different owner is dropped.
    assert_eq!(
        poller.dispatch(message(Some("order"), None)),
        EwdsDispatch::Routed("order".to_string())
    );
    assert_eq!(
        poller.dispatch(message(Some("order"), Some("other.owner"))),
        EwdsDispatch::ForeignOwner("other.owner".to_string())
    );
    assert_eq!(
        poller.dispatch(message(Some("trade"), Some("test.owner"))),
        EwdsDispatch::Unrouted(Some("trade".to_string()))
    );
    assert_eq!(
        poller.dispatch(message(None, Some("test.owner"))),
        EwdsDispatch::Unrouted(None)
    );
    assert_eq!(
        poller.dispatch(message(Some("site"), Some("test.owner"))),
        EwdsDispatch::ReceiverClosed("site".to_string())
    );
}

#[test]
fn messages_without_a_topic_are_routed_by_their_event_type() {
    let mut poller = EwdsChannelPoller::new(config("http://unused"))
        .with_fallback_router(route_event_by_type(EwdsEventTopicConfig::default()));
    let mut facilities = poller.route("facility");
    let event = EwdsEventEnvelope {
        event_id: "event-1".to_string(),
        event_type: EwdsEventType::FacilitySubmitted,
        occurred_at: "2026-10-08T10:00:00Z".to_string(),
        data: json!([]),
    };

    let dispatched = poller.dispatch(message_without_topic(
        serde_json::to_string(&event).unwrap(),
    ));

    assert_eq!(dispatched, EwdsDispatch::Routed("facility".to_string()));
    assert!(facilities.try_recv().is_ok());
    assert_eq!(
        poller.dispatch(message_without_topic("not json".to_string())),
        EwdsDispatch::Unrouted(None)
    );
}

#[test]
fn messages_without_a_topic_are_routed_by_their_operation() {
    let mut poller = EwdsChannelPoller::new(config("http://unused"))
        .with_fallback_router(route_request_by_operation(EwdsTopicConfig::default()));
    let mut trades = poller.route("tradesQuery");
    let request = EwdsRequestEnvelope {
        request_id: "request-1".to_string(),
        operation: EwdsOperation::TradesQuery,
        payload: json!({}),
    };

    let dispatched = poller.dispatch(message_without_topic(
        serde_json::to_string(&request).unwrap(),
    ));

    assert_eq!(dispatched, EwdsDispatch::Routed("tradesQuery".to_string()));
    assert!(trades.try_recv().is_ok());
}

/// A response of the real gateway to a channel poll without `topicName`/`topicOwner`, trimmed:
/// the messages of several topics, with their metadata.
const RECORDED_CHANNEL_POLL: &str = include_str!("fixtures/ewds_channel_poll_response.json");

#[tokio::test]
async fn poller_routes_a_recorded_gateway_response() {
    let server = MockServer::start().await;
    serve_once(
        &server,
        serde_json::from_str(RECORDED_CHANNEL_POLL).unwrap(),
    )
    .await;
    serve_nothing_afterwards(&server).await;

    let mut poller = EwdsChannelPoller::new(EwdsChannelPollerConfig {
        fqcn: "gsy.intelligent.responses.sub".to_string(),
        topic_owner: "integration.apps.intelligent.auth.ewc".to_string(),
        ..config(&server.uri())
    });
    let mut ids = poller.route("idsQueryTestResponse");
    let mut trades = poller.route("tradesQueryTestResponse");
    let task = tokio::spawn(poller.run());

    let id_response = ids.recv().await.unwrap();
    let trade_response = trades.recv().await.unwrap();
    task.abort();

    assert_eq!(id_response.topic_version.as_deref(), Some("1.0.0"));
    assert!(id_response.timestamp_nanos.is_some());
    assert!(id_response
        .transaction_id
        .unwrap()
        .starts_with("ids-query-"));
    assert!(id_response.sender.unwrap().starts_with("did:ethr:"));
    let payload: Value = serde_json::from_str(&trade_response.payload).unwrap();
    assert!(payload["requestId"]
        .as_str()
        .unwrap()
        .starts_with("trades-query-"));
}

#[test]
fn a_message_knows_its_age() {
    let published = std::time::UNIX_EPOCH + Duration::from_secs(1_791_380_319);
    let message = EwdsInboundMessage {
        timestamp_nanos: Some(1_791_380_319_000_000_000),
        ..Default::default()
    };

    assert_eq!(
        message.age_at(published + Duration::from_secs(90)),
        Some(Duration::from_secs(90))
    );
    // Clock skew: a message from the future is not negative in age.
    assert_eq!(
        message.age_at(published - Duration::from_secs(5)),
        Some(Duration::ZERO)
    );
    assert_eq!(EwdsInboundMessage::default().age_at(published), None);
}
