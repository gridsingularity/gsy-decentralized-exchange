use primitives::ewds::dto::EwdsFacilityDto;
use crate::certificates::query::{guarantees_of_origin, validate_window};
use crate::db::DatabaseWrapper;
use anyhow::Result;
use futures::future::{join, join_all};
use primitives::db_api_schema::profiles::{MeasurementPointType, MeasurementSchema};
use primitives::ewds::channel::{
    route_request_by_operation, EwdsChannelPoller, EwdsChannelPollerConfig,
};
use primitives::ewds::dto::{
    EwdsClearingResultDto, EwdsCommunityDto, EwdsErrorPayload, EwdsInboundMessage, EwdsMarketDto,
    EwdsMeasurementDto, EwdsOrderDto, EwdsRequestEnvelope, EwdsResponseEnvelope, EwdsTradeDto,
    FacilityMarketValidationRequest,
};
use primitives::ewds::{
    client_id_for_suffix, env_var, remember_id, EwdsClient, EwdsClientConfig,
    EwdsEventTopicConfig, EwdsEventType, EwdsOperation, EwdsTopicConfig,
};
use primitives::utils::{
    normalize_uuid, validate_offchain_actor_id, opt_rfc3339_to_epoch,
    timestamp_to_string_with_padding,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{HashMap, HashSet, VecDeque};
use std::fmt::Display;
use std::time::SystemTime;
use tokio::sync::mpsc::UnboundedReceiver;
use tokio::time::Duration;
use tracing::{error, info, info_span, warn, Instrument};

#[derive(Clone)]
pub struct EwdsHandlerConfig {
    pub enabled: bool,
    pub gateway_url: String,
    pub request_fqcn: String,
    pub response_fqcn: String,
    pub event_publish_fqcn: String,
    pub event_subscribe_fqcn: String,
    pub event_topics: EwdsEventTopicConfig,
    pub topic_owner: String,
    pub topic_version: String,
    pub request_client_id: String,
    pub topics: EwdsTopicConfig,
    pub poll_interval_ms: u64,
    pub event_poll_interval_ms: u64,
    pub request_batch_size: u32,
    pub response_send_timeout_ms: u64,
    /// Requests published longer ago than this are skipped: their senders have stopped waiting
    /// for a response.
    pub request_max_age_ms: u64,
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
            .unwrap_or(1_000);
        let event_poll_interval_ms = std::env::var("EWDS_EVENT_POLL_INTERVAL_MS")
            .ok()
            .and_then(|value| value.parse::<u64>().ok())
            .unwrap_or(1_000);

        let request_batch_size = std::env::var("EWDS_HANDLER_BATCH_SIZE")
            .ok()
            .and_then(|value| value.parse::<u32>().ok())
            .unwrap_or(100);
        let response_send_timeout_ms = std::env::var("EWDS_RESPONSE_TIMEOUT_MS")
            .ok()
            .and_then(|value| value.parse::<u64>().ok())
            .unwrap_or(60_000);

        let request_fqcn = env_var("EWDS_REQUEST_SUBSCRIBE_FQCN")
            .or_else(|| env_var("EWDS_REQUEST_FQCN"))
            .unwrap_or_else(|| "gsy.intelligent.requests.sub".to_string());
        let response_fqcn = env_var("EWDS_RESPONSE_PUBLISH_FQCN")
            .or_else(|| env_var("EWDS_RESPONSE_FQCN"))
            .unwrap_or_else(|| "gsy.intelligent.responses.pub".to_string());
        let event_publish_fqcn = env_var("EWDS_EVENT_PUBLISH_FQCN")
            .unwrap_or_else(|| "gsy.intelligent.events.pub".to_string());
        let event_subscribe_fqcn = env_var("EWDS_EVENT_SUBSCRIBE_FQCN")
            .unwrap_or_else(|| "gsy.intelligent.events.sub".to_string());

        Self {
            enabled,
            gateway_url: std::env::var("EWDS_GATEWAY_URL")
                .unwrap_or_else(|_| "http://ewds-gateway-api:3333".to_string()),
            request_fqcn,
            response_fqcn,
            event_publish_fqcn,
            event_subscribe_fqcn,
            event_topics: EwdsEventTopicConfig::from_env(),
            topic_owner: std::env::var("EWDS_TOPIC_OWNER")
                .unwrap_or_else(|_| "integration.apps.intelligent.auth.ewc".to_string()),
            topic_version: std::env::var("EWDS_TOPIC_VERSION")
                .unwrap_or_else(|_| "1.0.0".to_string()),
            request_client_id: env_var("EWDS_REQUEST_CLIENT_ID")
                .or_else(|| env_var("EWDS_OFFCHAIN_STORAGE_CLIENT_ID"))
                .unwrap_or_else(|| "gsyoffchainstorage".to_string()),
            topics: EwdsTopicConfig::from_env(),
            poll_interval_ms,
            event_poll_interval_ms,
            request_batch_size,
            response_send_timeout_ms,
            // The senders wait `EWDS_RESPONSE_TIMEOUT_MS` for a response, too.
            request_max_age_ms: env_var("EWDS_REQUEST_MAX_AGE_MS")
                .and_then(|value| value.parse::<u64>().ok())
                .unwrap_or(response_send_timeout_ms),
        }
    }

    /// The EWDS topic an event of the given type is published on.
    pub fn event_topic(&self, event_type: EwdsEventType) -> &str {
        self.event_topics.for_event_type(event_type)
    }

    /// The gateway client that publishes responses and events and polls events. The off-chain
    /// storage sends no queries, so the query settings keep their defaults.
    pub fn ewds_client(&self) -> EwdsClient {
        EwdsClient::new(EwdsClientConfig {
            gateway_base: self.gateway_url.clone(),
            topic_owner: self.topic_owner.clone(),
            topic_version: self.topic_version.clone(),
            consumer_client_id: self.request_client_id.clone(),
            timeout_ms: self.response_send_timeout_ms,
            event_poll_interval_ms: self.event_poll_interval_ms,
            event_publish_fqcn: self.event_publish_fqcn.clone(),
            event_subscribe_fqcn: self.event_subscribe_fqcn.clone(),
            event_batch_size: self.request_batch_size,
            event_topics: self.event_topics.clone(),
            ..EwdsClientConfig::from_env(
                "EWDS_REQUEST_CLIENT_ID",
                "gsyoffchainstorage",
                self.response_send_timeout_ms,
            )
        })
    }
}

