use crate::db::measurements_service::insert_measurements;
use crate::db::DatabaseWrapper;
use crate::ewds_handler::{send_message_with_fqcn, EwdsHandlerConfig};
use anyhow::{anyhow, Context, Result};
use futures::future::join_all;
use primitives::db_api_schema::{
    grid_topology::{EnergyCommunitySchema, FacilitySchema, SiteSchema},
    orders::DbOrderSchema,
    profiles::MeasurementSchema,
    trades::{ClearingResultSchema, DbTradeSchema},
};
use primitives::ewds::dto::{
    EwdsClearingResultDto, EwdsCommunityDto, EwdsEventEnvelope, EwdsMarketDto, EwdsMeasurementDto,
    EwdsOrderDto, EwdsTradeDto,
};
use primitives::ewds::{invalid_event, parse_batch, EwdsClient, EwdsClientConfig, EwdsEventType};
use primitives::utils::epoch_to_rfc3339;
use reqwest::Client;
use serde::Serialize;
use serde_json::Value;
use std::future::Future;
use tokio::task::JoinHandle;
use tokio::time::{sleep, Duration};
use tracing::{error, info, warn};
use uuid::Uuid;

/// The events other systems send to GSY. Each is polled on its own topic.
const SUBSCRIBED_EVENT_TYPES: [EwdsEventType; 4] = [
    EwdsEventType::Measurements,
    EwdsEventType::Facility,
    EwdsEventType::Site,
    EwdsEventType::Community,
];

const DB_WRITE_ATTEMPTS: u32 = 3;
const DB_RETRY_DELAY_MS: u64 = 500;

/// Publishes domain events of the off-chain storage on the EWDS events channel.
///
/// Every event is sent in its own background task, so callers such as the EVM listener are not
/// stalled by gateway retries. The returned handle only needs to be awaited when the caller has
/// to know that sending has finished, e.g. in tests.
#[derive(Clone)]
pub struct EwdsEventPublisher {
    client: Client,
    config: EwdsHandlerConfig,
}

impl EwdsEventPublisher {
    pub fn new(config: EwdsHandlerConfig) -> Self {
        Self {
            client: Client::new(),
            config,
        }
    }

    pub fn publish_order_status_event(&self, orders: Vec<DbOrderSchema>) -> JoinHandle<()> {
        let occurred_at = orders
            .iter()
            .filter_map(|order| order.status_updated_at)
            .max()
            .unwrap_or_default();
        self.spawn_event(
            EwdsEventType::OrderStatus,
            occurred_at,
            orders
                .into_iter()
                .map(EwdsOrderDto::from)
                .collect::<Vec<_>>(),
        )
    }

    pub fn publish_trade_event(&self, trades: Vec<DbTradeSchema>) -> JoinHandle<()> {
        let occurred_at = trades
            .iter()
            .map(|trade| trade.status_updated_at.unwrap_or(trade.creation_time))
            .max()
            .unwrap_or_default();
        self.spawn_event(
            EwdsEventType::Trade,
            occurred_at,
            trades
                .into_iter()
                .map(EwdsTradeDto::from)
                .collect::<Vec<_>>(),
        )
    }

    /// Re-read persisted statuses so replayed events cannot publish stale payloads.
    pub async fn publish_stored_trades(
        &self,
        db: &DatabaseWrapper,
        trade_ids: &[String],
    ) -> Result<()> {
        if !self.config.enabled || trade_ids.is_empty() {
            return Ok(());
        }
        let mut trades = Vec::with_capacity(trade_ids.len());
        for trade_id in trade_ids {
            let trade = db
                .trades()
                .find_one(mongodb::bson::doc! {"trade_uuid": trade_id})
                .await?
                .with_context(|| format!("Missing trade {} for status notification", trade_id))?;
            trades.push(trade);
        }
        self.publish_trade_event(trades);
        Ok(())
    }

    pub fn publish_clearing_result_event(
        &self,
        clearing_results: Vec<ClearingResultSchema>,
    ) -> JoinHandle<()> {
        let occurred_at = clearing_results
            .iter()
            .map(|clearing_result| clearing_result.clearing_time)
            .max()
            .unwrap_or_default();
        self.spawn_event(
            EwdsEventType::ClearingResult,
            occurred_at,
            clearing_results
                .into_iter()
                .map(EwdsClearingResultDto::from)
                .collect::<Vec<_>>(),
        )
    }

    pub fn publish_market_event(
        &self,
        markets: Vec<EwdsMarketDto>,
        occurred_at: u64,
    ) -> JoinHandle<()> {
        self.spawn_event(EwdsEventType::Market, occurred_at, markets)
    }

