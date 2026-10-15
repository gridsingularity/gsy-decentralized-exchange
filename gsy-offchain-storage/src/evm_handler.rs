use crate::db::DatabaseWrapper;
use crate::ewds_event_handler::EwdsEventPublisher;
use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use ethers::contract::LogMeta;
use ethers::types::U256;
use gsy_ethers_listener::{
    DeliveryVerifiedFilter, GsyEventHandler, MarketClearingFilter, MarketStatusUpdatedFilter,
    OrderCancelledFilter, OrderPlacedFilter, TradeSettledFilter,
};
use primitives::db_api_schema::{
    orders::{
        order_metadata_from_contract, ContractOrderMetadata, DbOrderSchema, OrderEnum, OrderStatus,
    },
    trades::{ClearingResultSchema, ClearingStatus, DbTradeSchema, TradeParameters, TradeStatus},
};
use primitives::ewds::dto::EwdsMarketStatusDto;
use primitives::utils::{bytes16_to_hex, NODE_FLOAT_SCALING_FACTOR};
use tracing::{error, info};

fn scaled_u256_to_f64(value: U256) -> f64 {
    value.as_u128() as f64 / NODE_FLOAT_SCALING_FACTOR
}

pub struct OffchainStorageEvmHandler {
    pub db: DatabaseWrapper,
    pub event_publisher: Option<EwdsEventPublisher>,
}

#[async_trait]
impl GsyEventHandler for OffchainStorageEvmHandler {
    async fn handle_order_placed(
        &self,
        event: OrderPlacedFilter,
        block_timestamp: u64,
    ) -> Result<()> {
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
        let (mut requirements, attributes) = order_metadata_from_contract(ContractOrderMetadata {
            energy_source_preference: event.energy_source_preference,
            energy_type: event.energy_type,
            preferred_trading_partner: event.preferred_trading_partner,
            preferred_energy_rate: event.preferred_energy_rate,
        });
        if let Some(partner) = requirements
            .as_mut()
            .and_then(|value| value.trading_partner_id.as_mut())
        {
            *partner = self
                .db
                .ids()
                .filter(Some(partner.clone()), None)
                .await?
                .pop()
                .with_context(|| format!("No facility ID mapping for on-chain ID {}", partner))?
                .offchain_id;
        }

        let schema = DbOrderSchema {
            order_id: order_id_str.clone(),
            status: OrderStatus::Submitted,
            status_updated_at: Some(block_timestamp),
            order_type: order_enum,
            area_uuid: created_by_str.clone(),
            market_id: market_id_str,
            time_slot: event.time_slot,
            creation_time: event.creation_time,
            energy_kWh: energy_f64,
            energy_rate: rate_f64,
            created_by: created_by_str,
            requirements,
            attributes,
        };

        self.db
            .orders()
            .insert_orders(vec![schema])
            .await
            .with_context(|| format!("Failed to insert order {} into DB", order_id_str))?;

        info!("Successfully indexed order from EVM");
        Ok(())
    }

    async fn handle_order_cancelled(
        &self,
        event: OrderCancelledFilter,
        block_timestamp: u64,
    ) -> Result<()> {
        info!(
            "Processing EVM OrderCancelled: {:?}",
            hex::encode(event.order_id)
        );
        let id_bson = mongodb::bson::to_bson(&bytes16_to_hex(event.order_id)).unwrap();

        match self
            .db
            .orders()
            .update_order_status_by_id(&id_bson, OrderStatus::Cancelled, block_timestamp)
            .await
        {
            Ok(_) => info!("Successfully marked order as deleted"),
            Err(e) => error!("Failed to update order status: {:?}", e),
        }
        Ok(())
    }

    async fn handle_trade_settled(
        &self,
        event: TradeSettledFilter,
        block_timestamp: u64,
    ) -> Result<()> {
        let trade_hash = bytes16_to_hex(event.trade_id);
        info!("Processing EVM TradeSettled: {:?}", trade_hash);

        let energy_f64 = event.energy.as_u64() as f64 / NODE_FLOAT_SCALING_FACTOR;
        let price_f64 = event.price.as_u64() as f64 / NODE_FLOAT_SCALING_FACTOR;

        let bid_hash_str = bytes16_to_hex(event.bid_id);
        let offer_hash_str = bytes16_to_hex(event.offer_id);
        let residual_bid_id = bytes16_to_optional_hex(event.residual_bid_id);
        let residual_offer_id = bytes16_to_optional_hex(event.residual_offer_id);
        let bid_status = if residual_bid_id.is_some() {
            OrderStatus::PartiallyFilled
        } else {
            OrderStatus::Filled
        };
        let offer_status = if residual_offer_id.is_some() {
            OrderStatus::PartiallyFilled
        } else {
            OrderStatus::Filled
        };

        let bid_bson = mongodb::bson::to_bson(&bid_hash_str).unwrap();
        let offer_bson = mongodb::bson::to_bson(&offer_hash_str).unwrap();

        let trade = DbTradeSchema {
            trade_uuid: trade_hash.clone(),
            status: TradeStatus::Settled,
            seller: bytes16_to_hex(event.seller_id),
            buyer: bytes16_to_hex(event.buyer_id),
            market_id: bytes16_to_hex(event.market_id),
            creation_time: chrono::Utc::now().timestamp() as u64,
            offer_hash: offer_hash_str,
            bid_hash: bid_hash_str,
            residual_offer_id,
            residual_bid_id,
            parameters: TradeParameters {
                selected_energy_kWh: energy_f64,
                energy_rate: price_f64,
            },
            status_updated_at: None,
        };

        self.db
            .trades()
            .insert_trades(vec![trade.clone()])
            .await
            .with_context(|| format!("Failed to insert trade {} into DB", trade_hash))?;

        self.db
            .orders()
            .update_order_status_by_id(&bid_bson, bid_status, block_timestamp)
            .await
            .with_context(|| {
                format!(
                    "Failed to update bid {} after settlement of trade {}",
                    bid_bson, trade_hash
                )
            })?;
        self.db
            .orders()
            .update_order_status_by_id(&offer_bson, offer_status, block_timestamp)
            .await
            .with_context(|| {
                format!(
                    "Failed to update offer {} after settlement of trade {}",
                    offer_bson, trade_hash
                )
            })?;

        info!("Trade persisted and orders updated.");

        // Publish only once the trade and both order status updates are persisted.
        if let Some(publisher) = &self.event_publisher {
            publisher.publish_trade_event(vec![trade]);
        }

        Ok(())
    }

