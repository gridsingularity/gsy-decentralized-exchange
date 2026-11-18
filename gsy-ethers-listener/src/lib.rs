use anyhow::{anyhow, Result};
use async_trait::async_trait;
use ethers::prelude::*;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::time::{sleep, Duration};
use tracing::{error, info};

const RECONNECT_DELAY: Duration = Duration::from_secs(2);

abigen!(
    GsyContracts,
    r#"[
        event OrderPlaced(bytes16 indexed orderId, bytes16 indexed createdBy, bytes16 indexed marketId, uint64 timeSlot, uint64 creationTime, uint64 energy, uint64 energyRate, uint8 energySourcePreference, uint8 energyType, bool isBid, bytes16 preferredTradingPartner, uint64 preferredEnergyRate)
        event OrderCancelled(bytes16 indexed orderId)
        event OrderStatusUpdated(bytes16 indexed orderId, uint8 status)
        event TradeSettled(bytes16 indexed tradeId, bytes16 indexed bidId, bytes16 indexed offerId, bytes16 buyerId, bytes16 sellerId, bytes16 marketId, uint64 timeSlot, bytes16 residualBidId, bytes16 residualOfferId, uint256 energy, uint256 price)
        event DeliveryVerified(bytes16 indexed tradeId, bytes16 indexed bidId, bytes16 indexed offerId)
        event PenaltyRecorded(bytes16 indexed penalizedActorId, bytes16 indexed marketId, bytes16 indexed tradeId, uint64 penaltyEnergy)
        event NewMarketCreated(bytes16 indexed marketId, bytes16 indexed communityId, uint64 openingTime, uint64 closingTime, uint64 deliveryStartTime, uint64 deliveryEndTime, uint8 marketType, uint8 matchingAlgorithm, uint64 createdAt)
        event MarketClearing(bytes16 indexed marketId, uint8 clearingStatus, uint256 clearingPrice, uint256 totalSupply, uint256 totalDemand, uint256 tradedQuantity, uint32 numTrades)
    ]"#
);

#[derive(Clone, Debug)]
pub struct ListenerConfig {
    pub node_url: String,
    pub order_registry_address: Address,
    pub trade_settlement_address: Address,
    pub market_controller_address: Address,
}

/// Original chain data retained when an off-chain projection cannot yet be resolved.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ChainEvent {
    pub stream_id: String,
    pub log: Log,
    pub block_timestamp: u64,
}

impl ChainEvent {
    pub fn decode(&self) -> Result<GsyContractsEvents> {
        let raw = ethers::abi::RawLog {
            topics: self.log.topics.clone(),
            data: self.log.data.to_vec(),
        };
        Ok(<GsyContractsEvents as ethers::contract::EthLogDecode>::decode_log(&raw)?)
    }

    pub fn metadata(&self) -> Result<LogMeta> {
        anyhow::ensure!(
            self.log.block_hash.is_some()
                && self.log.block_number.is_some()
                && self.log.transaction_hash.is_some()
                && self.log.transaction_index.is_some()
                && self.log.log_index.is_some(),
            "Contract log is missing mined-event metadata"
        );
        Ok(LogMeta::from(&self.log))
    }

    pub async fn dispatch<H: GsyEventHandler + ?Sized>(&self, handler: &H) -> Result<()> {
        let timestamp = self.block_timestamp;
        match self.decode()? {
            GsyContractsEvents::OrderPlacedFilter(event) => {
                handler.handle_order_placed(event, timestamp).await
            }
            GsyContractsEvents::OrderCancelledFilter(event) => {
                handler.handle_order_cancelled(event, timestamp).await
            }
            GsyContractsEvents::TradeSettledFilter(event) => {
                handler.handle_trade_settled(event, timestamp).await
            }
            GsyContractsEvents::DeliveryVerifiedFilter(event) => {
                handler.handle_delivery_verified(event, timestamp).await
            }
            GsyContractsEvents::PenaltyRecordedFilter(event) => {
                handler.handle_penalty_recorded(event, timestamp).await
            }
            GsyContractsEvents::NewMarketCreatedFilter(event) => {
                handler.handle_new_market_created(event).await
            }
            GsyContractsEvents::MarketClearingFilter(event) => {
                handler
                    .handle_market_clearing(event, self.metadata()?, timestamp)
                    .await
            }
            _ => Ok(()),
        }
    }
}

