use crate::db_api_schema::orders::{DbOrderSchema, EnergyType, OrderEnum, OrderStatus};
use crate::db_api_schema::ids::IdMappingSchema;
use anyhow::{anyhow, Result};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::Value;
use std::{env, fmt, time::Instant};
use tokio::time::{sleep, Duration};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EwdsOperation {
    #[serde(rename = "orders.query")]
    OrdersQuery,
    #[serde(rename = "trades.query")]
    TradesQuery,
    #[serde(rename = "measurements.query")]
    MeasurementsQuery,
    #[serde(rename = "ids.query")]
    IdsQuery,
}

impl EwdsOperation {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::OrdersQuery => "orders.query",
            Self::TradesQuery => "trades.query",
            Self::MeasurementsQuery => "measurements.query",
            Self::IdsQuery => "ids.query",
        }
    }

    pub fn request_id_prefix(self) -> &'static str {
        match self {
            Self::OrdersQuery => "orders-query",
            Self::TradesQuery => "trades-query",
            Self::MeasurementsQuery => "measurements-query",
            Self::IdsQuery => "ids-query",
        }
    }
}

impl fmt::Display for EwdsOperation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EwdsInboundMessage {
    pub payload: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EwdsRequestEnvelope {
    #[serde(alias = "request_id")]
    pub request_id: String,
    pub operation: EwdsOperation,
    pub payload: Value,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EwdsSendMessageDto {
    pub fqcn: String,
    pub topic_name: String,
    pub topic_version: String,
    pub topic_owner: String,
    pub transaction_id: String,
    pub payload: String,
    pub anonymous_recipient: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EwdsMessageDto {
    pub payload: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EwdsResponseEnvelope<T> {
    pub request_id: String,
    pub success: bool,
    pub data: Vec<T>,
    pub error: Option<EwdsErrorPayload>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EwdsQueryResponse<T> {
    #[serde(alias = "request_id")]
    pub request_id: String,
    pub success: bool,
    pub data: Option<Vec<T>>,
    pub error: Option<EwdsErrorPayload>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct EwdsErrorPayload {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EwdsOrderDto {
    pub order_id: String,
    pub market_id: String,
    pub order_type: String,
    pub status: String,
    pub area_uuid: String,
    #[serde(default)]
    pub nonce: Option<u64>,
    pub time_slot: u64,
    pub creation_time: u64,
    pub quantity: f64,
    pub price_limit: f64,
    pub created_by: String,
    #[serde(default)]
    pub requirements: Option<EwdsRequirementsDto>,
    #[serde(default)]
    pub attributes: Option<EwdsAttributesDto>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EwdsRequirementsDto {
    #[serde(default)]
    pub trading_partner_id: Option<String>,
    #[serde(default)]
    pub energy_type: Option<String>,
    #[serde(default)]
    pub preferred_energy_rate: Option<f64>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EwdsAttributesDto {
    #[serde(default)]
    pub trading_partner_id: Option<String>,
    pub energy_type: String,
}

pub struct EwdsQueryRequest {
    pub operation: EwdsOperation,
    pub query_payload: Value,
    pub request_topic_env: &'static str,
    pub request_topic_default: &'static str,
    pub response_topic_env: &'static str,
    pub response_topic_default: &'static str,
    pub response_client_id_env: &'static str,
    pub response_client_id_default: &'static str,
    pub timeout_ms_default: u64,
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

pub async fn query_via_ewds<T: DeserializeOwned>(request: EwdsQueryRequest) -> Result<Vec<T>> {
    let gateway_base =
        env::var("EWDS_GATEWAY_URL").unwrap_or_else(|_| "http://ewds-gateway-api:3333".to_string());
    let request_fqcn = env_var("EWDS_REQUEST_PUBLISH_FQCN")
        .or_else(|| env_var("EWDS_REQUEST_FQCN"))
        .unwrap_or_else(|| "gsy.intelligent.requests.pub".to_string());
    let response_fqcn = env_var("EWDS_RESPONSE_SUBSCRIBE_FQCN")
        .or_else(|| env_var("EWDS_RESPONSE_FQCN"))
        .unwrap_or_else(|| "gsy.intelligent.responses.sub".to_string());
    let topic_owner = env::var("EWDS_TOPIC_OWNER")
        .unwrap_or_else(|_| "integration.apps.intelligent.auth.ewc".to_string());
    let topic_version = env::var("EWDS_TOPIC_VERSION").unwrap_or_else(|_| "1.0.0".to_string());
    let response_client_id = env_var(request.response_client_id_env)
        .or_else(|| env_var("EWDS_RESPONSE_CLIENT_ID"))
        .unwrap_or_else(|| request.response_client_id_default.to_string());
    let request_topic = env::var(request.request_topic_env)
        .unwrap_or_else(|_| request.request_topic_default.to_string());
    let response_topic = env::var(request.response_topic_env)
        .unwrap_or_else(|_| request.response_topic_default.to_string());

    let timeout_ms = env::var("EWDS_RESPONSE_TIMEOUT_MS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(request.timeout_ms_default);
    let poll_interval_ms = env::var("EWDS_RESPONSE_POLL_INTERVAL_MS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(400);

    let request_id = format!(
        "{}-{}-{}",
        request.operation.request_id_prefix(),
        chrono::Utc::now().timestamp_millis(),
        std::process::id()
    );

    let envelope = EwdsRequestEnvelope {
        request_id: request_id.clone(),
        operation: request.operation,
        payload: request.query_payload,
    };

    let send_message_body = EwdsSendMessageDto {
        fqcn: request_fqcn,
        topic_name: request_topic,
        topic_version,
        topic_owner: topic_owner.clone(),
        transaction_id: request_id.clone(),
        payload: serde_json::to_string(&envelope)?,
        anonymous_recipient: Vec::new(),
    };

    let client = reqwest::Client::new();
    let post_url = format!("{}/api/v2/messages", gateway_base.trim_end_matches('/'));
    let send_response = client
        .post(post_url)
        .json(&send_message_body)
        .send()
        .await?;
    let send_status = send_response.status();
    if !send_status.is_success() {
        let body = send_response.text().await.unwrap_or_default();
        return Err(anyhow!(
            "EWDS message send failed for {}: HTTP {}{}",
            request.operation,
            send_status,
            format_response_body(&body)
        ));
    }

    let started = Instant::now();
    let get_url = format!("{}/api/v2/messages", gateway_base.trim_end_matches('/'));
    let poll_client_id = client_id_for_suffix(response_client_id.as_str(), response_topic.as_str());
    loop {
        if started.elapsed().as_millis() as u64 > timeout_ms {
            return Err(anyhow!(
                "EWDS timeout waiting for {} response (request_id={})",
                request.operation,
                request_id
            ));
        }

        let response = client
            .get(get_url.as_str())
            .query(&[
                ("fqcn", response_fqcn.as_str()),
                ("amount", "100"),
                ("topicName", response_topic.as_str()),
                ("topicOwner", topic_owner.as_str()),
                ("clientId", poll_client_id.as_str()),
            ])
            .send()
            .await?;

        let status = response.status();
        if status.is_success() {
            let messages = response
                .json::<Vec<EwdsMessageDto>>()
                .await
                .unwrap_or_default();
            for message in messages {
                let parsed = serde_json::from_str::<EwdsQueryResponse<T>>(&message.payload);
                if let Ok(parsed_payload) = parsed {
                    if parsed_payload.request_id == request_id {
                        if !parsed_payload.success {
                            let error_message = parsed_payload
                                .error
                                .map(|error| format!("{}: {}", error.code, error.message))
                                .unwrap_or_else(|| "Unknown EWDS error".to_string());
                            return Err(anyhow!(
                                "EWDS {} returned error (request_id={}): {}",
                                request.operation,
                                request_id,
                                error_message
                            ));
                        }
                        return Ok(parsed_payload.data.unwrap_or_default());
                    }
                }
            }
        } else {
            let body = response.text().await.unwrap_or_default();
            return Err(anyhow!(
                "EWDS response poll failed for {} (request_id={}): HTTP {}{}",
                request.operation,
                request_id,
                status,
                format_response_body(&body)
            ));
        }

        sleep(Duration::from_millis(poll_interval_ms)).await;
    }
}

pub fn format_response_body(body: &str) -> String {
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

pub fn env_var(key: &str) -> Option<String> {
    env::var(key)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

pub fn client_id_for_suffix(base: &str, suffix: &str) -> String {
    let mut value = String::with_capacity(base.len() + suffix.len());
    value.extend(base.chars().filter(|ch| ch.is_ascii_alphanumeric()));
    value.extend(suffix.chars().filter(|ch| ch.is_ascii_alphanumeric()));

    if value.is_empty() {
        base.to_string()
    } else {
        value
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

pub async fn get_onchain_id_via_ewds(
    offchain_id: String,
) -> Result<String> {
    let query = serde_json::json!({
        "offchain_id": offchain_id,
    });
    eprintln!("get_onchain_id_via_ewds{}", query); // todo remove
    let ids: Vec<IdMappingSchema> = query_via_ewds(EwdsQueryRequest {
        operation: EwdsOperation::IdsQuery,
        query_payload: query.clone(),
        request_topic_env: "EWDS_ID_REQUEST_TOPIC",
        request_topic_default: "idsQuery",
        response_topic_env: "EWDS_ID_RESPONSE_TOPIC",
        response_topic_default: "idsQueryResponse",
        response_client_id_env: "EWDS_MARKET_ORCHESTRATOR_ID", //todo
        response_client_id_default: "idsQueryUser", //todo
        timeout_ms_default: 8_000,
    }
    )
        .await?;
    eprintln!("get_onchain_id_via_ewds return value: {:?}", ids);  // todo remove
    let result = match ids.len() {
        1 => ids.into_iter().next().unwrap().onchain_id,
        n => anyhow::bail!("expected exactly one result, got {n}"),
    };
    Ok(result)
}
