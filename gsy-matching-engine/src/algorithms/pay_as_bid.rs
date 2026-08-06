use super::{ClearingPoint, PayAsBid};
use crate::models::{BidOfferMatch, MatchingData, Order};
use rand::Rng;
use std::collections::HashMap;

impl MatchingData {
    pub(super) fn orders_for_market_slot(&self) -> (Vec<Order>, Vec<Order>) {
        (
            self.bids
                .iter()
                .filter(|order| {
                    order.market_id == self.market_id && order.time_slot == self.time_slot
                })
                .cloned()
                .collect(),
            self.offers
                .iter()
                .filter(|order| {
                    order.market_id == self.market_id && order.time_slot == self.time_slot
                })
                .cloned()
                .collect(),
        )
    }

    pub(super) fn match_preferences(
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
                .filter(|offer| {
                    offer.market_id == bid.market_id
                        && offer.time_slot == bid.time_slot
                        && offer.created_by == *partner_id
                })
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

    fn match_standard(&self, bids: Vec<Order>, offers: Vec<Order>) -> Vec<BidOfferMatch> {
        self.match_standard_at_clearing_point(bids, offers, None)
    }

    pub(super) fn match_standard_at_clearing_point(
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

                if offer.area_uuid == bid.area_uuid
                    || offer.market_id != bid.market_id
                    || offer.time_slot != bid.time_slot
                    || offer.energy_rate > bid.energy_rate
                {
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

impl PayAsBid for MatchingData {
    type Output = BidOfferMatch;

    fn pay_as_bid(&mut self) -> Vec<Self::Output> {
        let (bids, offers) = self.orders_for_market_slot();
        let (mut matches, remaining_bids, remaining_offers) =
            self.match_preferences(bids, offers);
        matches.extend(self.match_standard(remaining_bids, remaining_offers));
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
        residual.order_id = random_bytes16_hex();
    }
    Some(residual)
}

fn random_bytes16_hex() -> String {
    format!("0x{:032x}", rand::thread_rng().gen::<u128>())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::Requirements;
    use primitives::db_api_schema::orders::{OrderEnum, OrderStatus};

    fn order(id: &str, order_type: OrderEnum, energy: u64, energy_rate: u64) -> Order {
        Order {
            order_id: id.to_string(),
            order_type,
            status: OrderStatus::Open,
            area_uuid: format!("area-{id}"),
            market_id: "0x00000000000000000000000000000001".to_string(),
            time_slot: 1_000,
            creation_time: 900,
            energy,
            energy_rate,
            created_by: format!("actor-{id}"),
            requirements: None,
            attributes: None,
        }
    }

    #[test]
    fn standard_match_uses_string_ids_and_creates_bytes16_residual_id() {
        let bid = order("bid", OrderEnum::Bid, 100, 50);
        let offer = order("offer", OrderEnum::Offer, 60, 40);
        let mut data = MatchingData {
            market_id: bid.market_id.clone(),
            time_slot: bid.time_slot,
            bids: vec![bid],
            offers: vec![offer],
        };

        let matches = data.pay_as_bid();

        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].selected_energy, 60);
        let residual_id = &matches[0]
            .residual_bid
            .as_ref()
            .expect("bid should be partially filled")
            .order_id;
        assert!(residual_id.starts_with("0x"));
        assert_eq!(residual_id.len(), 34);
    }

    #[test]
    fn preference_match_compares_actor_ids_without_account_wrappers() {
        let mut bid = order("bid", OrderEnum::Bid, 100, 50);
        let mut offer = order("offer", OrderEnum::Offer, 100, 40);
        offer.created_by = "0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_string();
        bid.requirements = Some(Requirements {
            trading_partner_id: Some(offer.created_by.clone()),
            energy_type: None,
            preferred_energy_rate: Some(45),
        });
        let mut data = MatchingData {
            market_id: bid.market_id.clone(),
            time_slot: bid.time_slot,
            bids: vec![bid],
            offers: vec![offer],
        };

        let matches = data.pay_as_bid();

        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].energy_rate, 45);
    }

    #[test]
    fn preference_rate_outside_order_limits_falls_back_to_standard_matching() {
        let mut bid = order("bid", OrderEnum::Bid, 100, 50);
        let mut offer = order("offer", OrderEnum::Offer, 100, 40);
        offer.created_by = "0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_string();
        bid.requirements = Some(Requirements {
            trading_partner_id: Some(offer.created_by.clone()),
            energy_type: None,
            preferred_energy_rate: Some(35),
        });
        let mut data = MatchingData {
            market_id: bid.market_id.clone(),
            time_slot: bid.time_slot,
            bids: vec![bid],
            offers: vec![offer],
        };

        let matches = data.pay_as_bid();

        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].energy_rate, 50);
    }
}
