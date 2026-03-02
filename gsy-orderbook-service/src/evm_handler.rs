use crate::db::DatabaseWrapper;
use anyhow::Result;
use async_trait::async_trait;
use gsy_ethers_listener::{
    GsyEventHandler, MarketStatusUpdatedFilter, OrderCancelledFilter, OrderPlacedFilter,
    TradeSettledFilter,
};
use gsy_offchain_primitives::db_api_schema::{
    orders::{DbBid, DbOffer, DbOrderComponent, DbOrderSchema, Order as DbOrder, OrderStatus},
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

        let component = DbOrderComponent {
            area_uuid: area_uuid_str,
            market_id: market_id_str,
            time_slot: event.time_slot,
            creation_time: event.creation_time,
            energy: energy_f64,
            energy_rate: rate_f64,
        };

        let order_enum = if event.is_bid {
            DbOrder::Bid(DbBid {
                buyer: owner_str,
                nonce: event.nonce as u32,
                bid_component: component,
                requirements: None,
            })
        } else {
            DbOrder::Offer(DbOffer {
                seller: owner_str,
                nonce: event.nonce as u32,
                offer_component: component,
                attributes: None,
            })
        };

        let schema = DbOrderSchema {
            _id: order_id_str,
            status: OrderStatus::Open,
            order: order_enum,
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
        let trade_id = format!("0x{}", hex::encode(event.trade_id));
        info!("Processing EVM TradeSettled: {:?}", trade_id);

        let energy_f64 = event.energy.as_u64() as f64 / NODE_FLOAT_SCALING_FACTOR;
        let price_f64 = event.price.as_u64() as f64 / NODE_FLOAT_SCALING_FACTOR;

        let bid_hash_str = format!("0x{}", hex::encode(event.bid_hash));
        let ask_hash_str = format!("0x{}", hex::encode(event.ask_hash));

        let bid_bson = mongodb::bson::to_bson(&bid_hash_str).unwrap();
        let ask_bson = mongodb::bson::to_bson(&ask_hash_str).unwrap();

        let bid_doc = self.db.orders().get_order_by_id(&bid_bson).await?;
        let ask_doc = self.db.orders().get_order_by_id(&ask_bson).await?;

        if let (Some(bid_order), Some(ask_order)) = (bid_doc, ask_doc) {
            let db_bid = match bid_order.order {
                DbOrder::Bid(b) => b,
                _ => return Ok(()),
            };
            let db_ask = match ask_order.order {
                DbOrder::Offer(o) => o,
                _ => return Ok(()),
            };

            let trade_schema = TradeSchema {
                _id: Uuid::new_v4().to_string(),
                status: TradeStatus::Settled,
                seller: db_ask.seller.clone(),
                buyer: db_bid.buyer.clone(),
                market_id: db_bid.bid_component.market_id.clone(),
                time_slot: db_bid.bid_component.time_slot,
                trade_uuid: trade_id,
                creation_time: chrono::Utc::now().timestamp() as u64,
                offer: db_ask,
                offer_hash: ask_hash_str,
                bid: db_bid,
                bid_hash: bid_hash_str,
                residual_offer: None,
                residual_bid: None,
                parameters: TradeParameters {
                    selected_energy: energy_f64,
                    energy_rate: price_f64,
                    trade_uuid: format!("0x{}", hex::encode(event.trade_id)),
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
