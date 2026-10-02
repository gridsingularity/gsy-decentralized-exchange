use crate::db::measurements_service::insert_measurements;
use crate::db::DatabaseWrapper;
use crate::ewds_handler::{send_message_with_fqcn, EwdsHandlerConfig};
use anyhow::{bail, Result};
use futures::future::join_all;
use primitives::db_api_schema::{
    grid_topology::{EnergyCommunitySchema, FacilitySchema, SiteSchema},
    profiles::MeasurementSchema,
    trades::{ClearingResultSchema, DbTradeSchema},
};
use primitives::ewds::dto::{
    EwdsClearingResultDto, EwdsCommunityDto, EwdsEventEnvelope, EwdsMarketStatusDto,
    EwdsMeasurementDto, EwdsTradeDto,
};
use primitives::ewds::{parse_batch, EwdsClient, EwdsClientConfig, EwdsEventType};
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
    EwdsEventType::MeasurementsSubmitted,
    EwdsEventType::FacilitySubmitted,
    EwdsEventType::SiteSubmitted,
    EwdsEventType::CommunitySubmitted,
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

    pub fn publish_trades_created(&self, trades: Vec<DbTradeSchema>) -> JoinHandle<()> {
        let occurred_at = trades
            .iter()
            .map(|trade| trade.creation_time)
            .max()
            .unwrap_or_default();
        self.spawn_event(
            EwdsEventType::TradeCreated,
            occurred_at,
            trades
                .into_iter()
                .map(EwdsTradeDto::from)
                .collect::<Vec<_>>(),
        )
    }

    pub fn publish_clearing_results_created(
        &self,
        clearing_results: Vec<ClearingResultSchema>,
    ) -> JoinHandle<()> {
        let occurred_at = clearing_results
            .iter()
            .map(|clearing_result| clearing_result.clearing_time)
            .max()
            .unwrap_or_default();
        self.spawn_event(
            EwdsEventType::ClearingResultCreated,
            occurred_at,
            clearing_results
                .into_iter()
                .map(EwdsClearingResultDto::from)
                .collect::<Vec<_>>(),
        )
    }

    pub fn publish_market_statuses_updated(
        &self,
        market_statuses: Vec<EwdsMarketStatusDto>,
        occurred_at: u64,
    ) -> JoinHandle<()> {
        self.spawn_event(
            EwdsEventType::MarketStatusUpdated,
            occurred_at,
            market_statuses,
        )
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
        EwdsEventType::MeasurementsSubmitted => {
            let measurements = parse_batch(envelope.data, |measurement: EwdsMeasurementDto| {
                MeasurementSchema::try_from(measurement)
            })?;
            retry_db_write(|| insert_measurements(db, &measurements)).await?;
        }
        EwdsEventType::FacilitySubmitted => {
            let facilities = parse_batch(envelope.data, |facility: FacilitySchema| Ok(facility))?;
            for facility in facilities {
                retry_db_write(|| {
                    let facility = facility.clone();
                    async move { db.facilities().upsert(facility).await }
                })
                .await?;
            }
        }
        EwdsEventType::SiteSubmitted => {
            let sites = parse_batch(envelope.data, |site: SiteSchema| Ok(site))?;
            for site in sites {
                retry_db_write(|| {
                    let site = site.clone();
                    async move { db.sites().upsert(site).await }
                })
                .await?;
            }
        }
        EwdsEventType::CommunitySubmitted => {
            let communities = parse_batch(envelope.data, |community: EwdsCommunityDto| {
                Ok(EnergyCommunitySchema::from(community))
            })?;
            for community in communities {
                retry_db_write(|| {
                    let community = community.clone();
                    async move { db.communities().upsert(community).await }
                })
                .await?;
            }
        }
        other => bail!("{} events are not handled by the off-chain storage", other),
    }
    Ok(())
}

async fn retry_db_write<T, F, Fut>(write: F) -> Result<T>
where
    F: Fn() -> Fut,
    Fut: Future<Output = Result<T>>,
{
    let mut attempt = 1;
    loop {
        match write().await {
            Err(error)
                if attempt < DB_WRITE_ATTEMPTS && !format!("{error:#}").contains("E11000") =>
            {
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