#[derive(Deserialize)]
struct OrdersQueryPayload {
    #[serde(alias = "marketId")]
    #[serde(default)]
    market_id: Option<String>,
    #[serde(alias = "startTime")]
    #[serde(default)]
    start_time: Option<String>,
    #[serde(alias = "endTime")]
    #[serde(default)]
    end_time: Option<String>,
}

#[derive(Deserialize)]
struct TradesQueryPayload {
    #[serde(alias = "marketId")]
    #[serde(default)]
    market_id: Option<String>,
    #[serde(alias = "startTime")]
    #[serde(default)]
    start_time: Option<String>,
    #[serde(alias = "endTime")]
    #[serde(default)]
    end_time: Option<String>,
}

#[derive(Deserialize)]
struct TimeRangePayload {
    #[serde(alias = "startTime")]
    #[serde(default)]
    start_time: Option<String>,
    #[serde(alias = "endTime")]
    #[serde(default)]
    end_time: Option<String>,
    #[serde(alias = "areaUuid", alias = "facilityId")]
    #[serde(default)]
    facility_id: Option<String>,
    #[serde(alias = "communityUuid", default)]
    community_uuid: Option<String>,
}

#[derive(Deserialize)]
struct GuaranteesOfOriginPayload {
    #[serde(alias = "startTime")]
    #[serde(default)]
    start_time: Option<u64>,
    #[serde(alias = "endTime")]
    #[serde(default)]
    end_time: Option<u64>,
}

/// `code` of the error payload sent for a request rejected before it reaches storage.
const INVALID_REQUEST_ERROR_CODE: &str = "invalid_request";

/// Maximum span of a `trades.query` range. The whole result is published as a
/// single EWDS message, so the range is capped to keep that message small.
pub const MAX_TRADES_QUERY_RANGE_SECS: u64 = 86_400;
pub const TIME_RANGE_TOO_LARGE: &str = "TIME_RANGE_TOO_LARGE";
pub const INVALID_TIME_RANGE: &str = "INVALID_TIME_RANGE";