    fn spawn_event<T: Serialize + Send + 'static>(
        &self,
        event_type: EwdsEventType,
        occurred_at: u64,
        data: T,
    ) -> JoinHandle<()> {
        let event_id = Uuid::new_v4().to_string();
        let publisher = self.clone();
        tokio::spawn(async move {
            if let Err(e) = publisher
                .send_event(event_type, event_id.clone(), occurred_at, data)
                .await
            {
                error!(
                    "Failed to publish EWDS {} event {}: {:?}",
                    event_type, event_id, e
                );
            }
        })
    }

    async fn send_event<T: Serialize>(
        &self,
        event_type: EwdsEventType,
        event_id: String,
        occurred_at: u64,
        data: T,
    ) -> Result<()> {
        let envelope = EwdsEventEnvelope {
            event_id: event_id.clone(),
            event_type,
            occurred_at: epoch_to_rfc3339(occurred_at),
            data,
        };

        send_message_with_fqcn(
            &self.client,
            &self.config,
            self.config.event_publish_fqcn.clone(),
            event_id,
            self.config.event_topic(event_type).to_string(),
            serde_json::to_string(&envelope)?,
        )
        .await
    }
}

pub async fn start_ewds_event_subscriber(db: DatabaseWrapper, config: EwdsHandlerConfig) {
    if !config.enabled {
        info!("EWDS event subscriber disabled");
        return;
    }

    info!(
        "Starting EWDS event subscriber (gateway={}, fqcn={})",
        config.gateway_url, config.event_subscribe_fqcn
    );

    let client = event_client(&config);
    let db = &db;
    let workers = SUBSCRIBED_EVENT_TYPES.into_iter().map(|event_type| {
        client.run_event_worker(event_type, move |envelope| handle_event(db, envelope))
    });
    join_all(workers).await;
}

/// The client the subscriber polls events with. It only polls, so the query settings keep their
/// defaults.
fn event_client(config: &EwdsHandlerConfig) -> EwdsClient {
    EwdsClient::new(EwdsClientConfig {
        gateway_base: config.gateway_url.clone(),
        topic_owner: config.topic_owner.clone(),
        topic_version: config.topic_version.clone(),
        consumer_client_id: config.request_client_id.clone(),
        event_poll_interval_ms: config.event_poll_interval_ms,
        event_publish_fqcn: config.event_publish_fqcn.clone(),
        event_subscribe_fqcn: config.event_subscribe_fqcn.clone(),
        event_batch_size: config.request_batch_size,
        event_topics: config.event_topics.clone(),
        ..EwdsClientConfig::from_env(
            "EWDS_REQUEST_CLIENT_ID",
            "gsyoffchainstorage",
            config.response_send_timeout_ms,
        )
    })
}

pub async fn handle_event(db: &DatabaseWrapper, envelope: EwdsEventEnvelope<Value>) -> Result<()> {
    match envelope.event_type {
        EwdsEventType::Measurements => {
            let measurements = parse_batch(envelope.data, |measurement: EwdsMeasurementDto| {
                MeasurementSchema::try_from(measurement)
            })?;
            retry_db_write(|| insert_measurements(db, &measurements)).await?;
        }
        EwdsEventType::Facility => {
            let facilities = parse_batch(envelope.data, |facility: FacilitySchema| Ok(facility))?;
            db.facilities().check_for_duplicates(&facilities).await?;
            for facility in facilities {
                retry_db_write(|| {
                    let facility = facility.clone();
                    async move { db.facilities().upsert(facility).await }
                })
                .await?;
            }
        }
        EwdsEventType::Site => {
            let sites = parse_batch(envelope.data, |site: SiteSchema| Ok(site))?;
            for site in sites {
                retry_db_write(|| {
                    let site = site.clone();
                    async move { db.sites().upsert(site).await }
                })
                .await?;
            }
        }
        EwdsEventType::Community => {
            let communities = parse_batch(envelope.data, |community: EwdsCommunityDto| {
                let mut community = EnergyCommunitySchema::from(community);
                community.community_id =
                    primitives::utils::normalize_community_id(&community.community_id)?;
                Ok(community)
            })?;
            db.communities().check_for_duplicates(&communities).await?;
            for community in communities {
                retry_db_write(|| {
                    let community = community.clone();
                    async move { db.communities().upsert(community).await }
                })
                .await?;
            }
        }
        other => {
            return Err(invalid_event(anyhow!(
                "{} events are not handled by the off-chain storage",
                other
            )))
        }
    }
    Ok(())
}

/// Runs `write` up to `DB_WRITE_ATTEMPTS` times. A duplicate key is not retried and marks the
/// event as invalid, because writing it again would fail the same way.
async fn retry_db_write<T, F, Fut>(write: F) -> Result<T>
where
    F: Fn() -> Fut,
    Fut: Future<Output = Result<T>>,
{
    let mut attempt = 1;
    loop {
        match write().await {
            Err(error) if is_duplicate_key(&error) => return Err(invalid_event(error)),
            Err(error) if attempt < DB_WRITE_ATTEMPTS => {
                warn!(
                    "EWDS event DB write failed (attempt {}): {:#}",
                    attempt, error
                );
                sleep(Duration::from_millis(
                    DB_RETRY_DELAY_MS * u64::from(attempt),
                ))
                .await;
                attempt += 1;
            }
            result => return result,
        }
    }
}

fn is_duplicate_key(error: &anyhow::Error) -> bool {
    format!("{error:#}").contains("E11000")
}
