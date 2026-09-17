use crate::db::DatabaseWrapper;
use anyhow::Result;
use async_trait::async_trait;
use gsy_ethers_listener::{
    GsyEventHandler, MarketStatusUpdatedFilter, OrderCancelledFilter, OrderPlacedFilter,
    TradeSettledFilter,
};
use primitives::db_api_schema::{
    orders::{
        energy_type_from_contract, DbAttributes, DbOrderSchema, DbRequirements, OrderEnum,
        OrderStatus,
    },
    trades::{DbTradeSchema, TradeParameters, TradeStatus},
};
use primitives::utils::{bytes16_to_hex, NODE_FLOAT_SCALING_FACTOR};
use tracing::{error, info};

pub struct OffchainStorageEvmHandler {
    pub db: DatabaseWrapper,
}

#[async_trait]
impl GsyEventHandler for OffchainStorageEvmHandler {
    async fn handle_order_placed(&self, event: OrderPlacedFilter) -> Result<()> {
        info!(
            "Processing EVM OrderPlaced: {:?}",
            hex::encode(event.order_id)
        );

        let energy_f64 = event.energy as f64 / NODE_FLOAT_SCALING_FACTOR;
        let rate_f64 = event.energy_rate as f64 / NODE_FLOAT_SCALING_FACTOR;

        let market_id_str = bytes16_to_hex(event.market_id);
        let order_id_str = bytes16_to_hex(event.order_id);
        let created_by_str = bytes16_to_hex(event.created_by);

        let order_enum = if event.is_bid {
            OrderEnum::Bid
        } else {
            OrderEnum::Offer
        };

        let schema = DbOrderSchema {
            order_id: order_id_str,
            status: OrderStatus::Submitted,
            order_type: order_enum,
            area_uuid: created_by_str.clone(),
            market_id: market_id_str,
            time_slot: event.time_slot,
            creation_time: event.creation_time,
            energy_kWh: energy_f64,
            energy_rate: rate_f64,
            created_by: created_by_str,
            requirements: requirements_from_event(&event),
            attributes: attributes_from_event(&event),
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
            hex::encode(event.order_id)
        );
        let id_bson = mongodb::bson::to_bson(&bytes16_to_hex(event.order_id)).unwrap();

        match self
            .db
            .orders()
            .update_order_status_by_id(&id_bson, OrderStatus::Cancelled)
            .await
        {
            Ok(_) => info!("Successfully marked order as deleted"),
            Err(e) => error!("Failed to update order status: {:?}", e),
        }
        Ok(())
    }

    async fn handle_trade_settled(&self, event: TradeSettledFilter) -> Result<()> {
        let trade_hash = bytes16_to_hex(event.trade_id);
        info!("Processing EVM TradeSettled: {:?}", trade_hash);

        let energy_f64 = event.energy.as_u64() as f64 / NODE_FLOAT_SCALING_FACTOR;
        let price_f64 = event.price.as_u64() as f64 / NODE_FLOAT_SCALING_FACTOR;

        let bid_hash_str = bytes16_to_hex(event.bid_id);
        let offer_hash_str = bytes16_to_hex(event.offer_id);
        let residual_bid_id = bytes16_to_optional_hex(event.residual_bid_id);
        let residual_offer_id = bytes16_to_optional_hex(event.residual_offer_id);

        let bid_bson = mongodb::bson::to_bson(&bid_hash_str).unwrap();
        let offer_bson = mongodb::bson::to_bson(&offer_hash_str).unwrap();

        let trade_schema = DbTradeSchema {
            trade_uuid: trade_hash.clone(),
            status: TradeStatus::Settled,
            seller: bytes16_to_hex(event.seller_id),
            buyer: bytes16_to_hex(event.buyer_id),
            market_id: bytes16_to_hex(event.market_id),
            time_slot: event.time_slot,
            creation_time: chrono::Utc::now().timestamp() as u64,
            offer_hash: offer_hash_str,
            bid_hash: bid_hash_str,
            residual_offer_id,
            residual_bid_id,
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
            .update_order_status_by_id(&offer_bson, OrderStatus::Executed)
            .await?;

        info!("Trade persisted and orders updated.");

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

fn requirements_from_event(event: &OrderPlacedFilter) -> Option<DbRequirements> {
    if !event.is_bid {
        return None;
    }

    energy_type_from_contract(event.energy_source_preference).map(|energy_type| DbRequirements {
        trading_partner_id: None,
        energy_type: Some(energy_type),
        preferred_energy_rate: None,
    })
}

fn attributes_from_event(event: &OrderPlacedFilter) -> Option<DbAttributes> {
    if event.is_bid {
        return None;
    }

    energy_type_from_contract(event.energy_type).map(|energy_type| DbAttributes {
        trading_partner_id: None,
        energy_type,
    })
}

fn bytes16_to_optional_hex(value: [u8; 16]) -> Option<String> {
    if value == [0u8; 16] {
        None
    } else {
        Some(bytes16_to_hex(value))
    }
}