#[async_trait]
pub trait GsyEventHandler: Send + Sync + 'static {
    async fn handle_event(&self, event: ChainEvent) -> Result<()> {
        event.dispatch(self).await
    }

    /// Retry deferred projections before applying this block's time-based transitions.
    async fn finish_block(&self, _stream_id: &str, block_timestamp: u64) -> Result<()> {
        self.handle_block(block_timestamp).await
    }
    /// Resume this chain/contract set, or initialize its first live-only boundary.
    async fn start_block(&self, _stream_id: &str, live_start: u64) -> Result<u64> {
        Ok(live_start)
    }

    async fn checkpoint(&self, _stream_id: &str, _next_block: u64) -> Result<()> {
        Ok(())
    }

    // *Filter types are automatically generated from the ABI based on the event name.
    async fn handle_order_placed(
        &self,
        event: OrderPlacedFilter,
        block_timestamp: u64,
    ) -> Result<()>;
    async fn handle_order_cancelled(
        &self,
        event: OrderCancelledFilter,
        block_timestamp: u64,
    ) -> Result<()>;
    async fn handle_trade_settled(
        &self,
        event: TradeSettledFilter,
        block_timestamp: u64,
    ) -> Result<()>;
    /// Called after contract events in an observed block have been handled.
    async fn handle_block(&self, _block_timestamp: u64) -> Result<()> {
        Ok(())
    }

    async fn handle_new_market_created(&self, event: NewMarketCreatedFilter) -> Result<()>;
    async fn handle_delivery_verified(
        &self,
        event: DeliveryVerifiedFilter,
        block_timestamp: u64,
    ) -> Result<()>;
    async fn handle_penalty_recorded(
        &self,
        event: PenaltyRecordedFilter,
        block_timestamp: u64,
    ) -> Result<()>;
    async fn handle_market_clearing(
        &self,
        event: MarketClearingFilter,
        meta: LogMeta,
        block_timestamp: u64,
    ) -> Result<()>;
}

pub struct GsyEthersListener<H: GsyEventHandler> {
    config: ListenerConfig,
    handler: Arc<H>,
}

impl<H: GsyEventHandler> GsyEthersListener<H> {
    pub fn new(config: ListenerConfig, handler: H) -> Self {
        Self {
            config,
            handler: Arc::new(handler),
        }
    }

    pub async fn run(&self) -> Result<()> {
        let mut next_block = None;
        loop {
            if let Err(error) = self.run_once(&mut next_block).await {
                error!(
                    "GSy Ethers Listener connection failed or stopped: {:?}. Reconnecting in {:?}...",
                    error, RECONNECT_DELAY
                );
            }

            sleep(RECONNECT_DELAY).await;
        }
    }

    async fn run_once(&self, next_block: &mut Option<u64>) -> Result<()> {
        info!("Connecting to EVM Node at {}", self.config.node_url);

        let provider = Provider::<Ws>::connect(&self.config.node_url).await?;
        // Fetch one block at a time so expiry cannot overtake settlement events.
        let filter = Filter::new()
            .address(vec![
                self.config.order_registry_address,
                self.config.trade_settlement_address,
                self.config.market_controller_address,
            ])
            .topic0(vec![
                OrderPlacedFilter::signature(),
                OrderCancelledFilter::signature(),
                TradeSettledFilter::signature(),
                DeliveryVerifiedFilter::signature(),
                PenaltyRecordedFilter::signature(),
                NewMarketCreatedFilter::signature(),
                MarketClearingFilter::signature(),
            ]);
        let stream_id = format!(
            "{}:{:?}:{:?}:{:?}",
            provider.get_chainid().await?,
            self.config.order_registry_address,
            self.config.trade_settlement_address,
            self.config.market_controller_address
        );
        if next_block.is_none() {
            let live_start = provider.get_block_number().await?.as_u64() + 1;
            *next_block = Some(self.handler.start_block(&stream_id, live_start).await?);
        }
        let mut stream = provider.subscribe_blocks().await?;
        let mut head = provider.get_block_number().await?.as_u64();
        let mut number = next_block.expect("Listener start block initialized");
        info!("GSy Ethers Listener started. Waiting for blocks...");

        loop {
            while number <= head {
                // Retain the failed block across reconnects instead of skipping its events.
                *next_block = Some(number);
                let block = provider
                    .get_block(number)
                    .await?
                    .ok_or_else(|| anyhow!("Block {} not found", number))?;
                let hash = block
                    .hash
                    .ok_or_else(|| anyhow!("Block {} has no hash", number))?;
                let timestamp = block.timestamp.as_u64();
                let mut logs = provider
                    .get_logs(&filter.clone().at_block_hash(hash))
                    .await?;
                logs.sort_by_key(|log| log.log_index);
                for log in logs {
                    self.handle_log(&stream_id, log, timestamp).await?;
                }
                self.handler.finish_block(&stream_id, timestamp).await?;
                self.handler.checkpoint(&stream_id, number + 1).await?;
                number += 1;
                *next_block = Some(number);
            }
            head = stream
                .next()
                .await
                .ok_or_else(|| anyhow!("Contract block stream ended"))?
                .number
                .ok_or_else(|| anyhow!("Block notification has no number"))?
                .as_u64();
        }
    }

    async fn handle_log(&self, stream_id: &str, log: Log, timestamp: u64) -> Result<()> {
        let event = ChainEvent {
            stream_id: stream_id.to_owned(),
            log,
            block_timestamp: timestamp,
        };
        let expected_address = match event.decode()? {
            GsyContractsEvents::OrderPlacedFilter(_)
            | GsyContractsEvents::OrderCancelledFilter(_) => self.config.order_registry_address,
            GsyContractsEvents::NewMarketCreatedFilter(_) => self.config.market_controller_address,
            GsyContractsEvents::TradeSettledFilter(_)
            | GsyContractsEvents::PenaltyRecordedFilter(_)
            | GsyContractsEvents::DeliveryVerifiedFilter(_)
            | GsyContractsEvents::MarketClearingFilter(_) => self.config.trade_settlement_address,
            _ => return Ok(()),
        };
        if event.log.address == expected_address {
            event.metadata()?;
            self.handler.handle_event(event).await?;
        }
        Ok(())
    }
}
