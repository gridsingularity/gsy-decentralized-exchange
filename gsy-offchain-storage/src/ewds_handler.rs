use crate::certificates::query::{guarantees_of_origin, validate_window};
use crate::db::DatabaseWrapper;
use crate::ewds_event_handler::EwdsEventPublisher;
use anyhow::{anyhow, Result};
use futures::future::join_all;
use primitives::db_api_schema::profiles::{MeasurementPointType, MeasurementSchema};
use primitives::db_api_schema::trades::{DbTradeSchema, TradeStatus};
use primitives::ewds::dto::{
    parse_market_id, EwdsClearingResultDto, EwdsCommunityDto, EwdsErrorPayload, EwdsInboundMessage,
    EwdsMarketDto, EwdsMeasurementDto, EwdsOrderDto, EwdsRequestEnvelope, EwdsResponseEnvelope,
    EwdsSendMessageDto, EwdsTradeDto,
};
use primitives::ewds::{
    client_id_for_suffix, env_var, ewds_rate_limit_backoff_ms, format_response_body,
    is_rate_limited_response, is_transient_gateway_response, next_poll_delay_ms,
    parse_gateway_delivery_summary, remember_id, EwdsEventTopicConfig, EwdsEventType,
    EwdsOperation, EwdsTopicConfig,
};
use primitives::utils::{
    bytes16_to_hex, normalize_community_id, opt_rfc3339_to_epoch, timestamp_to_string_with_padding,
};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};
use std::time::Instant;
use tokio::time::{sleep, Duration};
use tracing::{error, info, warn};

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
            .unwrap_or(60_000);

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
        }
    }

    /// The EWDS topic an event of the given type is published on.
    pub fn event_topic(&self, event_type: EwdsEventType) -> &str {
        self.event_topics.for_event_type(event_type)
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
    #[serde(alias = "areaUuid")]
    #[serde(default)]
    facility_id: Option<String>,
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
    if !config.enabled {
        info!("EWDS request handler disabled");
        return;
    }

    info!(
        "Starting EWDS request handler (gateway={}, request_fqcn={}, response_fqcn={})",
        config.gateway_url, config.request_fqcn, config.response_fqcn
    );

    let workers = EwdsOperation::ALL
        .into_iter()
        .map(|operation| run_topic_worker(db.clone(), Client::new(), config.clone(), operation));
    join_all(workers).await;
}

async fn run_topic_worker(
    db: DatabaseWrapper,
    client: Client,
    config: EwdsHandlerConfig,
    operation: EwdsOperation,
) {
    let mut seen_request_ids: HashSet<String> = HashSet::new();
    let mut seen_queue: VecDeque<String> = VecDeque::new();
    let mut rate_limit_attempt = 0u32;

    loop {
        let result = process_topic_batch(
            &db,
            &client,
            &config,
            operation,
            &mut seen_request_ids,
            &mut seen_queue,
        )
        .await;
        if let Err(error) = &result {
            warn!(
                "EWDS {} worker batch processing failed: {}",
                operation, error
            );
        }

        let delay_ms =
            next_poll_delay_ms(&result, config.poll_interval_ms, &mut rate_limit_attempt);
        sleep(Duration::from_millis(delay_ms)).await;
    }
}

async fn process_topic_batch(
    db: &DatabaseWrapper,
    client: &Client,
    config: &EwdsHandlerConfig,
    operation: EwdsOperation,
    seen_request_ids: &mut HashSet<String>,
    seen_queue: &mut VecDeque<String>,
) -> Result<()> {
    let amount = config.request_batch_size.to_string();
    let topic_name = config.topics.for_operation(operation).request.as_str();
    let messages = poll_messages(client, config, topic_name, amount.as_str()).await?;

    for message in messages {
        let parsed = serde_json::from_str::<EwdsRequestEnvelope>(&message.payload);
        let envelope = match parsed {
            Ok(value) => value,
            Err(_) => continue,
        };

        if seen_request_ids.contains(&envelope.request_id) {
            continue;
        }

        let request_id = envelope.request_id.clone();
        if let Err(error) = handle_request(db, client, config, envelope).await {
            error!(
                "EWDS request handling failed (request_id={}): {}",
                request_id, error
            );
            return Err(error);
        }

        remember_id(&request_id, seen_request_ids, seen_queue);
    }

    Ok(())
}

