use crate::ewds_handler::{send_message_with_fqcn, EwdsHandlerConfig};
use anyhow::Result;
use primitives::db_api_schema::{
    orders::DbOrderSchema,
    trades::{ClearingResultSchema, DbTradeSchema},
};
use primitives::ewds::dto::{
    EwdsClearingResultDto, EwdsEventEnvelope, EwdsMarketStatusDto, EwdsOrderDto, EwdsTradeDto,
};
use primitives::ewds::EwdsEventType;
use reqwest::Client;
use serde::Serialize;
use tokio::task::JoinHandle;
use tracing::error;

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

    pub fn publish_order_created(&self, order: DbOrderSchema) -> JoinHandle<()> {
        let event_id = format!("order-created-{}", order.order_id);
        let occurred_at = order.creation_time;
        self.spawn_event(
            EwdsEventType::OrderCreated,
            event_id,
            occurred_at,
            EwdsOrderDto::from(order),
        )
    }

    pub fn publish_trade_created(&self, trade: DbTradeSchema) -> JoinHandle<()> {
        let event_id = format!("trade-created-{}", trade.trade_uuid);
        let occurred_at = trade.creation_time;
        self.spawn_event(
            EwdsEventType::TradeCreated,
            event_id,
            occurred_at,
            EwdsTradeDto::from(trade),
        )
    }

    pub fn publish_clearing_result_created(
        &self,
        clearing_result: ClearingResultSchema,
    ) -> JoinHandle<()> {
        // A market can be cleared more than once (e.g. partial, then final), so the market ID
        // alone does not identify the clearing.
        let event_id = format!(
            "clearing-result-created-{}-{}",
            clearing_result.market_id, clearing_result.tx_hash
        );
        let occurred_at = clearing_result.clearing_time;
        self.spawn_event(
            EwdsEventType::ClearingResultCreated,
            event_id,
            occurred_at,
            EwdsClearingResultDto::from(clearing_result),
        )
    }

    pub fn publish_market_status_updated(
        &self,
        market_status: EwdsMarketStatusDto,
        occurred_at: u64,
    ) -> JoinHandle<()> {
        let status = if market_status.is_open {
            "open"
        } else {
            "closed"
        };
        let event_id = format!(
            "market-status-updated-{}-{}",
            market_status.market_id, status
        );
        self.spawn_event(
            EwdsEventType::MarketStatusUpdated,
            event_id,
            occurred_at,
            market_status,
        )
    }

    fn spawn_event<T: Serialize + Send + 'static>(
        &self,
        event_type: EwdsEventType,
        event_id: String,
        occurred_at: u64,
        data: T,
    ) -> JoinHandle<()> {
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
