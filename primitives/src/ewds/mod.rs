pub mod channel;
pub mod dto;

use anyhow::{anyhow, bail, Context, Result};
use channel::{route_event_by_type, EwdsChannelPoller, EwdsChannelPollerConfig};
use dto::{
    EwdsDeliverySummary, EwdsEventEnvelope, EwdsInboundMessage, EwdsQueryResponse,
    EwdsRequestEnvelope, EwdsSendMessageDto, EwdsSendMessageResponse,
};
use futures::future::{join, join_all};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::Value;
use std::collections::{HashSet, VecDeque};
use std::future::Future;
use std::{env, fmt, time::Instant};
use tokio::sync::mpsc::UnboundedReceiver;
use tokio::time::{sleep, timeout, Duration};
use tracing::{error, info, warn};

const DEFAULT_GATEWAY_URL: &str = "http://ewds-gateway-api:3333";
const DEFAULT_REQUEST_FQCN: &str = "gsy.intelligent.requests.pub";
const DEFAULT_RESPONSE_FQCN: &str = "gsy.intelligent.responses.sub";
const DEFAULT_EVENT_PUBLISH_FQCN: &str = "gsy.intelligent.events.pub";
const DEFAULT_EVENT_SUBSCRIBE_FQCN: &str = "gsy.intelligent.events.sub";
const DEFAULT_EVENT_BATCH_SIZE: u32 = 100;
const DEFAULT_EVENT_POLL_INTERVAL_MS: u64 = 1_000;
const DEFAULT_EVENT_HANDLE_ATTEMPTS: u32 = 8;
const DEFAULT_EVENT_RETRY_DELAY_MS: u64 = 2_000;
const MAX_EVENT_RETRY_DELAY_MS: u64 = 300_000;
const DEFAULT_TOPIC_OWNER: &str = "integration.apps.intelligent.auth.ewc";
const DEFAULT_TOPIC_VERSION: &str = "1.0.0";
const DEFAULT_POLL_INTERVAL_MS: u64 = 1_000;
const DEFAULT_EMPTY_RESPONSE_GRACE_MS: u64 = 10_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EwdsOperation {
    #[serde(rename = "orders.query")]
    OrdersQuery,
    #[serde(rename = "trades.query")]
    TradesQuery,
    #[serde(rename = "measurements.query")]
    MeasurementsQuery,
    #[serde(rename = "communities.query")]
    CommunitiesQuery,
    #[serde(rename = "ids.query")]
    IdsQuery,
    #[serde(rename = "clearing_results.query")]
    ClearingResultsQuery,
    #[serde(rename = "markets.query")]
    MarketsQuery,
    #[serde(rename = "facilities.query")]
    FacilitiesQuery,
}

impl EwdsOperation {
    pub const ALL: [Self; 8] = [
        Self::OrdersQuery,
        Self::TradesQuery,
        Self::MeasurementsQuery,
        Self::CommunitiesQuery,
        Self::ClearingResultsQuery,
        Self::MarketsQuery,
        Self::FacilitiesQuery,
        Self::IdsQuery,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::OrdersQuery => "orders.query",
            Self::TradesQuery => "trades.query",
            Self::MeasurementsQuery => "measurements.query",
            Self::CommunitiesQuery => "communities.query",
            Self::IdsQuery => "ids.query",
            Self::ClearingResultsQuery => "clearing_results.query",
            Self::MarketsQuery => "markets.query",
            Self::FacilitiesQuery => "facilities.query",
        }
    }

    pub fn request_id_prefix(self) -> &'static str {
        match self {
            Self::OrdersQuery => "orders-query",
            Self::TradesQuery => "trades-query",
            Self::MeasurementsQuery => "measurements-query",
            Self::CommunitiesQuery => "communities-query",
            Self::IdsQuery => "ids-query",
            Self::ClearingResultsQuery => "clearing_results-query",
            Self::MarketsQuery => "markets-query",
            Self::FacilitiesQuery => "facilities-query",
        }
    }
}

