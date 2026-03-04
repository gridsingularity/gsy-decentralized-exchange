#![allow(non_snake_case, non_upper_case_globals)]

use serde::{Serialize, Deserialize};
use async_trait::async_trait;
use std::collections::HashMap;
use gsy_offchain_primitives::algorithms::PayAsBid;
use crate::api::types::{DbBidOfferMatch, DbMatchingData};
use gsy_offchain_primitives::db_api_schema::orders::{DbOrderSchema, Order, DbBid, DbOffer};
use crate::api::model;


#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct DbMarketData {
    pub market_id: String,
    pub time_slot: u64,
    pub submitted_bid_count: u64,
    pub submitted_offer_count: u64,
    pub total_matches: u64,
    pub total_matched_energy_kWh: f64,
    pub total_unmatched_energy_kWh: f64,
}

#[async_trait]
pub trait MatchControllerBase: Send + Sync {
    async fn process_market_id_for_pay_as_bid(
            &self, orders: Vec<DbOrderSchema>) -> HashMap<String, Vec<DbBidOfferMatch>> {
        let mut matches = HashMap::new();
        let mut all_matches_to_insert = Vec::new();

        // Find all market ids in the orders
        let market_ids: Vec<String> = orders.iter().map(|order| {
            match order.order.clone() {
                Order::Bid(bid) => bid.bid_component.market_id.clone(),
                Order::Offer(offer) => offer.offer_component.market_id.clone(),
            }
        }).collect();

        for market_id in market_ids.iter() {
            let bids_list: Vec<DbBid> = orders.iter().filter_map(|order| {
                match &order.order {
                    Order::Bid(bid) if bid.bid_component.market_id == *market_id => Some(bid.clone()),
                    _ => None,
                }
            }).collect();
            let offers_list: Vec<DbOffer> = orders.iter().filter_map(|order| {
                match &order.order {
                    Order::Offer(offer) if offer.offer_component.market_id == *market_id => Some(offer.clone()),
                    _ => None,
                }
            }).collect();
            let mut matching_data = DbMatchingData {
                bids: bids_list,
                offers: offers_list,
                market_id: market_id.clone(),
            };
            let algorithm_result = matching_data.pay_as_bid();
            all_matches_to_insert.extend(algorithm_result.clone());
            matches.insert(market_id.clone(), algorithm_result);
        }
        self.insert_bid_offer_matches_to_db(all_matches_to_insert.clone()).await;
        let market_data_map = self.calculate_market_statistics(&orders, &all_matches_to_insert).await;
        self.update_market_statistics(market_data_map).await;
        matches
    }

    async fn calculate_market_statistics(
        &self, orders: &[DbOrderSchema], matches: &[DbBidOfferMatch]
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

            let entry = market_data_map.entry((market_id.clone(), time_slot)).or_insert(DbMarketData {
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
            let bid_energy = total_bid_energy_kWh.get(&(market_id.clone(), *time_slot)).cloned().unwrap_or(0.0);
            let offer_energy = total_offer_energy_kWh.get(&(market_id.clone(), *time_slot)).cloned().unwrap_or(0.0);
            let matched_energy = total_matched_energy_kWh.get(&(market_id.clone(), *time_slot)).cloned().unwrap_or(0.0);

            entry.total_matched_energy_kWh = matched_energy;
            entry.total_unmatched_energy_kWh = bid_energy + offer_energy - 2.0 * matched_energy;
        }
        
        
        
        market_data_map
    }

    async fn insert_bid_offer_matches_to_db(&self, matches: Vec<DbBidOfferMatch>);
    async fn update_market_statistics(&self, market_data_map: HashMap<(String, u64), DbMarketData>);
}

pub struct MatchController {}

#[async_trait]
impl MatchControllerBase for MatchController {
    async fn insert_bid_offer_matches_to_db(&self, matches: Vec<DbBidOfferMatch>) {
        if let Ok(model) = model::MatchModel::new().await {
            if let Err(e) = model.insert_matches(matches).await {
                eprintln!("Failed to insert matches into MongoDB: {:?}", e);
            }
        } else {
            eprintln!("Failed to connect to MongoDB");
        }
    }

    async fn update_market_statistics(&self, market_data_map: HashMap<(String, u64), DbMarketData>) {


        if let Ok(model) = model::MatchModel::new().await {
            let market_data_list: Vec<DbMarketData> = market_data_map.into_values().collect();
            if let Err(e) = model.upsert_market_data(market_data_list).await {
                eprintln!("Failed to upsert market data into MongoDB: {:?}", e);
            }
        } else {
            eprintln!("Failed to connect to MongoDB for market data upsert");
        }
    }
}
