#![allow(non_snake_case, non_upper_case_globals)]

use crate::api::model;
use crate::api::types::{
    DbBidOfferMatch, DbMatchingData, MatchingBid, MatchingOffer, OrdersToMatch,
};
use async_trait::async_trait;
use gsy_offchain_primitives::algorithms::PayAsBid;
use gsy_offchain_primitives::db_api_schema::orders::{DbOrderSchema, Order};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct DbMarketData {
    pub user_id: String,
    pub market_id: String,
    pub time_slot: u64,
    pub submitted_bid_count: u64,
    pub submitted_offer_count: u64,
    pub total_matches: u64,
    pub total_matched_energy_kWh: f64,
    pub total_unmatched_energy_kWh: f64,
}

/// Bids and offers of one (market_id, time_slot), in submission order.
type SlotOrders = (Vec<MatchingBid>, Vec<MatchingOffer>);

/// Matches a batch with Pay-As-Bid per (market_id, time_slot).
///
/// The batch is grouped in one pass (each group keeps the submission order
/// of its orders, which decides ties in `energy_rate`). Returns one entry
/// per market of the batch, sorted by `market_id`, even for a market without
/// matches; within a market the matches are ordered by `time_slot`
/// ascending and then as the algorithm produced them.
pub fn match_orders(
    orders: &[DbOrderSchema],
    user_id: &str,
) -> Vec<(String, Vec<DbBidOfferMatch>)> {
    let mut groups: BTreeMap<&str, BTreeMap<u64, SlotOrders>> = BTreeMap::new();
    for order in orders {
        match &order.order {
            Order::Bid(bid) => {
                let component = &bid.bid_component;
                groups
                    .entry(component.market_id.as_str())
                    .or_default()
                    .entry(component.time_slot)
                    .or_default()
                    .0
                    .push(MatchingBid {
                        order_id: order._id.clone(),
                        bid: bid.clone(),
                    });
            }
            Order::Offer(offer) => {
                let component = &offer.offer_component;
                groups
                    .entry(component.market_id.as_str())
                    .or_default()
                    .entry(component.time_slot)
                    .or_default()
                    .1
                    .push(MatchingOffer {
                        order_id: order._id.clone(),
                        offer: offer.clone(),
                    });
            }
        }
    }

    groups
        .into_iter()
        .map(|(market_id, slots)| {
            let mut market_matches = Vec::new();
            for (_time_slot, (bids, offers)) in slots {
                if bids.is_empty() || offers.is_empty() {
                    continue;
                }
                let mut matching_data = DbMatchingData {
                    bids,
                    offers,
                    market_id: market_id.to_string(),
                    user_id: user_id.to_string(),
                };
                market_matches.extend(matching_data.pay_as_bid());
            }
            (market_id.to_string(), market_matches)
        })
        .collect()
}

#[async_trait]
pub trait MatchControllerBase: Send + Sync {
    /// Matches the orders per (market_id, time_slot) with Pay-As-Bid.
    ///
    /// With `persist` the matches are inserted into `matches` and the market
    /// statistics are upserted into `market_data`. Without it (dry run) the
    /// same matches are returned, including their freshly generated
    /// `match_id`s, but nothing is written.
    async fn process_market_id_for_pay_as_bid(
        &self,
        orders_obj: OrdersToMatch,
        persist: bool,
    ) -> mongodb::error::Result<HashMap<String, Vec<DbBidOfferMatch>>> {
        let orders = orders_obj.orders;
        let user_id = orders_obj.user_id;
        let matches_per_market = match_orders(&orders, &user_id);

        if persist {
            let all_matches_to_insert: Vec<DbBidOfferMatch> = matches_per_market
                .iter()
                .flat_map(|(_, market_matches)| market_matches.iter().cloned())
                .collect();
            let market_data_map = self
                .calculate_market_statistics(&orders, &all_matches_to_insert, user_id.clone())
                .await;
            self.insert_bid_offer_matches_to_db(all_matches_to_insert)
                .await?;
            self.update_market_statistics_to_db(market_data_map).await?;
        }
        Ok(matches_per_market.into_iter().collect())
    }

