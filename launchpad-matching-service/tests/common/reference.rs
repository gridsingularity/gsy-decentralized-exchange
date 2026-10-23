//! Verbatim copy of the matching code before MS-10 (reference for the golden
//! test and the benchmark). Do not optimize or fix this file: it must keep
//! the old behaviour, including keying the remaining energy by the hash of
//! the serialized order.
//!
//! Copied from `src/api/types.rs` (`impl PayAsBid for DbMatchingData`, here
//! on the local type `ReferenceMatchingData`) and from
//! `src/api/controller.rs` (`process_market_id_for_pay_as_bid`, here the
//! synchronous `reference_match_orders` without the database writes).

use gsy_offchain_primitives::algorithms::PayAsBid;
use gsy_offchain_primitives::db_api_schema::orders::{DbBid, DbOffer, DbOrderComponent, Order};
use launchpad_matching_service::api::types::{
    DbBidOfferMatch, MatchingBid, MatchingOffer, OrdersToMatch,
};
use mongodb::bson::oid::ObjectId;
use std::collections::{BTreeSet, HashMap, HashSet};
use subxt::config::{Hasher, substrate::BlakeTwo256};
use subxt::utils::H256;

const EPS: f64 = 0.000001;

#[derive(Clone, Debug, PartialEq, PartialOrd)]
pub struct ReferenceMatchingData {
    pub bids: Vec<MatchingBid>,
    pub offers: Vec<MatchingOffer>,
    pub market_id: String,
    pub user_id: String,
}

impl PayAsBid for ReferenceMatchingData {
    type Output = DbBidOfferMatch;

    fn pay_as_bid(&mut self) -> Vec<Self::Output> {
        let mut bid_offer_pairs = Vec::new();

        let mut bids = self.bids.clone();
        let mut offers = self.offers.clone();

        // Sorting bids in descending order based on energy rate.
        // b is compared with a, therefore the sorting order is descending.
        bids.sort_by(|a, b| {
            b.bid
                .bid_component
                .energy_rate
                .partial_cmp(&a.bid.bid_component.energy_rate)
                .unwrap()
        });
        // Sorting offers in ascending order based on energy rate.
        // a is compared with b, therefore the sorting order is ascending.
        offers.sort_by(|a, b| {
            a.offer
                .offer_component
                .energy_rate
                .partial_cmp(&b.offer.offer_component.energy_rate)
                .unwrap()
        });

        let mut available_order_energy: HashMap<H256, f64> = HashMap::new();

        for matching_offer in &offers {
            for matching_bid in &bids {
                // The remaining-energy bookkeeping below is keyed by the hash
                // of the serialized DbBid / DbOffer, exactly as before the
                // order ids were carried along.
                let offer = &matching_offer.offer;
                let bid = &matching_bid.bid;
                if offer.offer_component.area_uuid == bid.bid_component.area_uuid
                    || offer.offer_component.energy < EPS
                    || bid.bid_component.energy < EPS
                {
                    continue;
                }

                if offer.offer_component.energy_rate > bid.bid_component.energy_rate {
                    continue;
                }

                let serialized_bid =
                    serde_json::to_string(bid).expect("Failed to serialize DbBid to JSON String");
                let serialized_offer = serde_json::to_string(offer)
                    .expect("Failed to serialize DbOffer to JSON String");
                let bid_id = BlakeTwo256.hash(&serialized_bid.into_bytes());
                let offer_id = BlakeTwo256.hash(&serialized_offer.into_bytes());

                let offer_energy = *available_order_energy
                    .entry(offer_id.clone())
                    .or_insert(offer.offer_component.energy);
                let bid_energy = *available_order_energy
                    .entry(bid_id.clone())
                    .or_insert(bid.bid_component.energy);

                let selected_energy = offer_energy.min(bid_energy);

                if selected_energy < EPS {
                    continue;
                }

                available_order_energy.insert(bid_id, bid_energy - selected_energy);
                available_order_energy.insert(offer_id, offer_energy - selected_energy);

                let residual_bid = if bid_energy > selected_energy {
                    Some(DbBid {
                        nonce: bid.nonce.wrapping_add(1),
                        bid_component: DbOrderComponent {
                            energy: bid_energy - selected_energy,
                            ..bid.bid_component.clone()
                        },
                        ..bid.clone()
                    })
                } else {
                    None
                };

                let residual_offer = if offer_energy > selected_energy {
                    Some(DbOffer {
                        nonce: offer.nonce.wrapping_add(1),
                        offer_component: DbOrderComponent {
                            energy: offer_energy - selected_energy,
                            ..offer.offer_component.clone()
                        },
                        ..offer.clone()
                    })
                } else {
                    None
                };

                let new_bid_offer_match = DbBidOfferMatch {
                    match_id: ObjectId::new().to_hex(),
                    user_id: self.user_id.clone(),
                    market_id: offer.offer_component.market_id.clone(),
                    time_slot: offer.offer_component.time_slot,
                    bid_order_id: matching_bid.order_id.clone(),
                    offer_order_id: matching_offer.order_id.clone(),
                    bid: bid.clone(),
                    offer: offer.clone(),
                    residual_bid,
                    residual_offer,
                    selected_energy,
                    energy_rate: bid.bid_component.energy_rate,
                };

                bid_offer_pairs.push(new_bid_offer_match);
            }
        }
        bid_offer_pairs
    }
}

