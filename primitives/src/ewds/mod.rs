pub mod dto;

use anyhow::{anyhow, Result};
use dto::{EwdsMessageDto, EwdsQueryResponse, EwdsRequestEnvelope, EwdsSendMessageDto};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::Value;
use std::{env, fmt, time::Instant};
use tokio::time::{sleep, Duration};
use crate::db_api_schema::ids::IdMappingSchema;

const DEFAULT_GATEWAY_URL: &str = "http://ewds-gateway-api:3333";
const DEFAULT_REQUEST_FQCN: &str = "gsy.intelligent.requests.pub";
const DEFAULT_RESPONSE_FQCN: &str = "gsy.intelligent.responses.sub";
const DEFAULT_TOPIC_OWNER: &str = "integration.apps.intelligent.auth.ewc";
const DEFAULT_TOPIC_VERSION: &str = "1.0.0";
const DEFAULT_POLL_INTERVAL_MS: u64 = 400;

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
    pub const ALL: [Self; 4] = [
        Self::OrdersQuery,
        Self::TradesQuery,
        Self::MeasurementsQuery,
        Self::IdsQuery,
    ];

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EwdsTopicPair {
    pub request: String,
    pub response: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EwdsTopicConfig {
    orders: EwdsTopicPair,
    trades: EwdsTopicPair,
    measurements: EwdsTopicPair,
    ids: EwdsTopicPair,
}

impl Default for EwdsTopicConfig {
    fn default() -> Self {
        Self {
            orders: EwdsTopicPair {
                request: "ordersQuery".to_string(),
                response: "ordersQueryResponse".to_string(),
            },
            trades: EwdsTopicPair {
                request: "tradesQuery".to_string(),
                response: "tradesQueryResponse".to_string(),
            },
            measurements: EwdsTopicPair {
                request: "measurementsQuery".to_string(),
                response: "measurementsQueryResponse".to_string(),
            },
            ids: EwdsTopicPair {
                request: "idsQuery".to_string(),
                response: "idsQueryResponse".to_string(),
            }
        }
    }
}

impl EwdsTopicConfig {
    pub fn from_env() -> Self {
        let defaults = Self::default();
        Self {
            orders: EwdsTopicPair {
                request: env_or(
                    "EWDS_ORDERS_REQUEST_TOPIC",
                    defaults.orders.request.as_str(),
                ),
                response: env_or(
                    "EWDS_ORDERS_RESPONSE_TOPIC",
                    defaults.orders.response.as_str(),
                ),
            },
            trades: EwdsTopicPair {
                request: env_or(
                    "EWDS_TRADES_REQUEST_TOPIC",
                    defaults.trades.request.as_str(),
                ),
                response: env_or(
                    "EWDS_TRADES_RESPONSE_TOPIC",
                    defaults.trades.response.as_str(),
                ),
            },
            measurements: EwdsTopicPair {
                request: env_or(
                    "EWDS_MEASUREMENTS_REQUEST_TOPIC",
                    defaults.measurements.request.as_str(),
                ),
                response: env_or(
                    "EWDS_MEASUREMENTS_RESPONSE_TOPIC",
                    defaults.measurements.response.as_str(),
                ),
            },
            ids: EwdsTopicPair {
                request: env_or(
                    "EWDS_IDS_REQUEST_TOPIC",
                    defaults.ids.request.as_str(),
                ),
                response: env_or(
                    "EWDS_IDS_RESPONSE_TOPIC",
                    defaults.ids.response.as_str(),
                )
            }
        }
    }

    pub fn for_operation(&self, operation: EwdsOperation) -> &EwdsTopicPair {
        match operation {
            EwdsOperation::OrdersQuery => &self.orders,
            EwdsOperation::TradesQuery => &self.trades,
            EwdsOperation::MeasurementsQuery => &self.measurements,
            EwdsOperation::IdsQuery => &self.ids,
        }
    }
}

#[derive(Debug, Clone)]
pub struct EwdsClientConfig {
    pub gateway_base: String,
    pub request_fqcn: String,
    pub response_fqcn: String,
    pub topic_owner: String,
    pub topic_version: String,
    pub consumer_client_id: String,
    pub timeout_ms: u64,
    pub poll_interval_ms: u64,
    pub topics: EwdsTopicConfig,
}

impl EwdsClientConfig {
    pub fn from_env(
        consumer_client_id_env: &str,
        consumer_client_id_default: &str,
        timeout_ms_default: u64,
    ) -> Self {
        Self {
            gateway_base: env_or("EWDS_GATEWAY_URL", DEFAULT_GATEWAY_URL),
            request_fqcn: env_var("EWDS_REQUEST_PUBLISH_FQCN")
                .or_else(|| env_var("EWDS_REQUEST_FQCN"))
                .unwrap_or_else(|| DEFAULT_REQUEST_FQCN.to_string()),
            response_fqcn: env_var("EWDS_RESPONSE_SUBSCRIBE_FQCN")
                .or_else(|| env_var("EWDS_RESPONSE_FQCN"))
                .unwrap_or_else(|| DEFAULT_RESPONSE_FQCN.to_string()),
            topic_owner: env_or("EWDS_TOPIC_OWNER", DEFAULT_TOPIC_OWNER),
            topic_version: env_or("EWDS_TOPIC_VERSION", DEFAULT_TOPIC_VERSION),
            consumer_client_id: env_var(consumer_client_id_env)
                .or_else(|| env_var("EWDS_RESPONSE_CLIENT_ID"))
                .unwrap_or_else(|| consumer_client_id_default.to_string()),
            timeout_ms: env_u64_or("EWDS_RESPONSE_TIMEOUT_MS", timeout_ms_default),
            poll_interval_ms: env_u64_or(
                "EWDS_RESPONSE_POLL_INTERVAL_MS",
                DEFAULT_POLL_INTERVAL_MS,
            ),
            topics: EwdsTopicConfig::from_env(),
        }
    }
}

pub struct EwdsClient {
    client: reqwest::Client,
    config: EwdsClientConfig,
}

struct PendingQuery {
    operation: EwdsOperation,
    request_id: String,
    response_topic: String,
}

impl EwdsClient {
    pub fn new(config: EwdsClientConfig) -> Self {
        Self {
            client: reqwest::Client::new(),
            config,
        }
    }

    pub fn from_env(
        consumer_client_id_env: &str,
        consumer_client_id_default: &str,
        timeout_ms_default: u64,
    ) -> Self {
        Self::new(EwdsClientConfig::from_env(
            consumer_client_id_env,
            consumer_client_id_default,
            timeout_ms_default,
        ))
    }

    pub async fn query<T: DeserializeOwned>(
        &self,
        operation: EwdsOperation,
        query_payload: Value,
    ) -> Result<Vec<T>> {
        let pending_query = self.send_query(operation, query_payload).await?;
        self.poll_response(pending_query).await
    }

    async fn send_query(
        &self,
        operation: EwdsOperation,
        query_payload: Value,
    ) -> Result<PendingQuery> {
        let request_id = format!(
            "{}-{}-{}",
            operation.request_id_prefix(),
            chrono::Utc::now().timestamp_millis(),
            std::process::id()
        );
        let topic_pair = self.config.topics.for_operation(operation);
        let envelope = EwdsRequestEnvelope {
            request_id: request_id.clone(),
            operation,
            payload: query_payload,
        };
        let send_message_body = EwdsSendMessageDto {
            fqcn: self.config.request_fqcn.clone(),
            topic_name: topic_pair.request.clone(),
            topic_version: self.config.topic_version.clone(),
            topic_owner: self.config.topic_owner.clone(),
            transaction_id: request_id.clone(),
            payload: serde_json::to_string(&envelope)?,
            anonymous_recipient: Vec::new(),
        };

        let post_url = format!(
            "{}/api/v2/messages",
            self.config.gateway_base.trim_end_matches('/')
        );
        let send_response = self
            .client
            .post(post_url)
            .json(&send_message_body)
            .send()
            .await?;
        let send_status = send_response.status();
        if !send_status.is_success() {
            let body = send_response.text().await.unwrap_or_default();
            return Err(anyhow!(
                "EWDS message send failed for {}: HTTP {}{}",
                operation,
                send_status,
                format_response_body(&body)
            ));
        }

        Ok(PendingQuery {
            operation,
            request_id,
            response_topic: topic_pair.response.clone(),
        })
    }

    async fn poll_response<T: DeserializeOwned>(
        &self,
        pending_query: PendingQuery,
    ) -> Result<Vec<T>> {
        let started = Instant::now();
        let get_url = format!(
            "{}/api/v2/messages",
            self.config.gateway_base.trim_end_matches('/')
        );
        let poll_client_id = client_id_for_suffix(
            self.config.consumer_client_id.as_str(),
            pending_query.response_topic.as_str(),
        );

        loop {
            if started.elapsed().as_millis() as u64 > self.config.timeout_ms {
                return Err(anyhow!(
                    "EWDS timeout waiting for {} response (request_id={})",
                    pending_query.operation,
                    pending_query.request_id
                ));
            }

            let response = self
                .client
                .get(get_url.as_str())
                .query(&[
                    ("fqcn", self.config.response_fqcn.as_str()),
                    ("amount", "100"),
                    ("topicName", pending_query.response_topic.as_str()),
                    ("topicOwner", self.config.topic_owner.as_str()),
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
                        if parsed_payload.request_id == pending_query.request_id {
                            if !parsed_payload.success {
                                let error_message = parsed_payload
                                    .error
                                    .map(|error| format!("{}: {}", error.code, error.message))
                                    .unwrap_or_else(|| "Unknown EWDS error".to_string());
                                return Err(anyhow!(
                                    "EWDS {} returned error (request_id={}): {}",
                                    pending_query.operation,
                                    pending_query.request_id,
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
                    pending_query.operation,
                    pending_query.request_id,
                    status,
                    format_response_body(&body)
                ));
            }

            sleep(Duration::from_millis(self.config.poll_interval_ms)).await;
        }
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

fn env_or(key: &str, default: &str) -> String {
    env_var(key).unwrap_or_else(|| default.to_string())
}

fn env_u64_or(key: &str, default: u64) -> u64 {
    env_var(key)
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(default)
}

pub async fn get_onchain_id_via_ewds(
    offchain_id: String,
) -> Result<String> {
    let query_payload = serde_json::json!({
        "offchain_id": offchain_id,
    });
    let ewds_client = EwdsClient::from_env(
        "EWDS_ID_CLIENT_ID", // todo
        "gsydex", // todo
        60_000,
    );
    eprintln!("get_onchain_id_via_ewds{}", query_payload); // todo remove
    let ids: Vec<IdMappingSchema> = ewds_client
        .query(EwdsOperation::IdsQuery, query_payload)
        .await?;
    eprintln!("get_onchain_id_via_ewds return value: {:?}", ids);  // todo remove
    let result = match ids.len() {
        1 => ids.into_iter().next().unwrap().onchain_id,
        n => anyhow::bail!("expected exactly one result, got {n}"),
    };
    Ok(result)
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn operations_map_to_their_topic_pairs() {
        let topics = EwdsTopicConfig::default();

        assert_eq!(
            topics.for_operation(EwdsOperation::OrdersQuery),
            &EwdsTopicPair {
                request: "ordersQuery".to_string(),
                response: "ordersQueryResponse".to_string(),
            }
        );
        assert_eq!(
            topics.for_operation(EwdsOperation::TradesQuery),
            &EwdsTopicPair {
                request: "tradesQuery".to_string(),
                response: "tradesQueryResponse".to_string(),
            }
        );
        assert_eq!(
            topics.for_operation(EwdsOperation::MeasurementsQuery),
            &EwdsTopicPair {
                request: "measurementsQuery".to_string(),
                response: "measurementsQueryResponse".to_string(),
            }
        );
    }
}