/// Validates a `trades.query` range (`endTime` is exclusive), returning the
/// bounds or the error to publish back to the requester. A query for one market
/// needs no range, since the market already limits the result; any other query
/// needs both bounds.
pub fn validate_trades_query_range(
    market_id: Option<&str>,
    start_time: Option<u64>,
    end_time: Option<u64>,
) -> std::result::Result<Option<(u64, u64)>, EwdsErrorPayload> {
    let (start, end) = match (start_time, end_time) {
        (Some(start), Some(end)) => (start, end),
        (None, None) if market_id.is_some() => return Ok(None),
        _ => {
            return Err(EwdsErrorPayload {
                code: INVALID_TIME_RANGE.to_string(),
                message: "trades.query requires marketId or both startTime and endTime".to_string(),
            })
        }
    };
    if end < start {
        return Err(EwdsErrorPayload {
            code: INVALID_TIME_RANGE.to_string(),
            message: format!(
                "trades.query endTime ({}) must not be before startTime ({})",
                end, start
            ),
        });
    }
    if end - start > MAX_TRADES_QUERY_RANGE_SECS {
        return Err(EwdsErrorPayload {
            code: TIME_RANGE_TOO_LARGE.to_string(),
            message: format!(
                "trades.query range {}s exceeds maximum of {}s (1 day)",
                end - start,
                MAX_TRADES_QUERY_RANGE_SECS
            ),
        });
    }
    Ok(Some((start, end)))
}

#[derive(Deserialize)]
struct IdsQueryPayload {
    #[serde(alias = "offchainId")]
    offchain_id: String,
}

#[derive(Deserialize)]
struct ClearingResultsPayload {
    #[serde(alias = "marketId")]
    market_id: String,
}

#[derive(Deserialize)]
struct MarketsQueryPayload {
    #[serde(alias = "marketId")]
    #[serde(default)]
    market_id: Option<String>,
    #[serde(alias = "communityId")]
    #[serde(default)]
    community_id: Option<String>,
    #[serde(alias = "startTime")]
    #[serde(default)]
    start_time: Option<String>,
    #[serde(alias = "endTime")]
    #[serde(default)]
    end_time: Option<String>,
}

pub async fn start_ewds_request_handler(db: DatabaseWrapper, config: EwdsHandlerConfig) {
    info!(
        gateway = %config.gateway_url,
        request_fqcn = %config.request_fqcn,
        response_fqcn = %config.response_fqcn,
        "Starting EWDS request handler"
    );

    let client = config.ewds_client();
    let mut poller = request_channel_poller(&config);
    let workers = EwdsOperation::ALL
        .into_iter()
        .map(|operation| {
            let requests = poller.route(config.topics.for_operation(operation).request.as_str());
            run_operation_worker(&db, &client, &config, operation, requests)
        })
        .collect::<Vec<_>>();
    join(poller.run(), join_all(workers)).await;
}

/// The poller of the request channel. Every operation's request topic gets its own queue, so a
/// slow request or a response that waits for a retry only holds up requests of its operation.
fn request_channel_poller(config: &EwdsHandlerConfig) -> EwdsChannelPoller {
    EwdsChannelPoller::new(EwdsChannelPollerConfig {
        gateway_base: config.gateway_url.clone(),
        fqcn: config.request_fqcn.clone(),
        client_id: client_id_for_suffix(config.request_client_id.as_str(), "requests"),
        topic_owner: config.topic_owner.clone(),
        batch_size: config.request_batch_size,
        poll_interval_ms: config.poll_interval_ms,
    })
    .with_fallback_router(route_request_by_operation(config.topics.clone()))
}

