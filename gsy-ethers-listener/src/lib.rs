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
        event MarketStatusUpdated(bytes16 indexed marketId, bool isOpen)
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
    async fn handle_order_placed(&self, event: OrderPlacedFilter) -> Result<()>;
    async fn handle_order_cancelled(&self, event: OrderCancelledFilter) -> Result<()>;
    async fn handle_trade_settled(&self, event: TradeSettledFilter) -> Result<()>;
    async fn handle_market_status(&self, event: MarketStatusUpdatedFilter) -> Result<()>;
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
        loop {
            if let Err(error) = self.run_once().await {
                error!(
                    "GSy Ethers Listener connection failed or stopped: {:?}. Reconnecting in {:?}...",
                    error, RECONNECT_DELAY
                );
            }

            sleep(RECONNECT_DELAY).await;
        }
    }

    async fn run_once(&self) -> Result<()> {
        info!("Connecting to EVM Node at {}", self.config.node_url);

        let provider = Provider::<Ws>::connect(&self.config.node_url).await?;
        // One subscription preserves log order across registry and settlement events.
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
                MarketStatusUpdatedFilter::signature(),
            ]);
        let mut stream = provider.subscribe_logs(&filter).await?;

        info!("GSy Ethers Listener started. Waiting for events...");

        while let Some(log) = stream.next().await {
            let raw = ethers::abi::RawLog {
                topics: log.topics,
                data: log.data.to_vec(),
            };
            let event = <GsyContractsEvents as ethers::contract::EthLogDecode>::decode_log(&raw)
                .map_err(|error| anyhow!("Contract event decode error: {:?}", error))?;
            let result = match event {
                GsyContractsEvents::OrderPlacedFilter(event)
                    if log.address == self.config.order_registry_address =>
                {
                    info!("Detected OrderPlaced: {:?}", hex::encode(event.order_id));
                    self.handler.handle_order_placed(event).await
                }
                GsyContractsEvents::OrderCancelledFilter(event)
                    if log.address == self.config.order_registry_address =>
                {
                    info!("Detected OrderCancelled: {:?}", hex::encode(event.order_id));
                    self.handler.handle_order_cancelled(event).await
                }
                GsyContractsEvents::TradeSettledFilter(event)
                    if log.address == self.config.trade_settlement_address =>
                {
                    info!("Detected TradeSettled: {:?}", hex::encode(event.trade_id));
                    self.handler.handle_trade_settled(event).await
                }
                GsyContractsEvents::MarketStatusUpdatedFilter(event)
                    if log.address == self.config.market_controller_address =>
                {
                    info!(
                        "Detected MarketStatusUpdated: {:?}",
                        hex::encode(event.market_id)
                    );
                    self.handler.handle_market_status(event).await
                }
                _ => continue,
            };
            if let Err(error) = result {
                error!("Error handling contract event: {:?}", error);
            }
        }
        Err(anyhow!("Contract event stream ended"))
    }
}
