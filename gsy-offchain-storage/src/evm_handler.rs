use crate::db::DatabaseWrapper;
use crate::ewds_event_handler::EwdsEventPublisher;
use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use ethers::contract::LogMeta;
use ethers::types::U256;
use futures::TryStreamExt;
use gsy_ethers_listener::{
    DeliveryVerifiedFilter, GsyEventHandler, MarketClearingFilter, NewMarketCreatedFilter,
    OrderCancelledFilter, OrderPlacedFilter, PenaltyRecordedFilter, TradeSettledFilter,
};
use mongodb::bson::{self, doc};
use primitives::db_api_schema::{
    market::{MarketChainRecord, MarketSchema},
    orders::{
        order_metadata_from_contract, ContractOrderMetadata, DbOrderSchema, OrderEnum, OrderStatus,
    },
    trades::{ClearingResultSchema, ClearingStatus, DbTradeSchema, TradeParameters, TradeStatus},
};
use primitives::ewds::dto::EwdsMarketDto;
use primitives::utils::{
    bytes16_to_hex, timestamp_to_string_with_padding, NODE_FLOAT_SCALING_FACTOR,
};
use tracing::info;

fn scaled_u256_to_f64(value: U256) -> f64 {
    value.as_u128() as f64 / NODE_FLOAT_SCALING_FACTOR
}

pub struct OffchainStorageEvmHandler {
    pub db: DatabaseWrapper,
    pub event_publisher: Option<EwdsEventPublisher>,
}

impl OffchainStorageEvmHandler {
    /// Read back persisted snapshots before publishing; HTTP-only operation stays local.
    async fn publish_order_statuses(&self, order_ids: &[String]) -> Result<()> {
        if let Some(publisher) = &self.event_publisher {
            let mut orders = Vec::with_capacity(order_ids.len());
            for order_id in order_ids {
                let order = self
                    .db
                    .orders()
                    .get_order_by_id(&order_id.clone().into())
                    .await?
                    .with_context(|| {
                        format!("Missing order {} for status notification", order_id)
                    })?;
                orders.push(order);
            }
            if !orders.is_empty() {
                publisher.publish_order_status_event(orders);
            }
        }
        Ok(())
    }
}

#[async_trait]
impl GsyEventHandler for OffchainStorageEvmHandler {
    async fn handle_block(&self, block_timestamp: u64) -> Result<()> {
        let submitted = bson::to_bson(&OrderStatus::Submitted)?;
        let market_ids = self
            .db
            .orders()
            .distinct("market_id", doc! {"status": &submitted})
            .await?;
        if market_ids.is_empty() {
            return Ok(());
        }
        let markets: Vec<MarketSchema> = self
            .db
            .markets()
            .find(doc! {
                "market_id": {"$in": market_ids},
                "closing_time": {"$lte": timestamp_to_string_with_padding(block_timestamp)},
            })
            .await?
            .try_collect()
            .await?;
        for market in markets {
            let order_ids = self
                .db
                .orders()
                .distinct(
                    "order_id",
                    doc! {
                        "market_id": &market.market_id, "status": &submitted,
                    },
                )
                .await?
                .into_iter()
                .filter_map(|id| id.as_str().map(str::to_owned))
                .collect::<Vec<_>>();
            // The deadline is immutable and remains stable even if indexing is delayed.
            self.db
                .orders()
                .expire_market_orders(&market.market_id, market.closing_time.parse()?)
                .await?;
            self.publish_order_statuses(&order_ids).await?;
        }
        Ok(())
    }

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
        self.publish_order_statuses(&[order_id_str]).await?;
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
        let order_id = bytes16_to_hex(event.order_id);
        let id_bson = mongodb::bson::to_bson(&order_id)?;

