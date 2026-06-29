use crate::db::DatabaseWrapper;
use anyhow::{anyhow, Result};
use gsy_offchain_primitives::db_api_schema::orders::{
    DbOrderSchema, EnergyType, OrderEnum, OrderStatus,
};
use gsy_offchain_primitives::db_api_schema::profiles::{MeasurementPointType, MeasurementSchema};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{HashMap, HashSet, VecDeque};
use tokio::time::{sleep, Duration};
use tracing::{error, info, warn};

#[derive(Clone)]
pub struct EwdsHandlerConfig {
    pub enabled: bool,
    pub gateway_url: String,
    pub request_fqcn: String,
    pub response_fqcn: String,
    pub topic_owner: String,
    pub topic_version: String,
    pub request_client_id: String,
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

        let request_fqcn = env_var("EWDS_REQUEST_SUBSCRIBE_FQCN")
            .or_else(|| env_var("EWDS_REQUEST_FQCN"))
            .unwrap_or_else(|| "gsy.intelligent.requests.sub".to_string());
        let response_fqcn = env_var("EWDS_RESPONSE_PUBLISH_FQCN")
            .or_else(|| env_var("EWDS_RESPONSE_FQCN"))
            .unwrap_or_else(|| "gsy.intelligent.responses.pub".to_string());