/// Answers the requests the poller routes to the request topic of `operation`, in order.
async fn run_operation_worker(
    db: &DatabaseWrapper,
    client: &EwdsClient,
    config: &EwdsHandlerConfig,
    operation: EwdsOperation,
    mut requests: UnboundedReceiver<EwdsInboundMessage>,
) {
    let mut seen_request_ids: HashSet<String> = HashSet::new();
    let mut seen_queue: VecDeque<String> = VecDeque::new();

    while let Some(message) = requests.recv().await {
        let max_age = Duration::from_millis(config.request_max_age_ms);
        if let Some(age) = message
            .age_at(SystemTime::now())
            .filter(|age| *age > max_age)
        {
            // Answering would only flood the response topic, which every requester reads.
            info!(
                %operation,
                transaction_id = message.transaction_id.as_deref().unwrap_or_default(),
                age_secs = age.as_secs(),
                "Skipping stale EWDS request"
            );
            continue;
        }
        let envelope = match serde_json::from_str::<EwdsRequestEnvelope>(&message.payload) {
            Ok(envelope) => envelope,
            Err(error) => {
                warn!(%operation, %error, "Skipping malformed EWDS request");
                continue;
            }
        };
        if seen_request_ids.contains(&envelope.request_id) {
            continue;
        }

        let request_id = envelope.request_id.clone();
        // Every log line written while answering carries the request's ID and operation.
        let span = info_span!("ewds_request", %request_id, operation = %envelope.operation);
        async {
            let result = if envelope.operation == operation {
                handle_request(db, client, config, envelope).await
            } else {
                reject_mismatched_operation(client, config, operation, envelope).await
            };
            // The gateway has already acknowledged the request, so a failure cannot be
            // redelivered; log it and carry on with the next request.
            if let Err(error) = result {
                error!(error = %format_args!("{error:#}"), "EWDS request handling failed");
            }
        }
        .instrument(span)
        .await;
        remember_id(&request_id, &mut seen_request_ids, &mut seen_queue);
    }
}

pub const OPERATION_TOPIC_MISMATCH: &str = "OPERATION_TOPIC_MISMATCH";

/// Answers a request whose `operation` doesn't belong to the topic it arrived on with an error,
/// on the response topic of its `operation`, where the requester waits.
async fn reject_mismatched_operation(
    client: &EwdsClient,
    config: &EwdsHandlerConfig,
    topic_operation: EwdsOperation,
    envelope: EwdsRequestEnvelope,
) -> Result<()> {
    let topics = config.topics.for_operation(envelope.operation);
    let error = EwdsErrorPayload {
        code: OPERATION_TOPIC_MISMATCH.to_string(),
        message: format!(
            "{} requests must be sent on topic '{}', not '{}'",
            envelope.operation,
            topics.request,
            config.topics.for_operation(topic_operation).request
        ),
    };
    send_response(
        client,
        config,
        envelope.request_id,
        &topics.response,
        Err(error),
    )
    .await
}

/// What a request is answered with: its records, or the error that rejects it.
type Response = std::result::Result<Vec<Value>, EwdsErrorPayload>;

/// Answers a request on the response topic of its operation.
pub async fn handle_request(
    db: &DatabaseWrapper,
    client: &EwdsClient,
    config: &EwdsHandlerConfig,
    envelope: EwdsRequestEnvelope,
) -> Result<()> {
    let EwdsRequestEnvelope {
        request_id,
        operation,
        payload,
    } = envelope;
    info!("Handling EWDS request");

    let response = match operation {
        EwdsOperation::OrdersQuery => orders_query(db, payload).await,
        EwdsOperation::TradesQuery => trades_query(db, payload).await,
        EwdsOperation::MeasurementsQuery => measurements_query(db, payload).await,
        EwdsOperation::CommunitiesQuery => communities_query(db).await,
        EwdsOperation::IdsQuery => ids_query(db, payload).await,
        EwdsOperation::ClearingResultsQuery => clearing_results_query(db, payload).await,
        EwdsOperation::MarketsQuery => markets_query(db, payload).await,
        EwdsOperation::FacilitiesQuery => facilities_query(db).await,
        EwdsOperation::FacilitiesValidate => facilities_validate(db, payload).await,
        EwdsOperation::GuaranteesOfOriginQuery => guarantees_of_origin_query(db, payload).await,
    };
    let response_topic = &config.topics.for_operation(operation).response;
    send_response(client, config, request_id, response_topic, response).await
}

async fn orders_query(db: &DatabaseWrapper, payload: Value) -> Response {
    let query: OrdersQueryPayload = serde_json::from_value(payload).map_err(invalid_request)?;
    let market_id = query
        .market_id
        .as_deref()
        .map(normalize_uuid)
        .transpose()
        .map_err(invalid_request)?;
    let start_time = opt_rfc3339_to_epoch(query.start_time.as_deref()).map_err(invalid_request)?;
    let end_time = opt_rfc3339_to_epoch(query.end_time.as_deref()).map_err(invalid_request)?;

    let orders = db
        .orders()
        .filter_orders(market_id, start_time, end_time)
        .await
        .map_err(storage_unavailable)?;
    records(orders.into_iter().map(EwdsOrderDto::from))
}

