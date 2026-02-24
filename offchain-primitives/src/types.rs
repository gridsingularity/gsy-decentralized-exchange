#![allow(non_snake_case)]

use crate::algorithms::PayAsBid;
use crate::db_api_schema::orders::{OrderEnum, OrderStatus};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use subxt::utils::{AccountId32, H256};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, PartialOrd)]
pub enum EnergyType {
    Clean,
    Battery,
    FossilFuel,
    Import,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, PartialOrd)]
pub struct Requirements {
    pub trading_partner_id: Option<AccountId32>,
    pub energy_type: Option<EnergyType>,
    pub preferred_energy_rate: Option<u64>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, PartialOrd)]
pub struct Attributes {
    pub trading_partner_id: Option<AccountId32>,
    pub energy_type: EnergyType,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, PartialOrd)]
pub struct Order {
    pub order_id: H256,
    pub order_type: OrderEnum,
    pub status: OrderStatus,
    pub area_uuid: H256,
    pub market_id: H256,
    pub time_slot: u64,
    pub creation_time: u64,
    pub energy: u64,
    pub energy_rate: u64,
    pub created_by: AccountId32,
    pub requirements: Option<Requirements>,
    pub attributes: Option<Attributes>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct BidOfferMatch {
    pub market_id: H256,
    pub time_slot: u64,
    pub bid: Order,
    pub offer: Order,
    pub residual_bid: Option<Order>,
    pub residual_offer: Option<Order>,
    pub selected_energy: u64,
    pub energy_rate: u64,
}

#[derive(Clone, Debug, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct MatchingData {
    pub bids: Vec<Order>,
    pub offers: Vec<Order>,
    pub market_id: H256,
}

impl MatchingData {
    fn match_preferences(&self) -> (Vec<BidOfferMatch>, Vec<Order>, Vec<Order>) {
        let mut matches = Vec::new();

        type OrderKey = H256;

        let mut bid_matched_amounts: HashMap<OrderKey, u64> = HashMap::new();
        let mut offer_matched_amounts: HashMap<OrderKey, u64> = HashMap::new();

        let (preference_bids, _non_preference_bids): (Vec<&Order>, Vec<&Order>) =
            self.bids.iter().partition(|b| {
                b.requirements
                    .as_ref()
                    .and_then(|r| r.trading_partner_id.as_ref())
                    .is_some()
            });

        for bid in preference_bids {
            let req = bid.requirements.as_ref().unwrap();
            let partner_id = req.trading_partner_id.as_ref().unwrap();
            let bid_key = bid.order_id;

            let partner_offers: Vec<&Order> = self
                .offers
                .iter()
                .filter(|o| &o.created_by == partner_id)
                .collect();

            for offer in partner_offers {
                let offer_key = offer.order_id;

                let bid_amount_used = *bid_matched_amounts.get(&bid_key).unwrap_or(&0);
                let offer_amount_used = *offer_matched_amounts.get(&offer_key).unwrap_or(&0);

                let bid_energy_needed = bid.energy.saturating_sub(bid_amount_used);
                let offer_energy_available = offer.energy.saturating_sub(offer_amount_used);

                let selected_energy = bid_energy_needed.min(offer_energy_available);

                if selected_energy > 0 {
                    let trade_rate = req.preferred_energy_rate.unwrap_or(bid.energy_rate);

                    matches.push(BidOfferMatch {
                        market_id: offer.market_id,
                        time_slot: offer.time_slot,
                        bid: bid.clone(),
                        offer: offer.clone(),
                        residual_bid: None,
                        residual_offer: None,
                        selected_energy: selected_energy,
                        energy_rate: trade_rate,
                    });

                    *bid_matched_amounts.entry(bid_key.clone()).or_insert(0) += selected_energy;
                    *offer_matched_amounts.entry(offer_key).or_insert(0) += selected_energy;

                    if bid
                        .energy
                        .saturating_sub(*bid_matched_amounts.get(&bid_key).unwrap_or(&0))
                        == 0
                    {
                        break;
                    }
                }
            }
        }

        let mut remaining_bids = Vec::new();

        for bid in self.bids.iter() {
            let has_reqs = bid
                .requirements
                .as_ref()
                .and_then(|r| r.trading_partner_id.as_ref())
                .is_some();

            if has_reqs {
                let bid_key = bid.order_id;
                let matched_amount = *bid_matched_amounts.get(&bid_key).unwrap_or(&0);

                if bid.energy > matched_amount {
                    let mut residual_bid = bid.clone();
                    residual_bid.energy -= matched_amount;
                    if matched_amount > 0 {
                        residual_bid.order_id = H256::random();
                    }
                    remaining_bids.push(residual_bid);
                }
            } else {
                remaining_bids.push(bid.clone());
            }
        }

        let mut remaining_offers = Vec::new();
        for offer in self.offers.iter() {
            let offer_key = offer.order_id;
            let matched_amount = *offer_matched_amounts.get(&offer_key).unwrap_or(&0);

            if offer.energy > matched_amount {
                let mut residual_offer = offer.clone();
                residual_offer.energy -= matched_amount;
                if matched_amount > 0 {
                    residual_offer.order_id = H256::random();
                }
                remaining_offers.push(residual_offer);
            }
        }

        (matches, remaining_bids, remaining_offers)
    }

