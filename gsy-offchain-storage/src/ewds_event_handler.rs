use crate::db::measurements_service::insert_measurements;
use crate::db::DatabaseWrapper;
use crate::ewds_handler::{
    next_poll_delay_ms, poll_messages, remember_id, send_message_with_fqcn, EwdsHandlerConfig,
};
use anyhow::{bail, Context, Result};
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
use primitives::ewds::EwdsEventType;
use primitives::utils::epoch_to_rfc3339;
use reqwest::Client;
use serde::Serialize;
use serde_json::Value;
use std::collections::{HashSet, VecDeque};
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

    let workers = SUBSCRIBED_EVENT_TYPES
        .into_iter()
        .map(|event_type| run_event_worker(db.clone(), Client::new(), config.clone(), event_type));
    join_all(workers).await;
}

async fn run_event_worker(
    db: DatabaseWrapper,
    client: Client,
    config: EwdsHandlerConfig,
    event_type: EwdsEventType,
) {
    let mut seen_event_ids: HashSet<String> = HashSet::new();
    let mut seen_queue: VecDeque<String> = VecDeque::new();
    let mut rate_limit_attempt = 0u32;

    loop {
        let result = process_event_batch(
            &db,
            &client,
            &config,
            event_type,
            &mut seen_event_ids,
            &mut seen_queue,
        )
        .await;
        if let Err(error) = &result {
            warn!("EWDS {} event worker failed to poll: {}", event_type, error);
        }

        let delay_ms = next_poll_delay_ms(&result, &config, &mut rate_limit_attempt);
        sleep(Duration::from_millis(delay_ms)).await;
    }
}

async fn process_event_batch(
    db: &DatabaseWrapper,
    client: &Client,
    config: &EwdsHandlerConfig,
    event_type: EwdsEventType,
    seen_event_ids: &mut HashSet<String>,
    seen_queue: &mut VecDeque<String>,
) -> Result<()> {
    let amount = config.request_batch_size.to_string();
    let topic_name = config.event_topic(event_type);
    let messages = poll_messages(client, config, topic_name, amount.as_str()).await?;

    for message in messages {
        let envelope = match serde_json::from_str::<EwdsEventEnvelope<Value>>(&message.payload) {
            Ok(envelope) => envelope,
            Err(error) => {
                warn!(
                    "Skipping malformed EWDS message on topic '{}': {}",
                    topic_name, error
                );
                continue;
            }
        };
        if envelope.event_type != event_type {
            warn!(
                "Skipping EWDS {} event {} on topic '{}', which carries {} events",
                envelope.event_type, envelope.event_id, topic_name, event_type
            );
            continue;
        }
        if seen_event_ids.contains(&envelope.event_id) {
            continue;
        }

        let event_id = envelope.event_id.clone();
        match handle_event(db, envelope).await {
            Ok(()) => info!("Saved EWDS {} event {}", event_type, event_id),
            Err(error) => error!(
                "Dropping EWDS {} event {}: {:#}",
                event_type, event_id, error
            ),
        }
        remember_id(&event_id, seen_event_ids, seen_queue);
    }

    Ok(())
}

pub async fn handle_event(db: &DatabaseWrapper, envelope: EwdsEventEnvelope<Value>) -> Result<()> {
    match envelope.event_type {
        EwdsEventType::MeasurementsSubmitted => {
            let measurements = parse_measurements(envelope.data)?;
            retry_db_write(|| insert_measurements(db, &measurements)).await?;
        }
        EwdsEventType::FacilitySubmitted => {
            let facility: FacilitySchema = serde_json::from_value(envelope.data)?;
            retry_db_write(|| {
                let facility = facility.clone();
                async move { db.facilities().upsert(facility).await }
            })
            .await?;
        }
        EwdsEventType::SiteSubmitted => {
            let site: SiteSchema = serde_json::from_value(envelope.data)?;
            retry_db_write(|| {
                let site = site.clone();
                async move { db.sites().upsert(site).await }
            })
            .await?;
        }
        EwdsEventType::CommunitySubmitted => {
            let community: EwdsCommunityDto = serde_json::from_value(envelope.data)?;
            let community = EnergyCommunitySchema::from(community);
            retry_db_write(|| {
                let community = community.clone();
                async move { db.communities().upsert(community).await }
            })
            .await?;
        }
        other => bail!("{} events are published by GSY, not saved from EWDS", other),
    }
    Ok(())
}

/// Parses a measurement batch. One invalid item rejects the whole batch.
fn parse_measurements(data: Value) -> Result<Vec<MeasurementSchema>> {
    let items: Vec<Value> =
        serde_json::from_value(data).context("the measurement batch is not a list")?;
    if items.is_empty() {
        bail!("the measurement batch is empty");
    }

    items
        .into_iter()
        .enumerate()
        .map(|(index, item)| {
            serde_json::from_value::<EwdsMeasurementDto>(item)
                .map_err(anyhow::Error::from)
                .and_then(MeasurementSchema::try_from)
                .with_context(|| format!("invalid measurement at index {}", index))
        })
        .collect()
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