async fn trades_query(db: &DatabaseWrapper, payload: Value) -> Response {
    let query: TradesQueryPayload = serde_json::from_value(payload).map_err(invalid_request)?;
    let market_id = query
        .market_id
        .as_deref()
        .map(normalize_uuid)
        .transpose()
        .map_err(invalid_request)?;
    let start_time = opt_rfc3339_to_epoch(query.start_time.as_deref()).map_err(invalid_request)?;
    let end_time = opt_rfc3339_to_epoch(query.end_time.as_deref()).map_err(invalid_request)?;
    let range = validate_trades_query_range(market_id.as_deref(), start_time, end_time)?;

    let trades = db
        .trades()
        .filter_trades(
            market_id,
            range.map(|(start, _)| start),
            range.map(|(_, end)| end),
        )
        .await
        .map_err(storage_unavailable)?;
    records(trades.into_iter().map(EwdsTradeDto::from))
}

async fn measurements_query(db: &DatabaseWrapper, payload: Value) -> Response {
    let query: TimeRangePayload = serde_json::from_value(payload).map_err(invalid_request)?;
    let start_time = opt_rfc3339_to_epoch(query.start_time.as_deref()).map_err(invalid_request)?;
    let end_time = opt_rfc3339_to_epoch(query.end_time.as_deref()).map_err(invalid_request)?;
    let facility_id = query
        .facility_id
        .as_deref()
        .map(validate_offchain_actor_id)
        .transpose()
        .map_err(invalid_request)?;
    let community_uuid = query
        .community_uuid
        .as_deref()
        .map(validate_offchain_actor_id)
        .transpose()
        .map_err(invalid_request)?;

    let measurements = fetch_measurements_from_timeseries(db, start_time, end_time)
        .await
        .map_err(storage_unavailable)?;
    records(
        measurements
            .into_iter()
            .filter(|measurement| {
                facility_id
                    .as_ref()
                    .is_none_or(|id| measurement.facility_id == *id)
            })
            .filter(|measurement| {
                community_uuid
                    .as_ref()
                    .is_none_or(|id| measurement.community_uuid == *id)
            })
            .map(EwdsMeasurementDto::from),
    )
}

async fn communities_query(db: &DatabaseWrapper) -> Response {
    let communities = db.communities().get().await.map_err(storage_unavailable)?;
    records(communities.into_iter().map(EwdsCommunityDto::from))
}

async fn ids_query(db: &DatabaseWrapper, payload: Value) -> Response {
    let query: IdsQueryPayload = serde_json::from_value(payload).map_err(invalid_request)?;
    let offchain_id = validate_offchain_actor_id(&query.offchain_id).map_err(invalid_request)?;

    let mapping = db
        .ids()
        .get_or_create(offchain_id)
        .await
        .map_err(storage_unavailable)?;
    records([mapping])
}

async fn clearing_results_query(db: &DatabaseWrapper, payload: Value) -> Response {
    let query: ClearingResultsPayload = serde_json::from_value(payload).map_err(invalid_request)?;
    let market_id = normalize_uuid(&query.market_id).map_err(invalid_request)?;

    let clearing_results = db
        .clearing_results()
        .get_by_market(&market_id)
        .await
        .map_err(storage_unavailable)?;
    records(
        clearing_results
            .into_iter()
            .map(EwdsClearingResultDto::from),
    )
}

async fn markets_query(db: &DatabaseWrapper, payload: Value) -> Response {
    let query: MarketsQueryPayload = serde_json::from_value(payload).map_err(invalid_request)?;
    // Market IDs use canonical hex storage; community IDs are opaque
    // and case-sensitive. Times are stored as zero-padded epoch seconds.
    let market_id = query
        .market_id
        .as_deref()
        .map(normalize_uuid)
        .transpose()
        .map_err(invalid_request)?;
    let community_id = query
        .community_id
        .as_deref()
        .map(validate_offchain_actor_id)
        .transpose()
        .map_err(invalid_request)?;
    let stored_time = |time: Option<String>| {
        opt_rfc3339_to_epoch(time.as_deref())
            .map(|epoch| epoch.map(timestamp_to_string_with_padding))
            .map_err(invalid_request)
    };
    let start_time = stored_time(query.start_time)?;
    let end_time = stored_time(query.end_time)?;

    let markets = db
        .markets()
        .filter(market_id, community_id, start_time, end_time)
        .await
        .and_then(|rows| {
            rows.into_iter()
                .map(EwdsMarketDto::try_from)
                .collect::<Result<Vec<_>>>()
        })
        .map_err(storage_unavailable)?;
    records(markets)
}