/// The old `process_market_id_for_pay_as_bid` without persistence. Returns
/// the response map and the list of all matches in insertion order.
pub fn reference_match_orders(
    orders_obj: OrdersToMatch,
) -> (HashMap<String, Vec<DbBidOfferMatch>>, Vec<DbBidOfferMatch>) {
    let orders = orders_obj.orders;
    let user_id = orders_obj.user_id;
    let mut matches: HashMap<String, Vec<DbBidOfferMatch>> = HashMap::new();
    let mut all_matches_to_insert = Vec::new();

    // Find all market ids in the orders
    let market_ids: HashSet<String> = orders
        .iter()
        .map(|order| match &order.order {
            Order::Bid(bid) => bid.bid_component.market_id.clone(),
            Order::Offer(offer) => offer.offer_component.market_id.clone(),
        })
        .collect();

    // Sorted, so the matches of a market come out ordered by time slot.
    let time_slots: BTreeSet<u64> = orders
        .iter()
        .map(|order| match &order.order {
            Order::Bid(bid) => bid.bid_component.time_slot,
            Order::Offer(offer) => offer.offer_component.time_slot,
        })
        .collect();

    for market_id in market_ids.iter() {
        // Every market of the batch gets a key, even without matches.
        let market_matches = matches.entry(market_id.clone()).or_default();
        for time_slot in time_slots.iter() {
            let bids_list: Vec<MatchingBid> = orders
                .iter()
                .filter_map(|order| match &order.order {
                    Order::Bid(bid)
                        if bid.bid_component.market_id == *market_id
                            && bid.bid_component.time_slot == *time_slot =>
                    {
                        Some(MatchingBid {
                            order_id: order._id.clone(),
                            bid: bid.clone(),
                        })
                    }
                    _ => None,
                })
                .collect();
            let offers_list: Vec<MatchingOffer> = orders
                .iter()
                .filter_map(|order| match &order.order {
                    Order::Offer(offer)
                        if offer.offer_component.market_id == *market_id
                            && offer.offer_component.time_slot == *time_slot =>
                    {
                        Some(MatchingOffer {
                            order_id: order._id.clone(),
                            offer: offer.clone(),
                        })
                    }
                    _ => None,
                })
                .collect();
            let mut matching_data = ReferenceMatchingData {
                bids: bids_list,
                offers: offers_list,
                market_id: market_id.clone(),
                user_id: user_id.clone(),
            };
            let algorithm_result = matching_data.pay_as_bid();
            all_matches_to_insert.extend(algorithm_result.clone());
            market_matches.extend(algorithm_result);
        }
    }
    (matches, all_matches_to_insert)
}
