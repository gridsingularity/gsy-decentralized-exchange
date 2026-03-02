use anyhow::Result;
use async_trait::async_trait;
use ethers::prelude::*;
use std::sync::Arc;
use tracing::{error, info};

abigen!(
    GsyContracts,
    r#"[
        event OrderPlaced(bytes32 indexed orderHash, address indexed owner, bytes32 indexed marketId, bytes32 areaUuid, uint64 nonce, uint64 timeSlot, uint64 creationTime, uint64 energy, uint64 energyRate, bool isBid)
        event OrderCancelled(bytes32 indexed orderHash)
        event OrderStatusUpdated(bytes32 indexed orderHash, uint8 status)
        event TradeSettled(bytes32 indexed tradeId, bytes32 indexed bidHash, bytes32 indexed askHash, uint256 energy, uint256 price)
        event MarketStatusUpdated(bytes32 indexed marketId, bool isOpen)
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
        info!("Connecting to EVM Node at {}", self.config.node_url);

        let provider = Provider::<Ws>::connect(&self.config.node_url).await?;
        let client = Arc::new(provider);

        let order_registry = GsyContracts::new(self.config.order_registry_address, client.clone());
        let trade_settlement =
            GsyContracts::new(self.config.trade_settlement_address, client.clone());
        let market_controller =
            GsyContracts::new(self.config.market_controller_address, client.clone());

        let order_placed_filter = order_registry.event::<OrderPlacedFilter>();
        let order_cancelled_filter = order_registry.event::<OrderCancelledFilter>();
        let trade_settled_filter = trade_settlement.event::<TradeSettledFilter>();
        let market_status_filter = market_controller.event::<MarketStatusUpdatedFilter>();

        let mut stream_order_placed = order_placed_filter.subscribe().await?;
        let mut stream_order_cancelled = order_cancelled_filter.subscribe().await?;
        let mut stream_trade_settled = trade_settled_filter.subscribe().await?;
        let mut stream_market_status = market_status_filter.subscribe().await?;

        info!("GSy Ethers Listener started. Waiting for events...");

        loop {
            tokio::select! {
                Some(log) = stream_order_placed.next() => {
                    match log {
                        Ok(event) => {
                            info!("Detected OrderPlaced: {:?}", hex::encode(event.order_hash));
                            if let Err(e) = self.handler.handle_order_placed(event).await {
                                error!("Error handling OrderPlaced: {:?}", e);
                            }
                        },
                        Err(e) => error!("Error in OrderPlaced stream: {:?}", e),
                    }
                }
                Some(log) = stream_order_cancelled.next() => {
                    match log {
                        Ok(event) => {
                            info!("Detected OrderCancelled: {:?}", hex::encode(event.order_hash));
                            if let Err(e) = self.handler.handle_order_cancelled(event).await {
                                error!("Error handling OrderCancelled: {:?}", e);
                            }
                        },
                        Err(e) => error!("Error in OrderCancelled stream: {:?}", e),
                    }
                }
                Some(log) = stream_trade_settled.next() => {
                    match log {
                        Ok(event) => {
                            info!("Detected TradeSettled: {:?}", hex::encode(event.trade_id));
                            if let Err(e) = self.handler.handle_trade_settled(event).await {
                                error!("Error handling TradeSettled: {:?}", e);
                            }
                        },
                        Err(e) => error!("Error in TradeSettled stream: {:?}", e),
                    }
                }
                Some(log) = stream_market_status.next() => {
                    match log {
                        Ok(event) => {
                            info!("Detected MarketStatusUpdated: {:?}", hex::encode(event.market_id));
                            if let Err(e) = self.handler.handle_market_status(event).await {
                                error!("Error handling MarketStatus: {:?}", e);
                            }
                        },
                        Err(e) => error!("Error in MarketStatus stream: {:?}", e),
                    }
                }
            }
        }
    }
}