    async fn calculate_market_statistics(
        &self,
        orders: &[DbOrderSchema],
        matches: &[DbBidOfferMatch],
        user_id: String,
    ) -> HashMap<(String, u64), DbMarketData> {
        let mut total_bid_energy_kWh: HashMap<(String, u64), f64> = HashMap::new();
        let mut total_offer_energy_kWh: HashMap<(String, u64), f64> = HashMap::new();
        let mut total_matched_energy_kWh: HashMap<(String, u64), f64> = HashMap::new();

        let mut market_data_map: HashMap<(String, u64), DbMarketData> = HashMap::new();

        for order_schema in orders {
            let (market_id, time_slot, energy, is_bid) = match &order_schema.order {
                Order::Bid(bid) => (
                    bid.bid_component.market_id.clone(),
                    bid.bid_component.time_slot,
                    bid.bid_component.energy,
                    true,
                ),
                Order::Offer(offer) => (
                    offer.offer_component.market_id.clone(),
                    offer.offer_component.time_slot,
                    offer.offer_component.energy,
                    false,
                ),
            };

            let entry = market_data_map
                .entry((market_id.clone(), time_slot))
                .or_insert(DbMarketData {
                    user_id: user_id.clone(),
                    market_id: market_id.clone(),
                    time_slot,
                    submitted_bid_count: 0,
                    submitted_offer_count: 0,
                    total_matches: 0,
                    total_matched_energy_kWh: 0.0,
                    total_unmatched_energy_kWh: 0.0,
                });

            if is_bid {
                entry.submitted_bid_count += 1;
                total_bid_energy_kWh
                    .entry((market_id.clone(), time_slot))
                    .and_modify(|e| *e += energy)
                    .or_insert(energy);
            } else {
                entry.submitted_offer_count += 1;
                total_offer_energy_kWh
                    .entry((market_id.clone(), time_slot))
                    .and_modify(|e| *e += energy)
                    .or_insert(energy);
            }
        }

        for m in matches {
            if let Some(entry) = market_data_map.get_mut(&(m.market_id.clone(), m.time_slot)) {
                entry.total_matches += 1;
            }
            total_matched_energy_kWh
                .entry((m.market_id.clone(), m.time_slot))
                .and_modify(|e| *e += m.selected_energy)
                .or_insert(m.selected_energy);
        }

        for ((market_id, time_slot), entry) in market_data_map.iter_mut() {
            let bid_energy = total_bid_energy_kWh
                .get(&(market_id.clone(), *time_slot))
                .cloned()
                .unwrap_or(0.0);
            let offer_energy = total_offer_energy_kWh
                .get(&(market_id.clone(), *time_slot))
                .cloned()
                .unwrap_or(0.0);
            let matched_energy = total_matched_energy_kWh
                .get(&(market_id.clone(), *time_slot))
                .cloned()
                .unwrap_or(0.0);

            entry.total_matched_energy_kWh = matched_energy;
            entry.total_unmatched_energy_kWh = bid_energy + offer_energy - 2.0 * matched_energy;
        }

        market_data_map
    }

    async fn insert_bid_offer_matches_to_db(
        &self,
        matches: Vec<DbBidOfferMatch>,
    ) -> mongodb::error::Result<()>;
    async fn update_market_statistics_to_db(
        &self,
        market_data_map: HashMap<(String, u64), DbMarketData>,
    ) -> mongodb::error::Result<()>;
    #[allow(clippy::too_many_arguments)]
    async fn filter_matches(
        &self,
        user_id: String,
        market_id: Option<String>,
        start_time: u64,
        end_time: u64,
        limit: Option<i64>,
        offset: Option<u64>,
        sort: model::SortOrder,
    ) -> mongodb::error::Result<Vec<DbBidOfferMatch>>;
    async fn get_market_statistics(
        &self,
        user_id: String,
        market_id: Option<String>,
        start_time: u64,
        end_time: u64,
        resolution: model::Resolution,
        tz: String,
    ) -> mongodb::error::Result<model::MarketStatisticsResponse>;
    async fn get_markets(&self, user_id: String) -> mongodb::error::Result<Vec<String>>;
}

pub struct MatchController {
    pub db: mongodb::Database,
}

#[async_trait]
impl MatchControllerBase for MatchController {
    async fn insert_bid_offer_matches_to_db(
        &self,
        matches: Vec<DbBidOfferMatch>,
    ) -> mongodb::error::Result<()> {
        let model = model::MatchModel::from_database(self.db.clone());
        model.insert_matches(matches).await
    }

    async fn update_market_statistics_to_db(
        &self,
        market_data_map: HashMap<(String, u64), DbMarketData>,
    ) -> mongodb::error::Result<()> {
        let model = model::MatchModel::from_database(self.db.clone());
        let market_data_list: Vec<DbMarketData> = market_data_map.into_values().collect();
        model.upsert_market_data(market_data_list).await
    }

    #[allow(clippy::too_many_arguments)]
    async fn filter_matches(
        &self,
        user_id: String,
        market_id: Option<String>,
        start_time: u64,
        end_time: u64,
        limit: Option<i64>,
        offset: Option<u64>,
        sort: model::SortOrder,
    ) -> mongodb::error::Result<Vec<DbBidOfferMatch>> {
        let model = model::MatchModel::from_database(self.db.clone());
        model
            .get_matches(
                start_time, end_time, user_id, market_id, limit, offset, sort,
            )
            .await
    }

    async fn get_market_statistics(
        &self,
        user_id: String,
        market_id: Option<String>,
        start_time: u64,
        end_time: u64,
        resolution: model::Resolution,
        tz: String,
    ) -> mongodb::error::Result<model::MarketStatisticsResponse> {
        let model = model::MatchModel::from_database(self.db.clone());

        // Time series, bucketed by MongoDB in the time zone `tz`.
        let average_trade_rate_timeseries = model
            .get_average_energy_rate_series(
                user_id.clone(),
                market_id.clone(),
                start_time,
                end_time,
                resolution,
                &tz,
            )
            .await?;
        let energy_timeseries = model
            .get_energy_series(
                user_id.clone(),
                market_id.clone(),
                start_time,
                end_time,
                resolution,
                &tz,
            )
            .await?;

        // Window totals.
        let totals = model
            .get_market_data_totals(user_id.clone(), market_id.clone(), start_time, end_time)
            .await?;
        let active_traders = model
            .count_active_traders(user_id, market_id, start_time, end_time)
            .await?;

        let total_energy = totals.total_matched_energy_kWh + totals.total_unmatched_energy_kWh;
        let success_rate = if total_energy > 0.0 {
            totals.total_matched_energy_kWh / total_energy
        } else {
            0.0
        };

        Ok(model::MarketStatisticsResponse {
            average_trade_rate_timeseries,
            energy_timeseries,
            total_matches: totals.total_matches,
            success_rate,
            active_traders,
            markets: totals.markets,
        })
    }

    async fn get_markets(&self, user_id: String) -> mongodb::error::Result<Vec<String>> {
        let model = model::MatchModel::from_database(self.db.clone());
        model.get_markets(user_id).await
    }
}
