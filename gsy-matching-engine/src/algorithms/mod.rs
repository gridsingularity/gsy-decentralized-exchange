mod pay_as_bid;
mod pay_as_clear;

pub use pay_as_clear::PayAsClearPricing;

use crate::models::{BidOfferMatch, MatchingData, Order};
use primitives::MatchingAlgorithm;
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
    fn match_orders(&self, matching_data: &mut MatchingData) -> Result<Vec<BidOfferMatch>, String> {
        self.match_orders_with_pricing(matching_data, PayAsClearPricing::default())
    }

    fn match_orders_with_pricing(
        &self,
        matching_data: &mut MatchingData,
        pricing: PayAsClearPricing,
    ) -> Result<Vec<BidOfferMatch>, String>;
}

impl MatchOrders for MatchingAlgorithm {
    fn match_orders_with_pricing(
        &self,
        matching_data: &mut MatchingData,
        pricing: PayAsClearPricing,
    ) -> Result<Vec<BidOfferMatch>, String> {
        match self {
            MatchingAlgorithm::PayAsBid => Ok(matching_data.pay_as_bid()),
            MatchingAlgorithm::PayAsClear => Ok(matching_data.pay_as_clear_with_pricing(pricing)),
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
        mut bids: Vec<Order>,
        mut offers: Vec<Order>,
    ) -> (Vec<BidOfferMatch>, Vec<Order>, Vec<Order>) {
        let mut matches = Vec::new();
        for bid in &mut bids {
            for offer in &mut offers {
                let Some(requirements) = bid.requirements.as_ref() else {
                    continue;
                };
                if requirements.trading_partner_id.as_ref() != Some(&offer.created_by) {
                    continue;
                }
                let preferred_rate = requirements.preferred_energy_rate.unwrap_or(bid.energy_rate);
                if preferred_rate < offer.energy_rate || preferred_rate > bid.energy_rate {
                    continue;
                }

                let selected_energy = bid.energy.min(offer.energy);

                if selected_energy == 0 {
                    continue;
                }

                matches.push(fill_order_pair(bid, offer, selected_energy, preferred_rate));

                if bid.energy == 0 {
                    break;
                }
            }
        }

        let remaining_bids = bids
            .into_iter()
            .filter(|bid| bid.energy > 0)
            .collect();
        let remaining_offers = offers
            .into_iter()
            .filter(|offer| offer.energy > 0)
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

        for offer in &mut offers {
            for bid in &mut bids {
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

                if offer.energy == 0 || bid.energy == 0 {
                    continue;
                }

                let selected_energy = offer.energy.min(bid.energy).min(remaining_clearing_energy);
                remaining_clearing_energy -= selected_energy;
                let rate = clearing_point
                    .map(|point| point.clearing_price)
                    .unwrap_or(bid.energy_rate);
                matches.push(fill_order_pair(bid, offer, selected_energy, rate));
            }
        }

        matches
    }
}

fn fill_order_pair(bid: &mut Order, offer: &mut Order, energy: u64, rate: u64) -> BidOfferMatch {
    let matched = BidOfferMatch {
        market_id: offer.market_id.clone(),
        time_slot: offer.time_slot,
        bid: bid.clone(),
        offer: offer.clone(),
        residual_bid: residual_order(bid, energy),
        residual_offer: residual_order(offer, energy),
        selected_energy: energy,
        energy_rate: rate,
    };
    // The next fill must consume the residual registered by this settlement.
    match &matched.residual_bid {
        Some(residual) => *bid = residual.clone(),
        None => bid.energy = 0,
    }
    match &matched.residual_offer {
        Some(residual) => *offer = residual.clone(),
        None => offer.energy = 0,
    }
    matched
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
