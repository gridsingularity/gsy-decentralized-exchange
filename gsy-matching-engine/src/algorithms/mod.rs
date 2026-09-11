mod pay_as_bid;
mod pay_as_clear;

use crate::models::{BidOfferMatch, MatchingData, Order};
use primitives::MatchingAlgorithm;
use std::collections::HashMap;
use uuid::Uuid;

pub trait PayAsBid {
    type Output;

    fn pay_as_bid(&mut self) -> Vec<Self::Output>;
}

pub trait PayAsClear {
    type Output;

    fn pay_as_clear(&mut self) -> Vec<Self::Output>;
}

pub trait MatchOrders {
    fn match_orders(&self, matching_data: &mut MatchingData) -> Result<Vec<BidOfferMatch>, String>;
}

impl MatchOrders for MatchingAlgorithm {
    fn match_orders(&self, matching_data: &mut MatchingData) -> Result<Vec<BidOfferMatch>, String> {
        match self {
            MatchingAlgorithm::PayAsBid => Ok(matching_data.pay_as_bid()),
            MatchingAlgorithm::PayAsClear => Ok(matching_data.pay_as_clear()),
            MatchingAlgorithm::AMM => {
                Err("Matching algorithm 'amm' is not implemented".to_string())
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ClearingPoint {
    traded_energy: u64,
    clearing_price: u64,
}

impl MatchingData {
    fn match_preferences(
        &self,
        bids: Vec<Order>,
        offers: Vec<Order>,
    ) -> (Vec<BidOfferMatch>, Vec<Order>, Vec<Order>) {
        let mut matches = Vec::new();
        let mut bid_matched_amounts: HashMap<String, u64> = HashMap::new();
        let mut offer_matched_amounts: HashMap<String, u64> = HashMap::new();

        let preference_bids = bids.iter().filter(|bid| {
            bid.requirements
                .as_ref()
                .and_then(|requirements| requirements.trading_partner_id.as_ref())
                .is_some()
        });

        for bid in preference_bids {
            let requirements = bid
                .requirements
                .as_ref()
                .expect("requirements checked above");
            let partner_id = requirements
                .trading_partner_id
                .as_ref()
                .expect("trading partner checked above");

            for offer in offers
                .iter()
                .filter(|offer| offer.created_by == *partner_id)
            {
                let preferred_rate = requirements
                    .preferred_energy_rate
                    .unwrap_or(bid.energy_rate);
                if preferred_rate < offer.energy_rate || preferred_rate > bid.energy_rate {
                    continue;
                }

                let bid_amount_used = bid_matched_amounts
                    .get(&bid.order_id)
                    .copied()
                    .unwrap_or_default();
                let offer_amount_used = offer_matched_amounts
                    .get(&offer.order_id)
                    .copied()
                    .unwrap_or_default();
                let selected_energy = bid
                    .energy
                    .saturating_sub(bid_amount_used)
                    .min(offer.energy.saturating_sub(offer_amount_used));

                if selected_energy == 0 {
                    continue;
                }

                matches.push(BidOfferMatch {
                    market_id: offer.market_id.clone(),
                    time_slot: offer.time_slot,
                    bid: bid.clone(),
                    offer: offer.clone(),
                    residual_bid: None,
                    residual_offer: None,
                    selected_energy,
                    energy_rate: preferred_rate,
                });

                *bid_matched_amounts.entry(bid.order_id.clone()).or_default() += selected_energy;
                *offer_matched_amounts
                    .entry(offer.order_id.clone())
                    .or_default() += selected_energy;

                if bid.energy
                    == bid_matched_amounts
                        .get(&bid.order_id)
                        .copied()
                        .unwrap_or_default()
                {
                    break;
                }
            }
        }

        let remaining_bids = bids
            .iter()
            .filter_map(|bid| {
                residual_order(
                    bid,
                    bid_matched_amounts
                        .get(&bid.order_id)
                        .copied()
                        .unwrap_or_default(),
                )
            })
            .collect();
        let remaining_offers = offers
            .iter()
            .filter_map(|offer| {
                residual_order(
                    offer,
                    offer_matched_amounts
                        .get(&offer.order_id)
                        .copied()
                        .unwrap_or_default(),
                )
            })
            .collect();

        (matches, remaining_bids, remaining_offers)
    }

    fn match_standard_at_clearing_point(
        &self,
        mut bids: Vec<Order>,
        mut offers: Vec<Order>,
        clearing_point: Option<ClearingPoint>,
    ) -> Vec<BidOfferMatch> {
        let mut matches = Vec::new();
        let mut remaining_clearing_energy = clearing_point
            .map(|point| point.traded_energy)
            .unwrap_or(u64::MAX);

        bids.sort_by(|left, right| right.energy_rate.cmp(&left.energy_rate));
        offers.sort_by(|left, right| left.energy_rate.cmp(&right.energy_rate));

        let mut available_bid_energy = bids
            .iter()
            .map(|bid| (bid.order_id.clone(), bid.energy))
            .collect::<HashMap<_, _>>();
        let mut available_offer_energy = offers
            .iter()
            .map(|offer| (offer.order_id.clone(), offer.energy))
            .collect::<HashMap<_, _>>();

        for offer in &offers {
            for bid in &bids {
                if remaining_clearing_energy == 0 {
                    return matches;
                }

                if offer.area_uuid == bid.area_uuid || offer.energy_rate > bid.energy_rate {
                    continue;
                }

                if let Some(point) = clearing_point {
                    if bid.energy_rate < point.clearing_price
                        || offer.energy_rate > point.clearing_price
                    {
                        continue;
                    }
                }

                let offer_energy = available_offer_energy
                    .get(&offer.order_id)
                    .copied()
                    .unwrap_or_default();
                let bid_energy = available_bid_energy
                    .get(&bid.order_id)
                    .copied()
                    .unwrap_or_default();

                if offer_energy == 0 || bid_energy == 0 {
                    continue;
                }

                let selected_energy = offer_energy.min(bid_energy).min(remaining_clearing_energy);
                let remaining_bid_energy = bid_energy - selected_energy;
                let remaining_offer_energy = offer_energy - selected_energy;
                available_bid_energy.insert(bid.order_id.clone(), remaining_bid_energy);
                available_offer_energy.insert(offer.order_id.clone(), remaining_offer_energy);
                remaining_clearing_energy -= selected_energy;

                matches.push(BidOfferMatch {
                    market_id: offer.market_id.clone(),
                    time_slot: offer.time_slot,
                    bid: bid.clone(),
                    offer: offer.clone(),
                    residual_bid: residual_order(bid, bid.energy - remaining_bid_energy),
                    residual_offer: residual_order(offer, offer.energy - remaining_offer_energy),
                    selected_energy,
                    energy_rate: clearing_point
                        .map(|point| point.clearing_price)
                        .unwrap_or(bid.energy_rate),
                });
            }
        }

        matches
    }
}

fn residual_order(order: &Order, matched_energy: u64) -> Option<Order> {
    if order.energy <= matched_energy {
        return None;
    }

    let mut residual = order.clone();
    residual.energy -= matched_energy;
    if matched_energy > 0 {
        residual.order_id = Uuid::new_v4().to_string();
    }
    Some(residual)
}
