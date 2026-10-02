use crate::ewds_handler::{send_message_with_fqcn, EwdsHandlerConfig};
use anyhow::Result;
use primitives::db_api_schema::trades::{ClearingResultSchema, DbTradeSchema};
use primitives::ewds::dto::{
    EwdsClearingResultDto, EwdsEventEnvelope, EwdsMarketStatusDto, EwdsTradeDto,
};
use primitives::ewds::EwdsEventType;
use reqwest::Client;
use serde::Serialize;
use tokio::task::JoinHandle;
use tracing::error;
use uuid::Uuid;

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
            occurred_at,
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
