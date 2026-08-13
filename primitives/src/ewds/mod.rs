pub mod dto;

use anyhow::{anyhow, Result};
use dto::{EwdsMessageDto, EwdsQueryResponse, EwdsRequestEnvelope, EwdsSendMessageDto};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::Value;
use std::{env, fmt, time::Instant};
use tokio::time::{sleep, Duration};
use tracing::warn;

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
    #[serde(rename = "community.upsert")]
    CommunityUpsert,
    #[serde(rename = "communities.query")]
    CommunitiesQuery,
}

impl EwdsOperation {
    pub const ALL: [Self; 5] = [
        Self::OrdersQuery,
        Self::TradesQuery,
        Self::MeasurementsQuery,
        Self::CommunityUpsert,
        Self::CommunitiesQuery,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::OrdersQuery => "orders.query",
            Self::TradesQuery => "trades.query",
            Self::MeasurementsQuery => "measurements.query",
            Self::CommunityUpsert => "community.upsert",
            Self::CommunitiesQuery => "communities.query",
        }
    }

    pub fn request_id_prefix(self) -> &'static str {
        match self {
            Self::OrdersQuery => "orders-query",
            Self::TradesQuery => "trades-query",
            Self::MeasurementsQuery => "measurements-query",
            Self::CommunityUpsert => "community-upsert",
            Self::CommunitiesQuery => "communities-query",
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
    community_upsert: EwdsTopicPair,
    communities: EwdsTopicPair,
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
            community_upsert: EwdsTopicPair {
                request: "communityUpsert".to_string(),
                response: "communityUpsertResponse".to_string(),
            },
            communities: EwdsTopicPair {
                request: "communitiesQuery".to_string(),
                response: "communitiesQueryResponse".to_string(),
            },
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
            community_upsert: EwdsTopicPair {
                request: env_or(
                    "EWDS_COMMUNITY_UPSERT_TOPIC",
                    defaults.community_upsert.request.as_str(),
                ),
                response: env_or(
                    "EWDS_COMMUNITY_UPSERT_RESPONSE_TOPIC",
                    defaults.community_upsert.response.as_str(),
                ),
            },
            communities: EwdsTopicPair {
                request: env_or(
                    "EWDS_COMMUNITIES_REQUEST_TOPIC",
                    defaults.communities.request.as_str(),
                ),
                response: env_or(
                    "EWDS_COMMUNITIES_RESPONSE_TOPIC",
                    defaults.communities.response.as_str(),
                ),
            },
        }
    }

    pub fn for_operation(&self, operation: EwdsOperation) -> &EwdsTopicPair {
        match operation {
            EwdsOperation::OrdersQuery => &self.orders,
            EwdsOperation::TradesQuery => &self.trades,
            EwdsOperation::MeasurementsQuery => &self.measurements,
            EwdsOperation::CommunityUpsert => &self.community_upsert,
            EwdsOperation::CommunitiesQuery => &self.communities,
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
    started: Instant,
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
        let started = Instant::now();
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
        let mut rate_limit_attempt = 0u32;
        loop {
            if started.elapsed() > Duration::from_millis(self.config.timeout_ms) {
                return Err(anyhow!(
                    "EWDS timeout sending {} request (request_id={})",
                    operation,
                    request_id
                ));
            }

            let send_response = self
                .client
                .post(post_url.as_str())
                .json(&send_message_body)
                .send()
                .await?;
            let send_status = send_response.status();
            if send_status.is_success() {
                break;
            }

            let body = send_response.text().await.unwrap_or_default();
            if is_rate_limited_response(send_status, &body) {
                let delay_ms = ewds_rate_limit_backoff_ms(rate_limit_attempt);
                warn!(
                    "EWDS rate limit while sending {} request; retrying in {} ms",
                    operation, delay_ms
                );
                rate_limit_attempt = rate_limit_attempt.saturating_add(1);
                sleep(Duration::from_millis(delay_ms)).await;
                continue;
            }

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
            started,
        })
    }

    async fn poll_response<T: DeserializeOwned>(
        &self,
        pending_query: PendingQuery,
    ) -> Result<Vec<T>> {
        let get_url = format!(
            "{}/api/v2/messages",
            self.config.gateway_base.trim_end_matches('/')
        );
        let poll_client_id = client_id_for_suffix(
            self.config.consumer_client_id.as_str(),
            pending_query.response_topic.as_str(),
        );
        let mut rate_limit_attempt = 0u32;

        loop {
            if pending_query.started.elapsed() > Duration::from_millis(self.config.timeout_ms) {
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
                rate_limit_attempt = 0;
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
                if is_rate_limited_response(status, &body) {
                    let delay_ms = ewds_rate_limit_backoff_ms(rate_limit_attempt);
                    warn!(
                        "EWDS rate limit while polling {} response; retrying in {} ms",
                        pending_query.operation, delay_ms
                    );
                    rate_limit_attempt = rate_limit_attempt.saturating_add(1);
                    sleep(Duration::from_millis(delay_ms)).await;
                    continue;
                }

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

pub fn is_rate_limited_response(status: reqwest::StatusCode, body: &str) -> bool {
    status == reqwest::StatusCode::TOO_MANY_REQUESTS || is_rate_limited_message(body)
}

pub fn is_rate_limited_message(message: &str) -> bool {
    let normalized = message.to_ascii_lowercase();
    normalized.contains("status code 429")
        || normalized.contains("\"statuscode\":429")
        || normalized.contains("too many requests")
}

pub fn ewds_rate_limit_backoff_ms(attempt: u32) -> u64 {
    let base_ms = env_u64_or("EWDS_RATE_LIMIT_BACKOFF_MS", 2_000);
    let max_ms = env_u64_or("EWDS_RATE_LIMIT_MAX_BACKOFF_MS", 30_000).max(base_ms);
    let multiplier = 1u64 << attempt.min(4);

    base_ms.saturating_mul(multiplier).min(max_ms)
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
        assert_eq!(
            topics.for_operation(EwdsOperation::CommunityUpsert),
            &EwdsTopicPair {
                request: "communityUpsert".to_string(),
                response: "communityUpsertResponse".to_string(),
            }
        );
        assert_eq!(
            topics.for_operation(EwdsOperation::CommunitiesQuery),
            &EwdsTopicPair {
                request: "communitiesQuery".to_string(),
                response: "communitiesQueryResponse".to_string(),
            }
        );
    }

    #[test]
    fn community_operations_round_trip_through_the_request_envelope() {
        for operation in [
            EwdsOperation::CommunityUpsert,
            EwdsOperation::CommunitiesQuery,
        ] {
            let envelope = EwdsRequestEnvelope {
                request_id: "request-id".to_string(),
                operation,
                payload: Value::Object(Default::default()),
            };

            let serialized = serde_json::to_string(&envelope).unwrap();
            let deserialized: EwdsRequestEnvelope = serde_json::from_str(&serialized).unwrap();

            assert_eq!(deserialized.operation, operation);
        }
    }

    #[test]
    fn recognizes_client_gateway_wrapped_rate_limit() {
        let body = r#"{
            "err": {
                "code": "MB::ERROR",
                "reason": "Request failed with status code 429"
            },
            "statusCode": 400
        }"#;

        assert!(is_rate_limited_response(
            reqwest::StatusCode::BAD_REQUEST,
            body
        ));
    }

    #[test]
    fn does_not_treat_an_unrelated_bad_request_as_rate_limit() {
        assert!(!is_rate_limited_response(
            reqwest::StatusCode::BAD_REQUEST,
            r#"{"reason":"Channel not found","statusCode":400}"#
        ));
    }
}