impl fmt::Display for EwdsOperation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EwdsEventType {
    #[serde(rename = "trade.created")]
    TradeCreated,
    #[serde(rename = "clearing_result.created")]
    ClearingResultCreated,
    #[serde(rename = "market_status.updated")]
    MarketStatusUpdated,
    #[serde(rename = "measurements.submitted")]
    MeasurementsSubmitted,
    #[serde(rename = "facility.submitted")]
    FacilitySubmitted,
    #[serde(rename = "site.submitted")]
    SiteSubmitted,
    #[serde(rename = "community.submitted")]
    CommunitySubmitted,
    #[serde(rename = "order.submitted")]
    OrderSubmitted,
}

impl EwdsEventType {
    pub const ALL: [Self; 8] = [
        Self::TradeCreated,
        Self::ClearingResultCreated,
        Self::MarketStatusUpdated,
        Self::MeasurementsSubmitted,
        Self::FacilitySubmitted,
        Self::SiteSubmitted,
        Self::CommunitySubmitted,
        Self::OrderSubmitted,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::TradeCreated => "trade.created",
            Self::ClearingResultCreated => "clearing_result.created",
            Self::MarketStatusUpdated => "market_status.updated",
            Self::MeasurementsSubmitted => "measurements.submitted",
            Self::FacilitySubmitted => "facility.submitted",
            Self::SiteSubmitted => "site.submitted",
            Self::CommunitySubmitted => "community.submitted",
            Self::OrderSubmitted => "order.submitted",
        }
    }
}