    fn match_standard(&self, mut bids: Vec<Order>, mut offers: Vec<Order>) -> Vec<BidOfferMatch> {
        let mut matches = Vec::new();

        bids.sort_by(|a, b| b.energy_rate.cmp(&a.energy_rate));
        offers.sort_by(|a, b| a.energy_rate.cmp(&b.energy_rate));

        type OrderKey = H256;
        let mut available_energy_bid: HashMap<OrderKey, u64> = HashMap::new();
        let mut available_energy_offer: HashMap<OrderKey, u64> = HashMap::new();

        for b in &bids {
            available_energy_bid.insert(b.order_id, b.energy);
        }
        for o in &offers {
            available_energy_offer.insert(o.order_id, o.energy);
        }

        for offer in &mut offers {
            for bid in &mut bids {
                if offer.area_uuid == bid.area_uuid {
                    continue;
                }

                if offer.energy_rate > bid.energy_rate {
                    continue;
                }

                let bid_key = bid.order_id;
                let offer_key = offer.order_id;

                let offer_energy = *available_energy_offer.get(&offer_key).unwrap_or(&0);
                let bid_energy = *available_energy_bid.get(&bid_key).unwrap_or(&0);

                if offer_energy == 0 || bid_energy == 0 {
                    continue;
                }

                let selected_energy = offer_energy.min(bid_energy);

                available_energy_bid.insert(bid_key.clone(), bid_energy - selected_energy);
                available_energy_offer.insert(offer_key.clone(), offer_energy - selected_energy);

                let residual_bid = if bid_energy > selected_energy {
                    Some(Order {
                        order_id: H256::random(),
                        energy: bid_energy - selected_energy,
                        ..bid.clone()
                    })
                } else {
                    None
                };

                let residual_offer = if offer_energy > selected_energy {
                    Some(Order {
                        order_id: H256::random(),
                        energy: offer_energy - selected_energy,
                        ..offer.clone()
                    })
                } else {
                    None
                };

                let new_bid_offer_match = BidOfferMatch {
                    market_id: offer.market_id,
                    time_slot: offer.time_slot,
                    bid: bid.clone(),
                    offer: offer.clone(),
                    residual_bid,
                    residual_offer,
                    selected_energy: selected_energy,
                    energy_rate: bid.energy_rate,
                };

                matches.push(new_bid_offer_match);
            }
        }
        matches
    }
}

impl PayAsBid for MatchingData {
    type Output = BidOfferMatch;

    fn pay_as_bid(&mut self) -> Vec<Self::Output> {
        let mut all_matches = Vec::new();

        let (pref_matches, remaining_bids, remaining_offers) = self.match_preferences();
        all_matches.extend(pref_matches);

        let standard_matches = self.match_standard(remaining_bids, remaining_offers);
        all_matches.extend(standard_matches);

        all_matches
    }
}