        let updated = self
            .db
            .orders()
            .update_order_status_by_id(&id_bson, OrderStatus::Cancelled, block_timestamp)
            .await
            .context("Failed to persist order cancellation")?;
        anyhow::ensure!(
            updated.matched_count == 1,
            "Missing cancelled order {}",
            order_id
        );
        self.publish_order_statuses(&[order_id]).await?;
        info!("Successfully marked order as cancelled");
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
        // TradeSettled confirms the match on-chain, not financial settlement.
        let trade = DbTradeSchema {
            trade_uuid: trade_hash.clone(),
            status: TradeStatus::Matched,
            seller: bytes16_to_hex(event.seller_id),
            buyer: bytes16_to_hex(event.buyer_id),
            market_id: bytes16_to_hex(event.market_id),
            creation_time: block_timestamp,
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
            .record_settlement(trade.clone(), block_timestamp)
            .await
            .with_context(|| format!("Failed to index trade {} into DB", trade_hash))?;

        for (order_id, partial) in [
            (&trade.bid_hash, trade.residual_bid_id.is_some()),
            (&trade.offer_hash, trade.residual_offer_id.is_some()),
        ] {
            self.db
                .orders()
                .mark_filled(order_id, partial, block_timestamp)
                .await
                .with_context(|| {
                    format!(
                        "Failed to update order {} after settlement of trade {}",
                        order_id, trade_hash
                    )
                })?;
        }

        info!("Trade persisted and orders updated.");

        // Publish only once the trade and both order status updates are persisted.
        self.publish_order_statuses(&[trade.bid_hash.clone(), trade.offer_hash.clone()])
            .await?;
        if let Some(publisher) = &self.event_publisher {
            publisher
                .publish_stored_trades(&self.db, &[trade_hash])
                .await?;
        }

        Ok(())
    }

    async fn handle_penalty_recorded(
        &self,
        event: PenaltyRecordedFilter,
        block_timestamp: u64,
    ) -> Result<()> {
        let trade_id = bytes16_to_hex(event.trade_id);
        let actor_id = bytes16_to_hex(event.penalized_actor_id);
        let trade = self
            .db
            .trades()
            .find_one(mongodb::bson::doc! {"trade_uuid": &trade_id})
            .await?
            .with_context(|| format!("Penalty received before trade {} was indexed", trade_id))?;
        anyhow::ensure!(
            event.penalty_energy > 0
                && trade.market_id == bytes16_to_hex(event.market_id)
                && (trade.buyer == actor_id || trade.seller == actor_id),
            "Penalty does not match trade {}",
            trade_id
        );
        self.db
            .trades()
            .mark_penalized(&trade_id, block_timestamp)
            .await?;
        info!("Indexed penalty for trade {}", trade_id);
        if let Some(publisher) = &self.event_publisher {
            publisher
                .publish_stored_trades(&self.db, &[trade_id])
                .await?;
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
            matches!(
                trade.status,
                TradeStatus::Matched | TradeStatus::Executed | TradeStatus::Penalized
            ),
            "Cannot complete unconfirmed trade {}",
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
        self.publish_order_statuses(&[bid_id, offer_id]).await?;
        if let Some(publisher) = &self.event_publisher {
            publisher
                .publish_stored_trades(&self.db, &[trade_id])
                .await?;
        }
        Ok(())
    }

    async fn handle_new_market_created(&self, event: NewMarketCreatedFilter) -> Result<()> {
        info!(
            "Processing EVM NewMarketCreated: {:?}",
            hex::encode(event.market_id)
        );

        // Upsert by market_id so a replayed event is harmless.
        let market = MarketSchema::try_from(MarketChainRecord {
            market_id: event.market_id,
            community_id: event.community_id,
            opening_time: event.opening_time,
            closing_time: event.closing_time,
            delivery_start_time: event.delivery_start_time,
            delivery_end_time: event.delivery_end_time,
            market_type: event.market_type,
            matching_algorithm: event.matching_algorithm,
            created_at: event.created_at,
        })?;
        let market = self.db.markets().upsert(market).await?;

        info!("Market persisted.");

        // Publish only once the market is persisted.
        if let Some(publisher) = &self.event_publisher {
            let market = EwdsMarketDto::try_from(market)
                .context("Failed to convert the market for its market.event")?;
            publisher.publish_market_event(vec![market], event.created_at);
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