impl fmt::Display for EwdsEventType {
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
    communities: EwdsTopicPair,
    ids: EwdsTopicPair,
    clearing_results: EwdsTopicPair,
    markets: EwdsTopicPair,
    facilities: EwdsTopicPair,
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
            communities: EwdsTopicPair {
                request: "communitiesQuery".to_string(),
                response: "communitiesQueryResponse".to_string(),
            },
            ids: EwdsTopicPair {
                request: "idsQuery".to_string(),
                response: "idsQueryResponse".to_string(),
            },
            clearing_results: EwdsTopicPair {
                request: "clearingResultsQuery".to_string(),
                response: "clearingResultsQueryResponse".to_string(),
            },
            markets: EwdsTopicPair {
                request: "marketsQuery".to_string(),
                response: "marketsQueryResponse".to_string(),
            },
            facilities: EwdsTopicPair {
                request: "facilitiesQuery".to_string(),
                response: "facilitiesQueryResponse".to_string(),
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
            ids: EwdsTopicPair {
                request: env_or("EWDS_IDS_REQUEST_TOPIC", defaults.ids.request.as_str()),
                response: env_or("EWDS_IDS_RESPONSE_TOPIC", defaults.ids.response.as_str()),
            },
            clearing_results: EwdsTopicPair {
                request: env_or(
                    "EWDS_CLEARING_RESULTS_REQUEST_TOPIC",
                    defaults.clearing_results.request.as_str(),
                ),
                response: env_or(
                    "EWDS_CLEARING_RESULTS_RESPONSE_TOPIC",
                    defaults.clearing_results.response.as_str(),
                ),
            },
            markets: EwdsTopicPair {
                request: env_or(
                    "EWDS_MARKETS_REQUEST_TOPIC",
                    defaults.markets.request.as_str(),
                ),
                response: env_or(
                    "EWDS_MARKETS_RESPONSE_TOPIC",
                    defaults.markets.response.as_str(),
                ),
            },
            facilities: EwdsTopicPair {
                request: env_or(
                    "EWDS_FACILITIES_REQUEST_TOPIC",
                    defaults.facilities.request.as_str(),
                ),
                response: env_or(
                    "EWDS_FACILITIES_RESPONSE_TOPIC",
                    defaults.facilities.response.as_str(),
                ),
            },
        }
    }

    pub fn for_operation(&self, operation: EwdsOperation) -> &EwdsTopicPair {
        match operation {
            EwdsOperation::OrdersQuery => &self.orders,
            EwdsOperation::TradesQuery => &self.trades,
            EwdsOperation::MeasurementsQuery => &self.measurements,
            EwdsOperation::CommunitiesQuery => &self.communities,
            EwdsOperation::IdsQuery => &self.ids,
            EwdsOperation::ClearingResultsQuery => &self.clearing_results,
            EwdsOperation::MarketsQuery => &self.markets,
            EwdsOperation::FacilitiesQuery => &self.facilities,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EwdsEventTopicConfig {
    trade_event: String,
    clearing_result_event: String,
    market_event: String,
    measurements_event: String,
    facility_event: String,
    site_event: String,
    community_event: String,
    order_event: String,
}

impl Default for EwdsEventTopicConfig {
    fn default() -> Self {
        Self {
            trade_event: "trade".to_string(),
            clearing_result_event: "clearingResult".to_string(),
            market_event: "market".to_string(),
            measurements_event: "measurements".to_string(),
            facility_event: "facility".to_string(),
            site_event: "site".to_string(),
            community_event: "community".to_string(),
            order_event: "order".to_string(),
        }
    }
}

impl EwdsEventTopicConfig {
    pub fn from_env() -> Self {
        let defaults = Self::default();
        Self {
            trade_event: env_or("EWDS_TRADE_EVENT_TOPIC", defaults.trade_event.as_str()),
            clearing_result_event: env_or(
                "EWDS_CLEARING_RESULT_EVENT_TOPIC",
                defaults.clearing_result_event.as_str(),
            ),
            market_event: env_or("EWDS_MARKET_EVENT_TOPIC", defaults.market_event.as_str()),
            measurements_event: env_or(
                "EWDS_MEASUREMENTS_EVENT_TOPIC",
                defaults.measurements_event.as_str(),
            ),
            facility_event: env_or(
                "EWDS_FACILITY_EVENT_TOPIC",
                defaults.facility_event.as_str(),
            ),
            site_event: env_or("EWDS_SITE_EVENT_TOPIC", defaults.site_event.as_str()),
            community_event: env_or(
                "EWDS_COMMUNITY_EVENT_TOPIC",
                defaults.community_event.as_str(),
            ),
            order_event: env_or("EWDS_ORDER_EVENT_TOPIC", defaults.order_event.as_str()),
        }
    }

    pub fn for_event_type(&self, event_type: EwdsEventType) -> &str {
        match event_type {
            EwdsEventType::TradeCreated => &self.trade_event,
            EwdsEventType::ClearingResultCreated => &self.clearing_result_event,
            EwdsEventType::MarketStatusUpdated => &self.market_event,
            EwdsEventType::MeasurementsSubmitted => &self.measurements_event,
            EwdsEventType::FacilitySubmitted => &self.facility_event,
            EwdsEventType::SiteSubmitted => &self.site_event,
            EwdsEventType::CommunitySubmitted => &self.community_event,
            EwdsEventType::OrderSubmitted => &self.order_event,
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
    pub empty_response_grace_ms: u64,
    pub topics: EwdsTopicConfig,
    pub event_publish_fqcn: String,
    pub event_subscribe_fqcn: String,
    /// How many events one poll of an event topic fetches at most.
    pub event_batch_size: u32,
    /// The pause between two polls of an event topic.
    pub event_poll_interval_ms: u64,
    /// How often an event whose handler fails is tried in total before it is dropped.
    pub event_handle_attempts: u32,
    /// The delay before the first retry of a failed event. It doubles with every further
    /// attempt, up to 5 minutes.
    pub event_retry_delay_ms: u64,
    pub event_topics: EwdsEventTopicConfig,
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
            empty_response_grace_ms: env_u64_or(
                "EWDS_EMPTY_RESPONSE_GRACE_MS",
                DEFAULT_EMPTY_RESPONSE_GRACE_MS,
            ),
            topics: EwdsTopicConfig::from_env(),
            event_publish_fqcn: env_or("EWDS_EVENT_PUBLISH_FQCN", DEFAULT_EVENT_PUBLISH_FQCN),
            event_subscribe_fqcn: env_or("EWDS_EVENT_SUBSCRIBE_FQCN", DEFAULT_EVENT_SUBSCRIBE_FQCN),
            event_batch_size: env_var("EWDS_EVENT_BATCH_SIZE")
                .and_then(|value| value.parse::<u32>().ok())
                .unwrap_or(DEFAULT_EVENT_BATCH_SIZE),
            event_poll_interval_ms: env_u64_or(
                "EWDS_EVENT_POLL_INTERVAL_MS",
                DEFAULT_EVENT_POLL_INTERVAL_MS,
            ),
            event_handle_attempts: env_var("EWDS_EVENT_HANDLE_ATTEMPTS")
                .and_then(|value| value.parse::<u32>().ok())
                .unwrap_or(DEFAULT_EVENT_HANDLE_ATTEMPTS),
            event_retry_delay_ms: env_u64_or(
                "EWDS_EVENT_RETRY_DELAY_MS",
                DEFAULT_EVENT_RETRY_DELAY_MS,
            ),
            event_topics: EwdsEventTopicConfig::from_env(),
        }
    }
}

pub struct EwdsClient {
    client: reqwest::Client,
    config: EwdsClientConfig,
}

/// An inbound event waiting to be handled, or to be retried after its handler failed.
struct QueuedEvent {
    envelope: EwdsEventEnvelope<Value>,
    attempts: u32,
    retry_at: Instant,
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

    pub async fn publish(
        &self,
        fqcn: &str,
        topic_name: &str,
        transaction_id: &str,
        payload: String,
    ) -> Result<()> {
        let message = EwdsSendMessageDto {
            fqcn: fqcn.to_string(),
            topic_name: topic_name.to_string(),
            topic_version: self.config.topic_version.clone(),
            topic_owner: self.config.topic_owner.clone(),
            transaction_id: transaction_id.to_string(),
            payload,
            anonymous_recipient: Vec::new(),
        };
        self.post_message(
            &message,
            format!("{} message", topic_name).as_str(),
            Instant::now(),
        )
        .await
    }

    pub async fn publish_event<T: Serialize>(&self, event: &EwdsEventEnvelope<T>) -> Result<()> {
        self.publish(
            self.config.event_publish_fqcn.as_str(),
            self.config.event_topics.for_event_type(event.event_type),
            event.event_id.as_str(),
            serde_json::to_string(event)?,
        )
        .await
    }

    /// Polls the events channel once for all of `event_types` and passes every new event to
    /// `handle`. Each event type's topic has its own queue, handled in the order the events
    /// arrived; malformed messages, events of another type and events seen before are skipped.
    ///
    /// The gateway acknowledges a message when it is polled, so a failed event is never
    /// delivered again and the subscriber retries it itself: up to `event_handle_attempts`
    /// times in total, with a delay that starts at `event_retry_delay_ms` and doubles with every
    /// attempt. Meanwhile the later events of its topic wait, so an older event never
    /// overwrites the data of a newer one; the other topics carry on. An event whose handler
    /// error is marked with [`invalid_event`] is dropped at once, because handling it again
    /// would fail the same way. Runs until the task is dropped.
    pub async fn run_event_subscriber<F, Fut>(&self, event_types: &[EwdsEventType], handle: F)
    where
        F: Fn(EwdsEventEnvelope<Value>) -> Fut,
        Fut: Future<Output = Result<()>>,
    {
        let mut poller = self.event_channel_poller();
        let workers = event_types
            .iter()
            .map(|event_type| {
                let topic_name = self.config.event_topics.for_event_type(*event_type);
                let messages = poller.route(topic_name);
                self.run_topic_event_worker(*event_type, messages, &handle)
            })
            .collect::<Vec<_>>();

        join(poller.run(), join_all(workers)).await;
    }

    /// The poller of the events channel. Its `clientId` is the consumer ID plus `events`, so it
    /// never shares a cursor with the per-topic response polls of the same consumer.
    fn event_channel_poller(&self) -> EwdsChannelPoller {
        EwdsChannelPoller::new(EwdsChannelPollerConfig {
            gateway_base: self.config.gateway_base.clone(),
            fqcn: self.config.event_subscribe_fqcn.clone(),
            client_id: client_id_for_suffix(self.config.consumer_client_id.as_str(), "events"),
            topic_owner: self.config.topic_owner.clone(),
            batch_size: self.config.event_batch_size,
            poll_interval_ms: self.config.event_poll_interval_ms,
        })
        .with_fallback_router(route_event_by_type(self.config.event_topics.clone()))
    }

    /// Handles the events the poller routes to the topic of `event_type`.
    async fn run_topic_event_worker<F, Fut>(
        &self,
        event_type: EwdsEventType,
        mut messages: UnboundedReceiver<EwdsInboundMessage>,
        handle: &F,
    ) where
        F: Fn(EwdsEventEnvelope<Value>) -> Fut,
        Fut: Future<Output = Result<()>>,
    {
        let topic_name = self.config.event_topics.for_event_type(event_type);
        let mut queue: VecDeque<QueuedEvent> = VecDeque::new();
        let mut seen_event_ids: HashSet<String> = HashSet::new();
        let mut seen_queue: VecDeque<String> = VecDeque::new();

        loop {
            while let Ok(message) = messages.try_recv() {
                enqueue_event(event_type, topic_name, message, &mut queue, &seen_event_ids);
            }

            self.handle_queued_events(
                event_type,
                handle,
                &mut queue,
                &mut seen_event_ids,
                &mut seen_queue,
            )
            .await;

            // Wait for the next message, but wake up early when a retry is due.
            let next = match queue.front() {
                Some(event) => {
                    let wait = event.retry_at.saturating_duration_since(Instant::now());
                    match timeout(wait, messages.recv()).await {
                        Ok(message) => message,
                        Err(_) => continue,
                    }
                }
                None => messages.recv().await,
            };
            match next {
                Some(message) => {
                    enqueue_event(event_type, topic_name, message, &mut queue, &seen_event_ids)
                }
                None if queue.is_empty() => return,
                // The poller stopped; finish the queued events before returning.
                None => {
                    sleep(queue.front().map_or(Duration::ZERO, |event| {
                        event.retry_at.saturating_duration_since(Instant::now())
                    }))
                    .await
                }
            }
        }
    }

    /// Handles the queued events in order until the queue is empty or the first event has to
    /// wait for its retry.
    async fn handle_queued_events<F, Fut>(
        &self,
        event_type: EwdsEventType,
        handle: &F,
        queue: &mut VecDeque<QueuedEvent>,
        seen_event_ids: &mut HashSet<String>,
        seen_queue: &mut VecDeque<String>,
    ) where
        F: Fn(EwdsEventEnvelope<Value>) -> Fut,
        Fut: Future<Output = Result<()>>,
    {
        let max_attempts = self.config.event_handle_attempts.max(1);
        while queue
            .front()
            .is_some_and(|event| event.retry_at <= Instant::now())
        {
            let Some(mut event) = queue.pop_front() else {
                break;
            };
            event.attempts += 1;
            let event_id = event.envelope.event_id.clone();
            match handle(event.envelope.clone()).await {
                Ok(()) => info!("Handled EWDS {} event {}", event_type, event_id),
                Err(error) if is_invalid_event(&error) => error!(
                    "Dropping EWDS {} event {}: {:#}",
                    event_type, event_id, error
                ),
                Err(error) if event.attempts >= max_attempts => error!(
                    "Dropping EWDS {} event {} after {} failed attempts: {:#}",
                    event_type, event_id, event.attempts, error
                ),
                Err(error) => {
                    let delay_ms =
                        event_retry_delay_ms(self.config.event_retry_delay_ms, event.attempts);
                    warn!(
                        "EWDS {} event {} failed (attempt {} of {}), retrying in {} ms; {} later events wait: {:#}",
                        event_type,
                        event_id,
                        event.attempts,
                        max_attempts,
                        delay_ms,
                        queue.len(),
                        error
                    );
                    event.retry_at = Instant::now() + Duration::from_millis(delay_ms);
                    queue.push_front(event);
                    return;
                }
            }
            remember_id(&event_id, seen_event_ids, seen_queue);
        }
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
        self.post_message(
            &send_message_body,
            format!("{} request", operation).as_str(),
            started,
        )
        .await?;

        Ok(PendingQuery {
            operation,
            request_id,
            response_topic: topic_pair.response.clone(),
            started,
        })
    }

    /// Posts `message` to the gateway. Rate limits, transient gateway errors and deliveries
    /// that reached no recipient are retried until the timeout, counted from `started`.
    /// `label` names the message in errors and logs.
    async fn post_message(
        &self,
        message: &EwdsSendMessageDto,
        label: &str,
        started: Instant,
    ) -> Result<()> {
        let post_url = format!(
            "{}/api/v2/messages",
            self.config.gateway_base.trim_end_matches('/')
        );
        let mut delivery_attempt = 0u32;
        loop {
            if started.elapsed() > Duration::from_millis(self.config.timeout_ms) {
                return Err(anyhow!(
                    "EWDS timeout sending {} (transaction_id={})",
                    label,
                    message.transaction_id
                ));
            }

            let send_response = self
                .client
                .post(post_url.as_str())
                .json(message)
                .send()
                .await?;
            let send_status = send_response.status();
            let body = send_response.text().await.unwrap_or_default();
            if send_status.is_success() {
                let delivery = parse_gateway_delivery_summary(body.as_str())?;
                if delivery.sent > 0 {
                    return Ok(());
                }

                let delay_ms = ewds_rate_limit_backoff_ms(delivery_attempt);
                warn!(
                    "EWDS gateway accepted {} but delivered it to no recipients (failed={}, total={}); retrying in {} ms",
                    label, delivery.failed, delivery.total, delay_ms
                );
                delivery_attempt = delivery_attempt.saturating_add(1);
                sleep(Duration::from_millis(delay_ms)).await;
                continue;
            }

            if is_rate_limited_response(send_status, &body) {
                let delay_ms = ewds_rate_limit_backoff_ms(delivery_attempt);
                warn!(
                    "EWDS rate limit while sending {}; retrying in {} ms",
                    label, delay_ms
                );
                delivery_attempt = delivery_attempt.saturating_add(1);
                sleep(Duration::from_millis(delay_ms)).await;
                continue;
            }

            if is_transient_gateway_response(send_status, &body) {
                let delay_ms = ewds_rate_limit_backoff_ms(delivery_attempt);
                warn!(
                    "EWDS transient gateway error while sending {}; retrying in {} ms",
                    label, delay_ms
                );
                delivery_attempt = delivery_attempt.saturating_add(1);
                sleep(Duration::from_millis(delay_ms)).await;
                continue;
            }

            return Err(anyhow!(
                "EWDS message send failed for {}: HTTP {}{}",
                label,
                send_status,
                format_response_body(&body)
            ));
        }
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
        let mut empty_response_seen_at = None;

        loop {
            if empty_response_grace_elapsed(
                empty_response_seen_at,
                self.config.empty_response_grace_ms,
            ) {
                return Ok(Vec::new());
            }

            if pending_query.started.elapsed() > Duration::from_millis(self.config.timeout_ms) {
                if empty_response_seen_at.is_some() {
                    return Ok(Vec::new());
                }
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
                let messages = match parse_inbound_messages(&response.text().await?) {
                    Ok(messages) => messages,
                    Err(error) => {
                        warn!(
                            "EWDS response poll for {} (request_id={}) failed: {:#}",
                            pending_query.operation, pending_query.request_id, error
                        );
                        Vec::new()
                    }
                };
                for message in messages {
                    let parsed = serde_json::from_str::<EwdsQueryResponse<T>>(&message.payload);
                    if let Err(error) = &parsed {
                        warn_about_unparsable_response(&pending_query, &message.payload, error);
                    }
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
                            if let Some(data) = select_response_data(
                                parsed_payload.data.unwrap_or_default(),
                                &mut empty_response_seen_at,
                                self.config.empty_response_grace_ms,
                            ) {
                                return Ok(data);
                            }
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

                if is_transient_gateway_response(status, &body) {
                    let delay_ms = ewds_rate_limit_backoff_ms(rate_limit_attempt);
                    warn!(
                        "EWDS transient gateway error while polling {} response; retrying in {} ms",
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

/// Queues `message` if it is a new event of `event_type`.
fn enqueue_event(
    event_type: EwdsEventType,
    topic_name: &str,
    message: EwdsInboundMessage,
    queue: &mut VecDeque<QueuedEvent>,
    seen_event_ids: &HashSet<String>,
) {
    let envelope = match serde_json::from_str::<EwdsEventEnvelope<Value>>(&message.payload) {
        Ok(envelope) => envelope,
        Err(error) => {
            warn!(
                "Skipping malformed EWDS message on topic '{}': {}",
                topic_name, error
            );
            return;
        }
    };
    if envelope.event_type != event_type {
        warn!(
            "Skipping EWDS {} event {} on topic '{}', which carries {} events",
            envelope.event_type, envelope.event_id, topic_name, event_type
        );
        return;
    }
    if seen_event_ids.contains(&envelope.event_id)
        || queue
            .iter()
            .any(|queued| queued.envelope.event_id == envelope.event_id)
    {
        return;
    }
    queue.push_back(QueuedEvent {
        envelope,
        attempts: 0,
        retry_at: Instant::now(),
    });
}

/// Logs a response to `pending_query` whose payload could not be parsed. Messages for other
/// requests on the same topic are expected and stay silent.
fn warn_about_unparsable_response(
    pending_query: &PendingQuery,
    payload: &str,
    error: &serde_json::Error,
) {
    let request_id = serde_json::from_str::<Value>(payload)
        .ok()
        .and_then(|value| value.get("requestId")?.as_str().map(str::to_string));
    if request_id.as_deref() == Some(pending_query.request_id.as_str()) {
        warn!(
            "Ignoring EWDS {} response that could not be parsed (request_id={}): {}",
            pending_query.operation, pending_query.request_id, error
        );
    }
}

pub fn empty_response_grace_elapsed(
    empty_response_seen_at: Option<Instant>,
    grace_ms: u64,
) -> bool {
    empty_response_seen_at
        .is_some_and(|seen_at| seen_at.elapsed() >= Duration::from_millis(grace_ms))
}

pub fn select_response_data<T>(
    data: Vec<T>,
    empty_response_seen_at: &mut Option<Instant>,
    grace_ms: u64,
) -> Option<Vec<T>> {
    if data.is_empty() && grace_ms > 0 {
        empty_response_seen_at.get_or_insert_with(Instant::now);
        None
    } else {
        Some(data)
    }
}

/// The delay before the next poll of a topic: backs off while the gateway rate-limits or fails
/// transiently, and waits `poll_interval_ms` otherwise.
pub fn next_poll_delay_ms(
    result: &Result<()>,
    poll_interval_ms: u64,
    rate_limit_attempt: &mut u32,
) -> u64 {
    match result {
        Ok(()) => {
            *rate_limit_attempt = 0;
            poll_interval_ms
        }
        Err(error) => {
            let message = error.to_string();
            if is_rate_limited_message(message.as_str())
                || is_transient_gateway_message(message.as_str())
            {
                let delay_ms = ewds_rate_limit_backoff_ms(*rate_limit_attempt);
                *rate_limit_attempt = rate_limit_attempt.saturating_add(1);
                delay_ms
            } else {
                poll_interval_ms
            }
        }
    }
}

/// Remembers a handled request or event ID, keeping only the most recent ones.
pub fn remember_id(id: &str, seen_ids: &mut HashSet<String>, seen_queue: &mut VecDeque<String>) {
    const MAX_SEEN_IDS: usize = 2_048;

    seen_ids.insert(id.to_string());
    seen_queue.push_back(id.to_string());

    while seen_queue.len() > MAX_SEEN_IDS {
        if let Some(evicted) = seen_queue.pop_front() {
            seen_ids.remove(&evicted);
        }
    }
}

/// Marks the error of an event handler as caused by the event itself, e.g. invalid data.
/// [`EwdsClient::run_event_subscriber`] drops such an event instead of retrying it.
#[derive(Debug)]
pub struct InvalidEvent;

impl fmt::Display for InvalidEvent {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("invalid event")
    }
}

impl std::error::Error for InvalidEvent {}

/// Marks `error` as caused by the event itself, so the event is not retried.
pub fn invalid_event(error: anyhow::Error) -> anyhow::Error {
    error.context(InvalidEvent)
}

/// Whether `error`, or an error it wraps, was marked with [`invalid_event`].
pub fn is_invalid_event(error: &anyhow::Error) -> bool {
    error.downcast_ref::<InvalidEvent>().is_some()
}

/// The delay before retrying an event that failed `attempts` times.
pub fn event_retry_delay_ms(base_delay_ms: u64, attempts: u32) -> u64 {
    let factor = 1u64
        .checked_shl(attempts.saturating_sub(1))
        .unwrap_or(u64::MAX);
    base_delay_ms
        .saturating_mul(factor)
        .min(MAX_EVENT_RETRY_DELAY_MS)
}

/// Parses the list of items an event carries. One invalid item rejects the whole event, and
/// any problem with the data is marked with [`invalid_event`].
pub fn parse_batch<Item: DeserializeOwned, T>(
    data: Value,
    convert: impl Fn(Item) -> Result<T>,
) -> Result<Vec<T>> {
    parse_items(data, convert).map_err(invalid_event)
}

fn parse_items<Item: DeserializeOwned, T>(
    data: Value,
    convert: impl Fn(Item) -> Result<T>,
) -> Result<Vec<T>> {
    let items: Vec<Value> = serde_json::from_value(data).context("the event data is not a list")?;
    if items.is_empty() {
        bail!("the event data is empty");
    }

    items
        .into_iter()
        .enumerate()
        .map(|(index, item)| {
            serde_json::from_value::<Item>(item)
                .map_err(anyhow::Error::from)
                .and_then(&convert)
                .with_context(|| format!("invalid item at index {}", index))
        })
        .collect()
}

/// Parses the body of a successful `GET /api/v2/messages`. A body that is not a list of
/// messages is an error rather than an empty poll, so a gateway problem does not go unnoticed.
pub fn parse_inbound_messages(body: &str) -> Result<Vec<EwdsInboundMessage>> {
    serde_json::from_str::<Vec<EwdsInboundMessage>>(body).map_err(|error| {
        anyhow!(
            "the gateway returned no message list ({}){}",
            error,
            format_response_body(body)
        )
    })
}

pub fn is_rate_limited_response(status: reqwest::StatusCode, body: &str) -> bool {
    status == reqwest::StatusCode::TOO_MANY_REQUESTS || is_rate_limited_message(body)
}

pub fn is_transient_gateway_response(status: reqwest::StatusCode, body: &str) -> bool {
    status.is_server_error() || is_transient_gateway_message(body)
}

pub fn parse_gateway_delivery_summary(body: &str) -> Result<EwdsDeliverySummary> {
    serde_json::from_str::<EwdsSendMessageResponse>(body)
        .map(|response| response.recipients)
        .map_err(|error| anyhow!("invalid EWDS gateway delivery response: {}", error))
}

pub fn is_rate_limited_message(message: &str) -> bool {
    let normalized = message.to_ascii_lowercase();
    normalized.contains("status code 429")
        || normalized.contains("\"statuscode\":429")
        || normalized.contains("too many requests")
}

pub fn is_transient_gateway_message(message: &str) -> bool {
    let normalized = message.to_ascii_lowercase();
    normalized.contains("timeout or no response waiting for nats jetstream server")
        || normalized.contains("cannot destructure property 'status' of 'e.response'")
        || normalized.contains("\"statuscode\":500")
        || normalized.contains("http 500 internal server error")
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