async fn facilities_query(db: &DatabaseWrapper) -> Response {
    let facilities = db.facilities().get().await.map_err(storage_unavailable)?;
    records(facilities.into_iter().map(EwdsFacilityDto::from))
}

async fn facilities_validate(db: &DatabaseWrapper, payload: Value) -> Response {
    let requests: Vec<FacilityMarketValidationRequest> =
        serde_json::from_value(payload).map_err(invalid_request)?;

    let results = db
        .validate_facility_markets(&requests)
        .await
        .map_err(|error| {
            error!(error = %format_args!("{error:#}"), "EWDS facility validation failed");
            EwdsErrorPayload {
                code: "VALIDATION_UNAVAILABLE".to_string(),
                message: "Facility validation could not read the stored topology".to_string(),
            }
        })?;
    records(results)
}

async fn guarantees_of_origin_query(db: &DatabaseWrapper, payload: Value) -> Response {
    let query: GuaranteesOfOriginPayload =
        serde_json::from_value(payload).map_err(invalid_request)?;
    let window = validate_window(query.start_time, query.end_time).map_err(invalid_request)?;

    let certificates = guarantees_of_origin(db, window)
        .await
        .map_err(storage_unavailable)?;
    records(certificates)
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
            start_time.map(timestamp_to_string_with_padding),
            end_time.map(timestamp_to_string_with_padding),
        )
        .await?;

    Ok(values
        .into_iter()
        .filter_map(|value| {
            let point = points_by_id.get(&value.measurement_point)?;
            let time_slot = parse_timeseries_timestamp(value.timestamp.as_str())?;
            Some(MeasurementSchema {
                facility_id: point.asset_name.clone(),
                community_uuid: point.datasource_name.clone().unwrap_or_default(),
                time_slot,
                creation_time: time_slot,
                energy_kwh: point.signed_energy_kwh(value.value),
            })
        })
        .collect())
}

fn parse_timeseries_timestamp(timestamp: &str) -> Option<u64> {
    timestamp.parse::<u64>().ok()
}

/// The records of a successful response.
fn records<T: Serialize>(rows: impl IntoIterator<Item = T>) -> Response {
    rows.into_iter()
        .map(serde_json::to_value)
        .collect::<serde_json::Result<_>>()
        .map_err(|error| storage_unavailable(error.into()))
}

/// Rejects a request whose payload is invalid.
fn invalid_request(error: impl Display) -> EwdsErrorPayload {
    EwdsErrorPayload {
        code: INVALID_REQUEST_ERROR_CODE.to_string(),
        message: error.to_string(),
    }
}

/// Rejects a request whose records cannot be read. The cause is only logged.
fn storage_unavailable(error: anyhow::Error) -> EwdsErrorPayload {
    error!(error = %format_args!("{error:#}"), "EWDS storage query failed");
    EwdsErrorPayload {
        code: "STORAGE_UNAVAILABLE".to_string(),
        message: "Unable to read the requested records".to_string(),
    }
}

/// Publishes the response to a request on `topic_name`. Polling has acknowledged the request,
/// so it cannot be redelivered: a rejected request is answered too, with `success: false`,
/// rather than left for its sender to time out on.
async fn send_response(
    client: &EwdsClient,
    config: &EwdsHandlerConfig,
    request_id: String,
    topic_name: &str,
    response: Response,
) -> Result<()> {
    let envelope = match response {
        Ok(data) => {
            info!(topic_name, records = data.len(), "Publishing EWDS response");
            EwdsResponseEnvelope {
                request_id: request_id.clone(),
                success: true,
                data,
                error: None,
            }
        }
        Err(error) => {
            warn!(
                topic_name,
                code = %error.code,
                reason = %error.message,
                "Rejecting EWDS request"
            );
            EwdsResponseEnvelope {
                request_id: request_id.clone(),
                success: false,
                data: Vec::new(),
                error: Some(error),
            }
        }
    };

    client
        .publish(
            &config.response_fqcn,
            topic_name,
            &request_id,
            serde_json::to_string(&envelope)?,
        )
        .await
}
