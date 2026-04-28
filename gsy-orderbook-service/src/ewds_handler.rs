use crate::db::DatabaseWrapper;
use anyhow::{anyhow, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{HashSet, VecDeque};
use tokio::time::{sleep, Duration};
use tracing::{error, info, warn};

#[derive(Clone)]
pub struct EwdsHandlerConfig {
    pub enabled: bool,
    pub gateway_url: String,
    pub request_fqcn: String,
    pub response_fqcn: String,
    pub topic_owner: String,
    pub orders_request_topic: String,
    pub trades_request_topic: String,
    pub measurements_request_topic: String,
    pub orders_response_topic: String,
    pub trades_response_topic: String,
    pub measurements_response_topic: String,
    pub poll_interval_ms: u64,
    pub request_batch_size: u32,
}

impl EwdsHandlerConfig {
    pub fn from_env() -> Self {
        let enabled = std::env::var("EWDS_ENABLE_HANDLER")
            .map(|value| {
                let normalized = value.to_ascii_lowercase();
                normalized == "1" || normalized == "true" || normalized == "yes"
            })
            .unwrap_or(false);

        let poll_interval_ms = std::env::var("EWDS_HANDLER_POLL_INTERVAL_MS")
            .ok()
            .and_then(|value| value.parse::<u64>().ok())
            .unwrap_or(500);

        let request_batch_size = std::env::var("EWDS_HANDLER_BATCH_SIZE")
            .ok()
            .and_then(|value| value.parse::<u32>().ok())
            .unwrap_or(100);

        Self {
            enabled,
            gateway_url: std::env::var("EWDS_GATEWAY_URL")
                .unwrap_or_else(|_| "http://ewds-gateway-api:3333".to_string()),
            request_fqcn: std::env::var("EWDS_REQUEST_FQCN")
                .unwrap_or_else(|_| "gsy.dex.offchain.request".to_string()),
            response_fqcn: std::env::var("EWDS_RESPONSE_FQCN")
                .unwrap_or_else(|_| "gsy.dex.offchain.response".to_string()),
            topic_owner: std::env::var("EWDS_TOPIC_OWNER")
                .unwrap_or_else(|_| "gsy.dex.offchain-storage".to_string()),
            orders_request_topic: std::env::var("EWDS_ORDERS_REQUEST_TOPIC")
                .unwrap_or_else(|_| "orders.query".to_string()),
            trades_request_topic: std::env::var("EWDS_TRADES_REQUEST_TOPIC")
                .unwrap_or_else(|_| "trades.query".to_string()),
            measurements_request_topic: std::env::var("EWDS_MEASUREMENTS_REQUEST_TOPIC")
                .unwrap_or_else(|_| "measurements.query".to_string()),
            orders_response_topic: std::env::var("EWDS_ORDERS_RESPONSE_TOPIC")
                .unwrap_or_else(|_| "orders.query.response".to_string()),
            trades_response_topic: std::env::var("EWDS_TRADES_RESPONSE_TOPIC")
                .unwrap_or_else(|_| "trades.query.response".to_string()),
            measurements_response_topic: std::env::var("EWDS_MEASUREMENTS_RESPONSE_TOPIC")
                .unwrap_or_else(|_| "measurements.query.response".to_string()),
            poll_interval_ms,
            request_batch_size,
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct EwdsInboundMessage {
    payload: String,
}

#[derive(Deserialize)]
struct EwdsRequestEnvelope {
    request_id: String,
    operation: String,
    payload: Value,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct EwdsSendMessageDto {
    fqcn: String,
    topic_name: String,
    topic_version: String,
    topic_owner: String,
    transaction_id: String,
    payload: String,
    anonymous_recipient: Vec<String>,
}

#[derive(Serialize)]
struct EwdsResponseEnvelope<T> {
    request_id: String,
    success: bool,
    data: Vec<T>,
    error: Option<EwdsErrorPayload>,
}

#[derive(Serialize)]
struct EwdsErrorPayload {
    code: String,
    message: String,
}

#[derive(Deserialize)]
struct OrdersQueryPayload {
    #[serde(alias = "marketId")]
    #[serde(default)]
    market_id: Option<String>,
    #[serde(alias = "startTime")]
    #[serde(default)]
    start_time: Option<u32>,
    #[serde(alias = "endTime")]
    #[serde(default)]
    end_time: Option<u32>,
}

#[derive(Deserialize)]
struct TimeRangePayload {
    #[serde(alias = "startTime")]
    #[serde(default)]
    start_time: Option<u32>,
    #[serde(alias = "endTime")]
    #[serde(default)]
    end_time: Option<u32>,
    #[serde(alias = "areaUuid")]
    #[serde(default)]
    area_uuid: Option<String>,
}

pub async fn start_ewds_request_handler(db: DatabaseWrapper, config: EwdsHandlerConfig) {
    if !config.enabled {
        info!("EWDS request handler disabled");
        return;
    }

    info!(
        "Starting EWDS request handler (gateway={}, request_fqcn={}, response_fqcn={})",
        config.gateway_url, config.request_fqcn, config.response_fqcn
    );

    let client = Client::new();
    let mut seen_request_ids: HashSet<String> = HashSet::new();
    let mut seen_queue: VecDeque<String> = VecDeque::new();

    loop {
        if let Err(error) = process_batch(
            &db,
            &client,
            &config,
            &mut seen_request_ids,
            &mut seen_queue,
        )
        .await
        {
            warn!("EWDS batch processing failed: {}", error);
        }

        sleep(Duration::from_millis(config.poll_interval_ms)).await;
    }
}

async fn process_batch(
    db: &DatabaseWrapper,
    client: &Client,
    config: &EwdsHandlerConfig,
    seen_request_ids: &mut HashSet<String>,
    seen_queue: &mut VecDeque<String>,
) -> Result<()> {
    let amount = config.request_batch_size.to_string();
    let mut messages = Vec::new();
    for topic_name in [
        config.orders_request_topic.as_str(),
        config.trades_request_topic.as_str(),
        config.measurements_request_topic.as_str(),
    ] {
        messages
            .extend(poll_requests_for_topic(client, config, topic_name, amount.as_str()).await?);
    }

    for message in messages {
        let parsed = serde_json::from_str::<EwdsRequestEnvelope>(&message.payload);
        let envelope = match parsed {
            Ok(value) => value,
            Err(_) => continue,
        };

        if seen_request_ids.contains(&envelope.request_id) {
            continue;
        }

        remember_request_id(&envelope.request_id, seen_request_ids, seen_queue);

        if let Err(error) = handle_request(db, client, config, envelope).await {
            error!("EWDS request handling failed: {}", error);
        }
    }

    Ok(())
}

async fn poll_requests_for_topic(
    client: &Client,
    config: &EwdsHandlerConfig,
    topic_name: &str,
    amount: &str,
) -> Result<Vec<EwdsInboundMessage>> {
    let get_url = format!(
        "{}/api/v2/messages",
        config.gateway_url.trim_end_matches('/')
    );
    let response = client
        .get(get_url.as_str())
        .query(&[
            ("fqcn", config.request_fqcn.as_str()),
            ("amount", amount),
            ("topicName", topic_name),
            ("topicOwner", config.topic_owner.as_str()),
        ])
        .send()
        .await?;

    if !response.status().is_success() {
        return Err(anyhow!(
            "EWDS request poll failed for topic '{}': HTTP {}",
            topic_name,
            response.status()
        ));
    }

    Ok(response
        .json::<Vec<EwdsInboundMessage>>()
        .await
        .unwrap_or_default())
}

fn remember_request_id(
    request_id: &str,
    seen_request_ids: &mut HashSet<String>,
    seen_queue: &mut VecDeque<String>,
) {
    const MAX_SEEN_REQUEST_IDS: usize = 2_048;

    seen_request_ids.insert(request_id.to_string());
    seen_queue.push_back(request_id.to_string());

    while seen_queue.len() > MAX_SEEN_REQUEST_IDS {
        if let Some(evicted) = seen_queue.pop_front() {
            seen_request_ids.remove(&evicted);
        }
    }
}

async fn handle_request(
    db: &DatabaseWrapper,
    client: &Client,
    config: &EwdsHandlerConfig,
    envelope: EwdsRequestEnvelope,
) -> Result<()> {
    match envelope.operation.as_str() {
        "orders.query" => {
            let payload = serde_json::from_value::<OrdersQueryPayload>(envelope.payload.clone())
                .map_err(|e| anyhow!("orders.query payload parse error: {}", e))?;

            let data = db
                .orders()
                .filter_orders(payload.market_id, payload.start_time, payload.end_time)
                .await?;

            send_success_response(
                client,
                config,
                envelope.request_id,
                config.orders_response_topic.as_str(),
                data,
            )
            .await
        }
        "trades.query" => {
            let payload = serde_json::from_value::<TimeRangePayload>(envelope.payload.clone())
                .map_err(|e| anyhow!("trades.query payload parse error: {}", e))?;

            let data = db
                .trades()
                .filter_trades(payload.start_time, payload.end_time)
                .await?;

            send_success_response(
                client,
                config,
                envelope.request_id,
                config.trades_response_topic.as_str(),
                data,
            )
            .await
        }
        "measurements.query" => {
            let payload = serde_json::from_value::<TimeRangePayload>(envelope.payload.clone())
                .map_err(|e| anyhow!("measurements.query payload parse error: {}", e))?;

            let data = db
                .measurements()
                .filter_measurements(payload.area_uuid, payload.start_time, payload.end_time)
                .await?;

            send_success_response(
                client,
                config,
                envelope.request_id,
                config.measurements_response_topic.as_str(),
                data,
            )
            .await
        }
        unsupported => {
            send_error_response(
                client,
                config,
                envelope.request_id,
                format!("{}.response", unsupported),
                "unsupported_operation",
                format!("Operation '{}' is not supported", unsupported),
            )
            .await
        }
    }
}

async fn send_success_response<T: Serialize>(
    client: &Client,
    config: &EwdsHandlerConfig,
    request_id: String,
    topic_name: &str,
    data: Vec<T>,
) -> Result<()> {
    let payload = EwdsResponseEnvelope {
        request_id: request_id.clone(),
        success: true,
        data,
        error: None,
    };

    send_message(
        client,
        config,
        request_id,
        topic_name.to_string(),
        serde_json::to_string(&payload)?,
    )
    .await
}

async fn send_error_response(
    client: &Client,
    config: &EwdsHandlerConfig,
    request_id: String,
    topic_name: String,
    code: &str,
    message: String,
) -> Result<()> {
    let payload: EwdsResponseEnvelope<Value> = EwdsResponseEnvelope {
        request_id: request_id.clone(),
        success: false,
        data: Vec::new(),
        error: Some(EwdsErrorPayload {
            code: code.to_string(),
            message,
        }),
    };

    send_message(
        client,
        config,
        request_id,
        topic_name,
        serde_json::to_string(&payload)?,
    )
    .await
}

async fn send_message(
    client: &Client,
    config: &EwdsHandlerConfig,
    request_id: String,
    topic_name: String,
    payload: String,
) -> Result<()> {
    let post_url = format!(
        "{}/api/v2/messages",
        config.gateway_url.trim_end_matches('/')
    );
    let body = EwdsSendMessageDto {
        fqcn: config.response_fqcn.clone(),
        topic_name,
        topic_version: "1.0.0".to_string(),
        topic_owner: config.topic_owner.clone(),
        transaction_id: request_id,
        payload,
        anonymous_recipient: Vec::new(),
    };

    let response = client.post(post_url).json(&body).send().await?;
    if !response.status().is_success() {
        return Err(anyhow!(
            "EWDS response send failed: HTTP {}",
            response.status()
        ));
    }

    Ok(())
}