        Self {
            enabled,
            gateway_url: std::env::var("EWDS_GATEWAY_URL")
                .unwrap_or_else(|_| "http://ewds-gateway-api:3333".to_string()),
            request_fqcn,
            response_fqcn,
            topic_owner: std::env::var("EWDS_TOPIC_OWNER")
                .unwrap_or_else(|_| "integration.apps.intelligent.auth.ewc".to_string()),
            topic_version: std::env::var("EWDS_TOPIC_VERSION")
                .unwrap_or_else(|_| "1.0.0".to_string()),
            request_client_id: env_var("EWDS_REQUEST_CLIENT_ID")
                .or_else(|| env_var("EWDS_OFFCHAIN_STORAGE_CLIENT_ID"))
                .unwrap_or_else(|| "gsyoffchainstorage".to_string()),
            orders_request_topic: std::env::var("EWDS_ORDERS_REQUEST_TOPIC")
                .unwrap_or_else(|_| "ordersQuery".to_string()),
            trades_request_topic: std::env::var("EWDS_TRADES_REQUEST_TOPIC")
                .unwrap_or_else(|_| "tradesQuery".to_string()),
            measurements_request_topic: std::env::var("EWDS_MEASUREMENTS_REQUEST_TOPIC")
                .unwrap_or_else(|_| "measurementsQuery".to_string()),
            orders_response_topic: std::env::var("EWDS_ORDERS_RESPONSE_TOPIC")
                .unwrap_or_else(|_| "ordersQueryResponse".to_string()),
            trades_response_topic: std::env::var("EWDS_TRADES_RESPONSE_TOPIC")
                .unwrap_or_else(|_| "tradesQueryResponse".to_string()),
            measurements_response_topic: std::env::var("EWDS_MEASUREMENTS_RESPONSE_TOPIC")
                .unwrap_or_else(|_| "measurementsQueryResponse".to_string()),
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
#[serde(rename_all = "camelCase")]
struct EwdsRequestEnvelope {
    #[serde(alias = "request_id")]
    request_id: String,
    operation: String,
    payload: Value,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct EwdsOrderDto {
    order_id: String,
    market_id: String,
    order_type: String,
    status: String,
    area_uuid: String,
    nonce: Option<u64>,
    time_slot: u64,
    creation_time: u64,
    quantity: f64,
    price_limit: f64,
    created_by: String,
    requirements: Option<EwdsRequirementsDto>,
    attributes: Option<EwdsAttributesDto>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct EwdsRequirementsDto {
    trading_partner_id: Option<String>,
    energy_type: Option<String>,
    preferred_energy_rate: Option<f64>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct EwdsAttributesDto {
    trading_partner_id: Option<String>,
    energy_type: String,
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
#[serde(rename_all = "camelCase")]
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
    start_time: Option<u64>,
    #[serde(alias = "endTime")]
    #[serde(default)]
    end_time: Option<u64>,
}

#[derive(Deserialize)]
struct TimeRangePayload {
    #[serde(alias = "startTime")]
    #[serde(default)]
    start_time: Option<u64>,
    #[serde(alias = "endTime")]
    #[serde(default)]
    end_time: Option<u64>,
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
    let client_id = client_id_for_suffix(config.request_client_id.as_str(), topic_name);
    let response = client
        .get(get_url.as_str())
        .query(&[
            ("fqcn", config.request_fqcn.as_str()),
            ("amount", amount),
            ("topicName", topic_name),
            ("topicOwner", config.topic_owner.as_str()),
            ("clientId", client_id.as_str()),
        ])
        .send()
        .await?;

    let status = response.status();
    if !status.is_success() {
        let body = response.text().await.unwrap_or_default();
        return Err(anyhow!(
            "EWDS request poll failed for topic '{}': HTTP {}{}",
            topic_name,
            status,
            format_response_body(&body)
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
            let request_id = envelope.request_id;

            info!(
                "Handling EWDS orders.query request (request_id={})",
                request_id
            );

            let data = db
                .orders()
                .filter_orders(payload.market_id, payload.start_time, payload.end_time)
                .await?
                .into_iter()
                .map(EwdsOrderDto::from)
                .collect::<Vec<_>>();
            info!(
                "Publishing EWDS orders.query response (request_id={}, orders={})",
                request_id,
                data.len()
            );

            send_success_response(
                client,
                config,
                request_id,
                config.orders_response_topic.as_str(),
                data,
            )
            .await
        }
        "trades.query" => {
            let payload = serde_json::from_value::<TimeRangePayload>(envelope.payload.clone())
                .map_err(|e| anyhow!("trades.query payload parse error: {}", e))?;
            let request_id = envelope.request_id;

            info!(
                "Handling EWDS trades.query request (request_id={})",
                request_id
            );

            let data = db
                .trades()
                .filter_trades(payload.start_time, payload.end_time)
                .await?;
            info!(
                "Publishing EWDS trades.query response (request_id={}, orders={})",
                request_id,
                data.len()
            );

            send_success_response(
                client,
                config,
                request_id,
                config.trades_response_topic.as_str(),
                data,
            )
            .await
        }
        "measurements.query" => {
            let payload = serde_json::from_value::<TimeRangePayload>(envelope.payload.clone())
                .map_err(|e| anyhow!("measurements.query payload parse error: {}", e))?;
            let request_id = envelope.request_id;

            info!(
                "Handling EWDS measurements.query request (request_id={})",
                request_id
            );

            let data = fetch_measurements_from_timeseries(db, payload.start_time, payload.end_time)
                .await?
                .into_iter()
                .filter(|measurement| match payload.area_uuid.as_ref() {
                    Some(area_uuid) => measurement.area_uuid == *area_uuid,
                    None => true,
                })
                .collect::<Vec<_>>();
            info!(
                "Publishing EWDS measurements.query response (request_id={}, orders={})",
                request_id,
                data.len()
            );

            send_success_response(
                client,
                config,
                request_id,
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

async fn fetch_measurements_from_timeseries(
    db: &DatabaseWrapper,
    start_time: Option<u64>,
    end_time: Option<u64>,
) -> Result<Vec<MeasurementSchema>> {
    let points = db
        .measurement_points()
        .filter_points(None, Some(MeasurementPointType::Measurement))
        .await?;
    let points_by_id = points
        .into_iter()
        .map(|point| (point.measurement_id.clone(), point))
        .collect::<HashMap<_, _>>();

    let values = db
        .timeseries()
        .filter_values(
            None,
            start_time.map(format_timeseries_timestamp),
            end_time.map(format_timeseries_timestamp),
        )
        .await?;

    Ok(values
        .into_iter()
        .filter_map(|value| {
            let point = points_by_id.get(&value.measurement_point)?;
            let time_slot = parse_timeseries_timestamp(value.timestamp.as_str())?;
            Some(MeasurementSchema {
                area_uuid: point.asset_name.clone(),
                community_uuid: point.datasource_name.clone().unwrap_or_default(),
                time_slot,
                creation_time: time_slot,
                energy_kwh: value.value,
            })
        })
        .collect())
}

fn format_timeseries_timestamp(timestamp: u64) -> String {
    format!("{:020}", timestamp)
}

fn parse_timeseries_timestamp(timestamp: &str) -> Option<u64> {
    timestamp.parse::<u64>().ok()
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

impl From<DbOrderSchema> for EwdsOrderDto {
    fn from(order: DbOrderSchema) -> Self {
        Self {
            order_id: order.order_id,
            market_id: order.market_id,
            order_type: order_type_to_ewds(&order.order_type).to_string(),
            status: order_status_to_ewds(&order.status).to_string(),
            area_uuid: order.area_uuid,
            nonce: order.nonce,
            time_slot: order.time_slot,
            creation_time: order.creation_time,
            quantity: order.energy_kWh,
            price_limit: order.energy_rate,
            created_by: order.created_by,
            requirements: order.requirements.map(|requirements| EwdsRequirementsDto {
                trading_partner_id: requirements.trading_partner_id,
                energy_type: requirements
                    .energy_type
                    .map(|value| energy_type_to_ewds(&value).to_string()),
                preferred_energy_rate: requirements.preferred_energy_rate,
            }),
            attributes: order.attributes.map(|attributes| EwdsAttributesDto {
                trading_partner_id: attributes.trading_partner_id,
                energy_type: energy_type_to_ewds(&attributes.energy_type).to_string(),
            }),
        }
    }
}

fn order_type_to_ewds(order_type: &OrderEnum) -> &'static str {
    match order_type {
        OrderEnum::Bid => "bid",
        OrderEnum::Offer => "offer",
    }
}

fn order_status_to_ewds(status: &OrderStatus) -> &'static str {
    match status {
        OrderStatus::Open => "open",
        OrderStatus::Executed => "executed",
        OrderStatus::Expired => "expired",
        OrderStatus::Deleted => "deleted",
    }
}

fn energy_type_to_ewds(energy_type: &EnergyType) -> &'static str {
    match energy_type {
        EnergyType::Clean => "clean",
        EnergyType::Battery => "battery",
        EnergyType::FossilFuel => "fossilFuel",
        EnergyType::Import => "import",
    }
}

async fn send_message(
    client: &Client,
    config: &EwdsHandlerConfig,
    request_id: String,
    topic_name: String,
    payload: String,
) -> Result<()> {
    send_message_with_fqcn(
        client,
        config,
        config.response_fqcn.clone(),
        request_id,
        topic_name,
        payload,
    )
    .await
}

async fn send_message_with_fqcn(
    client: &Client,
    config: &EwdsHandlerConfig,
    fqcn: String,
    transaction_id: String,
    topic_name: String,
    payload: String,
) -> Result<()> {
    let post_url = format!(
        "{}/api/v2/messages",
        config.gateway_url.trim_end_matches('/')
    );
    let request_body = EwdsSendMessageDto {
        fqcn,
        topic_name,
        topic_version: config.topic_version.clone(),
        topic_owner: config.topic_owner.clone(),
        transaction_id,
        payload,
        anonymous_recipient: Vec::new(),
    };

    let response = client.post(post_url).json(&request_body).send().await?;
    let status = response.status();
    if !status.is_success() {
        let error_body = response.text().await.unwrap_or_default();
        return Err(anyhow!(
            "EWDS message send failed for fqcn='{}', topic='{}': HTTP {}{}",
            request_body.fqcn,
            request_body.topic_name,
            status,
            format_response_body(&error_body)
        ));
    }

    Ok(())
}

fn format_response_body(body: &str) -> String {
    let compact = body.split_whitespace().collect::<Vec<_>>().join(" ");
    if compact.is_empty() {
        return String::new();
    }

    let max_chars = 1_024;
    let truncated = compact.chars().take(max_chars).collect::<String>();
    if compact.chars().count() > max_chars {
        format!(": {}...", truncated)
    } else {
        format!(": {}", truncated)
    }
}

fn env_var(key: &str) -> Option<String> {
    std::env::var(key)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn client_id_for_suffix(base: &str, suffix: &str) -> String {
    let mut value = String::with_capacity(base.len() + suffix.len());
    value.extend(base.chars().filter(|ch| ch.is_ascii_alphanumeric()));
    value.extend(suffix.chars().filter(|ch| ch.is_ascii_alphanumeric()));

    if value.is_empty() {
        "gsyoffchainstorage".to_string()
    } else {
        value
    }
}
