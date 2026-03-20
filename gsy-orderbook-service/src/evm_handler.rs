use crate::db::DatabaseWrapper;
use anyhow::Result;
use async_trait::async_trait;
use gsy_ethers_listener::{
    GsyEventHandler, MarketStatusUpdatedFilter, OrderCancelledFilter, OrderPlacedFilter,
    TradeSettledFilter,
};
use gsy_offchain_primitives::db_api_schema::{
    orders::{DbOrderSchema, OrderEnum, OrderStatus},
    trades::{TradeParameters, TradeSchema, TradeStatus},
};
use gsy_offchain_primitives::utils::NODE_FLOAT_SCALING_FACTOR;
use tracing::{error, info, warn};
use uuid::Uuid;

pub struct OrderbookEvmHandler {
    pub db: DatabaseWrapper,
}

#[async_trait]
impl GsyEventHandler for OrderbookEvmHandler {
    async fn handle_order_placed(&self, event: OrderPlacedFilter) -> Result<()> {
        info!(
            "Processing EVM OrderPlaced: {:?}",
            hex::encode(event.order_hash)
        );

        let energy_f64 = event.energy as f64 / NODE_FLOAT_SCALING_FACTOR;
        let rate_f64 = event.energy_rate as f64 / NODE_FLOAT_SCALING_FACTOR;

        let area_uuid_str = format!("0x{}", hex::encode(event.area_uuid));
        let market_id_str = format!("0x{}", hex::encode(event.market_id));
        let order_id_str = format!("0x{}", hex::encode(event.order_hash));
        let owner_str = format!("{:?}", event.owner);

        let order_enum = if event.is_bid {
            OrderEnum::Bid
        } else {
            OrderEnum::Offer
        };

        let schema = DbOrderSchema {
            order_id: order_id_str,
            status: OrderStatus::Open,
            order_type: order_enum,
            area_uuid: area_uuid_str,
            market_id: market_id_str,
            nonce: Some(event.nonce),
            time_slot: event.time_slot,
            creation_time: event.creation_time,
            energy_kWh: energy_f64,
            energy_rate: rate_f64,
            created_by: owner_str,
            requirements: None,
            attributes: None,
        };

        match self.db.orders().insert_orders(vec![schema]).await {
            Ok(_) => info!("Successfully indexed order from EVM"),
            Err(e) => error!("Failed to insert order into DB: {:?}", e),
        }

        Ok(())
    }

    async fn handle_order_cancelled(&self, event: OrderCancelledFilter) -> Result<()> {
        info!(
            "Processing EVM OrderCancelled: {:?}",
            hex::encode(event.order_hash)
        );
        let id_bson =
            mongodb::bson::to_bson(&format!("0x{}", hex::encode(event.order_hash))).unwrap();

        match self
            .db
            .orders()
            .update_order_status_by_id(&id_bson, OrderStatus::Deleted)
            .await
        {
            Ok(_) => info!("Successfully marked order as deleted"),
            Err(e) => error!("Failed to update order status: {:?}", e),
        }
        Ok(())
    }

    async fn handle_trade_settled(&self, event: TradeSettledFilter) -> Result<()> {
        let trade_hash = format!("0x{}", hex::encode(event.trade_id));
        info!("Processing EVM TradeSettled: {:?}", trade_hash);

        let energy_f64 = event.energy.as_u64() as f64 / NODE_FLOAT_SCALING_FACTOR;
        let price_f64 = event.price.as_u64() as f64 / NODE_FLOAT_SCALING_FACTOR;

        let bid_hash_str = format!("0x{}", hex::encode(event.bid_hash));
        let ask_hash_str = format!("0x{}", hex::encode(event.ask_hash));

        let bid_bson = mongodb::bson::to_bson(&bid_hash_str).unwrap();
        let ask_bson = mongodb::bson::to_bson(&ask_hash_str).unwrap();

        let bid_doc = self.db.orders().get_order_by_id(&bid_bson).await?;
        let ask_doc = self.db.orders().get_order_by_id(&ask_bson).await?;

        if let (Some(bid_order), Some(ask_order)) = (bid_doc, ask_doc) {
            let trade_schema = TradeSchema {
                trade_uuid: Uuid::new_v4().to_string(),
                status: TradeStatus::Settled,
                seller: ask_order.created_by.clone(),
                buyer: bid_order.created_by.clone(),
                market_id: bid_order.market_id.clone(),
                time_slot: bid_order.time_slot,
                creation_time: chrono::Utc::now().timestamp() as u64,
                offer: ask_order,
                offer_hash: ask_hash_str,
                bid: bid_order,
                bid_hash: bid_hash_str,
                residual_offer: None,
                residual_bid: None,
                parameters: TradeParameters {
                    selected_energy_kWh: energy_f64,
                    energy_rate: price_f64,
                },
            };

            self.db.trades().insert_trades(vec![trade_schema]).await?;

            self.db
                .orders()
                .update_order_status_by_id(&bid_bson, OrderStatus::Executed)
                .await?;
            self.db
                .orders()
                .update_order_status_by_id(&ask_bson, OrderStatus::Executed)
                .await?;

            info!("Trade persisted and orders updated.");
        } else {
            warn!("Could not find one of the orders for the settled trade. Skipping persistence.");
        }

        Ok(())
    }

    async fn handle_market_status(&self, event: MarketStatusUpdatedFilter) -> Result<()> {
        info!(
            "Processing EVM MarketStatus: {:?} -> Open? {}",
            hex::encode(event.market_id),
            event.is_open
        );
        Ok(())
    }
}