async fn poll_messages(
    client: &Client,
    config: &EwdsHandlerConfig,
    topic_name: &str,
    amount: &str,
) -> Result<Vec<EwdsInboundMessage>> {
    let get_url = format!(
        "{}/api/v2/messages",
        config.gateway_url.trim_end_matches('/')
    );
    let fqcn = config.request_fqcn.as_str();
    let client_id = client_id_for_suffix(config.request_client_id.as_str(), topic_name);
    let response = client
        .get(get_url.as_str())
        .query(&[
            ("fqcn", fqcn),
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
            "EWDS poll failed for fqcn='{}', topic='{}': HTTP {}{}",
            fqcn,
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

pub async fn handle_request(
    db: &DatabaseWrapper,
    client: &Client,
    config: &EwdsHandlerConfig,
    envelope: EwdsRequestEnvelope,
) -> Result<()> {
    let response_topic = config
        .topics
        .for_operation(envelope.operation)
        .response
        .clone();

    match envelope.operation {
        EwdsOperation::OrdersQuery => {
            let payload = serde_json::from_value::<OrdersQueryPayload>(envelope.payload.clone())
                .map_err(|e| anyhow!("orders.query payload parse error: {}", e))?;
            let request_id = envelope.request_id;

            info!(
                "Handling EWDS orders.query request (request_id={})",
                request_id
            );

            let data = db
                .orders()
                .filter_orders(
                    payload.market_id,
                    opt_rfc3339_to_epoch(payload.start_time.as_deref())?,
                    opt_rfc3339_to_epoch(payload.end_time.as_deref())?,
                )
                .await?
                .into_iter()
                .map(EwdsOrderDto::from)
                .collect::<Vec<_>>();
            info!(
                "Publishing EWDS orders.query response (request_id={}, orders={})",
                request_id,
                data.len()
            );

            send_success_response(client, config, request_id, response_topic.as_str(), data).await
        }
        EwdsOperation::TradesQuery => {
            let payload = serde_json::from_value::<TradesQueryPayload>(envelope.payload.clone())
                .map_err(|e| anyhow!("trades.query payload parse error: {}", e))?;
            let request_id = envelope.request_id;

            info!(
                "Handling EWDS trades.query request (request_id={})",
                request_id
            );

            let payload_start_time = opt_rfc3339_to_epoch(payload.start_time.as_deref())?;
            let payload_end_time = opt_rfc3339_to_epoch(payload.end_time.as_deref())?;
            let range = match validate_trades_query_range(
                payload.market_id.as_deref(),
                payload_start_time,
                payload_end_time,
            ) {
                Ok(range) => range,
                Err(error) => {
                    warn!(
                        "Rejecting EWDS trades.query request (request_id={}): {}: {}",
                        request_id, error.code, error.message
                    );
                    return send_error_response(
                        client,
                        config,
                        request_id,
                        response_topic.as_str(),
                        error,
                    )
                    .await;
                }
            };

            let data = db
                .trades()
                .filter_trades(
                    payload.market_id,
                    range.map(|(start, _)| start),
                    range.map(|(_, end)| end),
                )
                .await?
                .into_iter()
                .map(EwdsTradeDto::from)
                .collect::<Vec<_>>();
            info!(
                "Publishing EWDS trades.query response (request_id={}, trades={})",
                request_id,
                data.len()
            );

            send_success_response(client, config, request_id, response_topic.as_str(), data).await
        }
        EwdsOperation::TradesImport => {
            // Convert the whole batch before writing, like POST /trades.
            let trades = serde_json::from_value::<Vec<EwdsTradeDto>>(envelope.payload)
                .map_err(anyhow::Error::from)
                .and_then(|trades| {
                    if trades.is_empty() {
                        return Err(anyhow!("trade batch must not be empty"));
                    }
                    trades
                        .into_iter()
                        .map(DbTradeSchema::try_from)
                        .collect::<Result<Vec<_>>>()
                });
            let trades = match trades {
                Ok(trades) => trades,
                Err(error) => {
                    return send_error_response(
                        client,
                        config,
                        envelope.request_id,
                        &response_topic,
                        EwdsErrorPayload {
                            code: INVALID_REQUEST_ERROR_CODE.to_string(),
                            message: format!("trades.import payload error: {}", error),
                        },
                    )
                    .await;
                }
            };
            let trade_ids = trades
                .iter()
                .map(|trade| trade.trade_uuid.clone())
                .collect();
            let matched_ids: Vec<_> = trades
                .iter()
                .filter(|trade| trade.status == TradeStatus::Matched)
                .map(|trade| trade.trade_uuid.clone())
                .collect();
            match db.trades().insert_trades(trades).await {
                Ok(_) => {
                    if config.enabled {
                        EwdsEventPublisher::new(config.clone())
                            .publish_stored_trades(db, &matched_ids)
                            .await?;
                    }
                    send_success_response(
                        client,
                        config,
                        envelope.request_id,
                        &response_topic,
                        trade_ids,
                    )
                    .await
                }
                Err(error) => {
                    warn!(
                        "EWDS trades.import failed (request_id={}): {}",
                        envelope.request_id, error
                    );
                    send_error_response(
                        client,
                        config,
                        envelope.request_id,
                        &response_topic,
                        EwdsErrorPayload {
                            code: "trade_import_failed".to_string(),
                            message: format!("trades.import failed: {}", error),
                        },
                    )
                    .await
                }
            }
        }
        EwdsOperation::MeasurementsQuery => {
            let payload = serde_json::from_value::<TimeRangePayload>(envelope.payload.clone())
                .map_err(|e| anyhow!("measurements.query payload parse error: {}", e))?;
            let request_id = envelope.request_id;

            info!(
                "Handling EWDS measurements.query request (request_id={})",
                request_id
            );

            let data = fetch_measurements_from_timeseries(
                db,
                opt_rfc3339_to_epoch(payload.start_time.as_deref())?,
                opt_rfc3339_to_epoch(payload.end_time.as_deref())?,
            )
            .await?
            .into_iter()
            .filter(|measurement| match payload.facility_id.as_ref() {
                Some(facility_id) => measurement.facility_id == *facility_id,
                None => true,
            })
            .map(EwdsMeasurementDto::from)
            .collect::<Vec<_>>();
            info!(
                "Publishing EWDS measurements.query response (request_id={}, measurements={})",
                request_id,
                data.len()
            );

            send_success_response(client, config, request_id, response_topic.as_str(), data).await
        }
        EwdsOperation::CommunitiesQuery => {
            let request_id = envelope.request_id;

            info!(
                "Handling EWDS communities.query request (request_id={})",
                request_id
            );

            let data = db
                .communities()
                .get_all()
                .await?
                .into_iter()
                .map(EwdsCommunityDto::from)
                .collect::<Vec<_>>();
            info!(
                "Publishing EWDS communities.query response (request_id={}, communities={})",
                request_id,
                data.len()
            );

            send_success_response(client, config, request_id, response_topic.as_str(), data).await
        }
        EwdsOperation::IdsQuery => {
            let payload = serde_json::from_value::<IdsQueryPayload>(envelope.payload.clone())
                .map_err(|e| anyhow!("id.query payload parse error: {}", e))?;
            let request_id = envelope.request_id;

            info!(
                "Handling EWDS ids.query request (request_id={})",
                request_id
            );

            let data = db.ids().get_or_create(payload.offchain_id).await?;
            send_success_response(
                client,
                config,
                request_id,
                response_topic.as_str(),
                vec![data],
            )
            .await
        }
        EwdsOperation::ClearingResultsQuery => {
            let payload =
                serde_json::from_value::<ClearingResultsPayload>(envelope.payload.clone())
                    .map_err(|e| anyhow!("clearing_results.query payload parse error: {}", e))?;
            let request_id = envelope.request_id;
            let results = db
                .clearing_results()
                .get_by_market(&payload.market_id)
                .await
                .map_err(|e| anyhow!("clearing_results.query DB error: {}", e))?;

            let data = results
                .into_iter()
                .map(EwdsClearingResultDto::from)
                .collect::<Vec<_>>();
            info!(
                "Publishing EWDS clearing_results.query response (request_id={}, results={})",
                request_id,
                data.len()
            );

            send_success_response(client, config, request_id, response_topic.as_str(), data).await
        }
        EwdsOperation::MarketsQuery => {
            let payload = serde_json::from_value::<MarketsQueryPayload>(envelope.payload.clone())
                .map_err(|e| anyhow!("markets.query payload parse error: {}", e))?;
            let request_id = envelope.request_id;

            info!(
                "Handling EWDS markets.query request (request_id={})",
                request_id
            );

            // The request uses the forms of the response (UUID or hex market
            // and community IDs, RFC 3339 times); markets are stored with a
            // `0x…` ID, a lowercase UUID community ID and zero-padded epoch
            // seconds.
            let market_id = payload
                .market_id
                .map(|market_id| parse_market_id(&market_id).map(bytes16_to_hex))
                .transpose()
                .map_err(|e| anyhow!("markets.query payload parse error: {}", e))?;
            let community_id = payload
                .community_id
                .as_deref()
                .map(normalize_community_id)
                .transpose()
                .map_err(|e| anyhow!("markets.query payload parse error: {}", e))?;
            let stored_time = |time: Option<String>| {
                opt_rfc3339_to_epoch(time.as_deref())
                    .map(|epoch| epoch.map(timestamp_to_string_with_padding))
                    .map_err(|e| anyhow!("markets.query payload parse error: {}", e))
            };
            let data = db
                .markets()
                .filter(
                    market_id,
                    community_id,
                    stored_time(payload.start_time)?,
                    stored_time(payload.end_time)?,
                )
                .await
                .map_err(|e| anyhow!("markets.query DB error: {}", e))?
                .into_iter()
                .map(EwdsMarketDto::try_from)
                .collect::<Result<Vec<_>>>()
                .map_err(|e| anyhow!("markets.query invalid stored market: {}", e))?;
            info!(
                "Publishing EWDS markets.query response (request_id={}, markets={})",
                request_id,
                data.len()
            );

            send_success_response(client, config, request_id, response_topic.as_str(), data).await
        }
        EwdsOperation::FacilitiesQuery => {
            let request_id = envelope.request_id;
            let data = db.facilities().get_all().await?;
            info!(
                "Publishing EWDS facilities.query response (request_id={}, facilities={})",
                request_id,
                data.len()
            );

            send_success_response(client, config, request_id, response_topic.as_str(), data).await
        }
        EwdsOperation::GuaranteesOfOriginQuery => {
            let request_id = envelope.request_id;

            info!(
                "Handling EWDS guarantees_of_origin.query request (request_id={})",
                request_id
            );

            // An invalid request is answered with `success: false` rather than returned as an
            // error: an erroring worker re-polls the same message forever.
            let window = match serde_json::from_value::<GuaranteesOfOriginPayload>(envelope.payload)
                .map_err(|e| format!("guarantees_of_origin.query payload parse error: {}", e))
                .and_then(|payload| validate_window(payload.start_time, payload.end_time))
            {
                Ok(window) => window,
                Err(reason) => {
                    warn!(
                        "Rejecting EWDS guarantees_of_origin.query request (request_id={}): {}",
                        request_id, reason
                    );
                    return send_error_response(
                        client,
                        config,
                        request_id,
                        response_topic.as_str(),
                        EwdsErrorPayload {
                            code: INVALID_REQUEST_ERROR_CODE.to_string(),
                            message: reason,
                        },
                    )
                    .await;
                }
            };

            let data = guarantees_of_origin(db, window).await?;
            info!(
                "Publishing EWDS guarantees_of_origin.query response (request_id={}, records={})",
                request_id,
                data.len()
            );

            send_success_response(client, config, request_id, response_topic.as_str(), data).await
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
                energy_kwh: value.value,
            })
        })
        .collect())
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
    topic_name: &str,
    error: EwdsErrorPayload,
) -> Result<()> {
    let payload = EwdsResponseEnvelope::<serde_json::Value> {
        request_id: request_id.clone(),
        success: false,
        data: Vec::new(),
        error: Some(error),
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

pub(crate) async fn send_message_with_fqcn(
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

    let started = Instant::now();
    let mut delivery_attempt = 0u32;
    loop {
        if started.elapsed() > Duration::from_millis(config.response_send_timeout_ms) {
            return Err(anyhow!(
                "EWDS response send timed out for fqcn='{}', topic='{}'",
                request_body.fqcn,
                request_body.topic_name
            ));
        }

        let response = client
            .post(post_url.as_str())
            .json(&request_body)
            .send()
            .await?;
        let status = response.status();
        let response_body = response.text().await.unwrap_or_default();
        if status.is_success() {
            let delivery = parse_gateway_delivery_summary(response_body.as_str())?;
            if delivery.sent > 0 {
                return Ok(());
            }

            let delay_ms = ewds_rate_limit_backoff_ms(delivery_attempt);
            warn!(
                "EWDS gateway accepted response for topic '{}' but delivered it to no recipients (failed={}, total={}); retrying in {} ms",
                request_body.topic_name, delivery.failed, delivery.total, delay_ms
            );
            delivery_attempt = delivery_attempt.saturating_add(1);
            sleep(Duration::from_millis(delay_ms)).await;
            continue;
        }

        if is_rate_limited_response(status, response_body.as_str()) {
            let delay_ms = ewds_rate_limit_backoff_ms(delivery_attempt);
            warn!(
                "EWDS rate limit while publishing response for topic '{}'; retrying in {} ms",
                request_body.topic_name, delay_ms
            );
            delivery_attempt = delivery_attempt.saturating_add(1);
            sleep(Duration::from_millis(delay_ms)).await;
            continue;
        }

        if is_transient_gateway_response(status, response_body.as_str()) {
            let delay_ms = ewds_rate_limit_backoff_ms(delivery_attempt);
            warn!(
                "EWDS transient gateway error while publishing response for topic '{}'; retrying in {} ms",
                request_body.topic_name, delay_ms
            );
            delivery_attempt = delivery_attempt.saturating_add(1);
            sleep(Duration::from_millis(delay_ms)).await;
            continue;
        }

        return Err(anyhow!(
            "EWDS message send failed for fqcn='{}', topic='{}': HTTP {}{}",
            request_body.fqcn,
            request_body.topic_name,
            status,
            format_response_body(&response_body)
        ));
    }
}
