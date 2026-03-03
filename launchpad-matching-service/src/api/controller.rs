use std::collections::HashMap;
use gsy_offchain_primitives::algorithms::PayAsBid;
use crate::api::types::{DbBidOfferMatch, DbMatchingData};
use gsy_offchain_primitives::db_api_schema::orders::{DbOrderSchema, Order, DbBid, DbOffer};

pub fn process_market_id_for_pay_as_bid(
        orders: Vec<DbOrderSchema>) -> HashMap<String, Vec<DbBidOfferMatch>> {
    let mut matches = HashMap::new();

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
        matches.insert(market_id.clone(), algorithm_result);
    }
    matches
}