    async fn handle_delivery_verified(
        &self,
        event: DeliveryVerifiedFilter,
        block_timestamp: u64,
    ) -> Result<()> {
        let trade_id = bytes16_to_hex(event.trade_id);
        let bid_id = bytes16_to_hex(event.bid_id);
        let offer_id = bytes16_to_hex(event.offer_id);
        let trade = self
            .db
            .trades()
            .find_one(mongodb::bson::doc! {"trade_uuid": &trade_id})
            .await?
            .with_context(|| {
                format!(
                    "Verification received before trade {} was indexed",
                    trade_id
                )
            })?;
        if trade.bid_hash != bid_id || trade.offer_hash != offer_id {
            return Err(anyhow!(
                "Verification order IDs do not match trade {}",
                trade_id
            ));
        }
        anyhow::ensure!(
            matches!(trade.status, TradeStatus::Settled | TradeStatus::Executed),
            "Cannot complete unsettled trade {}",
            trade_id
        );
        // Check both orders before writing, so malformed or incomplete projections
        // cannot complete one side of a trade. Retrying partial DB writes is safe.
        for order_id in [&bid_id, &offer_id] {
            let order = self
                .db
                .orders()
                .get_order_by_id(&order_id.clone().into())
                .await?
                .with_context(|| format!("Missing consumed order {}", order_id))?;
            anyhow::ensure!(
                matches!(
                    order.status,
                    OrderStatus::Filled | OrderStatus::PartiallyFilled | OrderStatus::Executed
                ),
                "Cannot complete unfilled order {}",
                order_id
            );
        }

        for order_id in [&bid_id, &offer_id] {
            self.db
                .orders()
                .mark_executed(order_id, block_timestamp)
                .await?;
        }
        self.db
            .trades()
            .mark_executed(&trade_id, block_timestamp)
            .await?;
        info!("Indexed delivery verification for trade {}", trade_id);
        Ok(())
    }

    async fn handle_market_status(
        &self,
        event: MarketStatusUpdatedFilter,
        block_timestamp: u64,
    ) -> Result<()> {
        info!(
            "Processing EVM MarketStatus: {:?} -> Open? {}",
            hex::encode(event.market_id),
            event.is_open
        );

        if !event.is_open {
            self.db
                .orders()
                .expire_market_orders(&bytes16_to_hex(event.market_id), block_timestamp)
                .await
                .context("Failed to expire orders after market closure")?;
        }

        if let Some(publisher) = &self.event_publisher {
            publisher.publish_market_event(
                vec![EwdsMarketStatusDto {
                    market_id: bytes16_to_hex(event.market_id),
                    is_open: event.is_open,
                }],
                block_timestamp,
            );
        }

        Ok(())
    }

    async fn handle_market_clearing(
        &self,
        event: MarketClearingFilter,
        meta: LogMeta,
        block_timestamp: u64,
    ) -> Result<()> {
        info!(
            "Processing EVM MarketClearing: {:?}",
            hex::encode(event.market_id)
        );

        let clearing_status = ClearingStatus::from_evm(event.clearing_status)
            .ok_or_else(|| anyhow!("invalid clearing status byte: {}", event.clearing_status))?;

        let clearing_result = ClearingResultSchema {
            market_id: bytes16_to_hex(event.market_id),
            clearing_status,
            no_bid_reason: None,
            clearing_price: scaled_u256_to_f64(event.clearing_price),
            total_supply: scaled_u256_to_f64(event.total_supply),
            total_demand: scaled_u256_to_f64(event.total_demand),
            traded_quantity: scaled_u256_to_f64(event.traded_quantity),
            num_trades: event.num_trades,
            tx_hash: format!("{:?}", meta.transaction_hash),
            clearing_time: block_timestamp,
        };

        let market_id = clearing_result.market_id.clone();
        let clearing_result = self
            .db
            .clearing_results()
            .insert(clearing_result)
            .await
            .with_context(|| {
                format!(
                    "Failed to insert clearing result for market {} into DB",
                    market_id
                )
            })?;

        info!("Market clearing result saved.");

        if let Some(publisher) = &self.event_publisher {
            publisher.publish_clearing_result_event(vec![clearing_result]);
        }
        Ok(())
    }
}

fn bytes16_to_optional_hex(value: [u8; 16]) -> Option<String> {
    if value == [0u8; 16] {
        None
    } else {
        Some(bytes16_to_hex(value))
    }
}
