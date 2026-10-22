use anyhow::{anyhow, Result};
use async_trait::async_trait;
use ethers::prelude::*;
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

#[async_trait]
pub trait GsyEventHandler: Send + Sync + 'static {
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
    /// Called after all contract events in a confirmed block have been indexed.
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
        // Preserve the live-only startup boundary; replay needs a persistent cursor.
        // Record it before subscribing, then fetch any blocks mined during setup.
        if next_block.is_none() {
            *next_block = Some(provider.get_block_number().await?.as_u64() + 1);
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
                    self.handle_log(log, timestamp).await?;
                }
                self.handler.handle_block(timestamp).await?;
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

    async fn handle_log(&self, log: Log, timestamp: u64) -> Result<()> {
        let raw = ethers::abi::RawLog {
            topics: log.topics.clone(),
            data: log.data.to_vec(),
        };
        let event = <GsyContractsEvents as ethers::contract::EthLogDecode>::decode_log(&raw)
            .map_err(|error| anyhow!("Contract event decode error: {:?}", error))?;
        match event {
            GsyContractsEvents::OrderPlacedFilter(event)
                if log.address == self.config.order_registry_address =>
            {
                self.handler.handle_order_placed(event, timestamp).await
            }
            GsyContractsEvents::OrderCancelledFilter(event)
                if log.address == self.config.order_registry_address =>
            {
                self.handler.handle_order_cancelled(event, timestamp).await
            }
            GsyContractsEvents::TradeSettledFilter(event)
                if log.address == self.config.trade_settlement_address =>
            {
                self.handler.handle_trade_settled(event, timestamp).await
            }
            GsyContractsEvents::PenaltyRecordedFilter(event)
                if log.address == self.config.trade_settlement_address =>
            {
                self.handler.handle_penalty_recorded(event, timestamp).await
            }
            GsyContractsEvents::DeliveryVerifiedFilter(event)
                if log.address == self.config.trade_settlement_address =>
            {
                self.handler
                    .handle_delivery_verified(event, timestamp)
                    .await
            }
            GsyContractsEvents::NewMarketCreatedFilter(event)
                if log.address == self.config.market_controller_address =>
            {
                self.handler.handle_new_market_created(event).await
            }
            GsyContractsEvents::MarketClearingFilter(event)
                if log.address == self.config.trade_settlement_address =>
            {
                self.handler
                    .handle_market_clearing(event, LogMeta::from(&log), timestamp)
                    .await
            }
            _ => Ok(()),
        }
    }
}
