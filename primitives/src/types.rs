#![allow(non_snake_case)]

use crate::algorithms::{PayAsBid, PayAsClear};
use crate::db_api_schema::orders::{OrderEnum, OrderStatus};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use std::str::FromStr;

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct H256([u8; 32]);

impl H256 {
    pub fn zero() -> Self {
        Self([0u8; 32])
    }

    pub fn random() -> Self {
        let mut bytes = [0u8; 32];
        rand::thread_rng().fill_bytes(&mut bytes);
        Self(bytes)
    }

    pub fn from_slice(bytes: &[u8]) -> Self {
        let mut value = [0u8; 32];
        value.copy_from_slice(bytes);
        Self(value)
    }

    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl From<[u8; 32]> for H256 {
    fn from(value: [u8; 32]) -> Self {
        Self(value)
    }
}

impl fmt::Debug for H256 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "0x{}", hex::encode(self.0))
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AccountId32([u8; 32]);

impl AccountId32 {
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl From<[u8; 32]> for AccountId32 {
    fn from(value: [u8; 32]) -> Self {
        Self(value)
    }
}

impl fmt::Debug for AccountId32 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "0x{}", hex::encode(self.0))
    }
}

impl FromStr for AccountId32 {
    type Err = hex::FromHexError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let trimmed = value.trim();
        let hex_value = trimmed.strip_prefix("0x").unwrap_or(trimmed);
        let decoded = hex::decode(hex_value)?;
        if decoded.len() != 32 {
            return Err(hex::FromHexError::InvalidStringLength);
        }

        let mut bytes = [0u8; 32];
        bytes.copy_from_slice(&decoded);
        Ok(Self(bytes))
    }
}

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
    pub time_slot: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ClearingPoint {
    traded_energy: u64,
    clearing_price: u64,
}

impl MatchingData {
    pub fn validate_market_slot(&self) -> Result<(), String> {
        let mismatched_order =
            self.bids.iter().chain(self.offers.iter()).find(|order| {
                order.market_id != self.market_id || order.time_slot != self.time_slot
            });

        if let Some(order) = mismatched_order {
            return Err(format!(
                "Order {:?} belongs to market {:?} timeslot {}, expected market {:?} timeslot {}",
                order.order_id, order.market_id, order.time_slot, self.market_id, self.time_slot
            ));
        }

        Ok(())
    }

    fn orders_for_market_slot(&self) -> (Vec<Order>, Vec<Order>) {
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

    fn match_preferences(
        &self,
        bids: Vec<Order>,
        offers: Vec<Order>,
    ) -> (Vec<BidOfferMatch>, Vec<Order>, Vec<Order>) {
        let mut matches = Vec::new();

        type OrderKey = H256;

        let mut bid_matched_amounts: HashMap<OrderKey, u64> = HashMap::new();
        let mut offer_matched_amounts: HashMap<OrderKey, u64> = HashMap::new();

        let (preference_bids, _non_preference_bids): (Vec<&Order>, Vec<&Order>) =
            bids.iter().partition(|b| {
                b.requirements
                    .as_ref()
                    .and_then(|r| r.trading_partner_id.as_ref())
                    .is_some()
            });

        for bid in preference_bids {
            let req = bid.requirements.as_ref().unwrap();
            let partner_id = req.trading_partner_id.as_ref().unwrap();
            let bid_key = bid.order_id;

            let partner_offers: Vec<&Order> = offers
                .iter()
                .filter(|offer| {
                    offer.market_id == bid.market_id
                        && offer.time_slot == bid.time_slot
                        && &offer.created_by == partner_id
                })
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

        for bid in &bids {
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
        for offer in &offers {
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

    fn match_standard(&self, bids: Vec<Order>, offers: Vec<Order>) -> Vec<BidOfferMatch> {
        self.match_standard_at_clearing_point(bids, offers, None)
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
                if remaining_clearing_energy == 0 {
                    return matches;
                }

                if offer.area_uuid == bid.area_uuid {
                    continue;
                }

                if offer.market_id != bid.market_id || offer.time_slot != bid.time_slot {
                    continue;
                }

                if offer.energy_rate > bid.energy_rate {
                    continue;
                }

                if let Some(point) = clearing_point {
                    if bid.energy_rate < point.clearing_price
                        || offer.energy_rate > point.clearing_price
                    {
                        continue;
                    }
                }

                let bid_key = bid.order_id;
                let offer_key = offer.order_id;

                let offer_energy = *available_energy_offer.get(&offer_key).unwrap_or(&0);
                let bid_energy = *available_energy_bid.get(&bid_key).unwrap_or(&0);

                if offer_energy == 0 || bid_energy == 0 {
                    continue;
                }

                let selected_energy = offer_energy.min(bid_energy).min(remaining_clearing_energy);

                available_energy_bid.insert(bid_key.clone(), bid_energy - selected_energy);
                available_energy_offer.insert(offer_key.clone(), offer_energy - selected_energy);
                remaining_clearing_energy -= selected_energy;

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
                    energy_rate: clearing_point
                        .map(|point| point.clearing_price)
                        .unwrap_or(bid.energy_rate),
                };

                matches.push(new_bid_offer_match);
            }
        }
        matches
    }

    fn calculate_clearing_point(&self, bids: &[Order], offers: &[Order]) -> Option<ClearingPoint> {
        let mut bids = bids.iter().collect::<Vec<_>>();
        let mut offers = offers.iter().collect::<Vec<_>>();
        bids.sort_by(|a, b| b.energy_rate.cmp(&a.energy_rate));
        offers.sort_by(|a, b| a.energy_rate.cmp(&b.energy_rate));

        let mut bid_index = 0;
        let mut offer_index = 0;
        let mut bid_energy = bids.first().map(|bid| bid.energy).unwrap_or_default();
        let mut offer_energy = offers.first().map(|offer| offer.energy).unwrap_or_default();
        let mut traded_energy = 0u64;
        let mut clearing_price = None;

        while bid_index < bids.len() && offer_index < offers.len() {
            if bid_energy == 0 {
                bid_index += 1;
                bid_energy = bids
                    .get(bid_index)
                    .map(|bid| bid.energy)
                    .unwrap_or_default();
                continue;
            }

            if offer_energy == 0 {
                offer_index += 1;
                offer_energy = offers
                    .get(offer_index)
                    .map(|offer| offer.energy)
                    .unwrap_or_default();
                continue;
            }

            let bid = &bids[bid_index];
            let offer = &offers[offer_index];

            // The curves cross before this tranche: everything to the right
            // remains outside the current clearing interval.
            if bid.energy_rate < offer.energy_rate {
                break;
            }

            let accepted_energy = bid_energy.min(offer_energy);
            traded_energy += accepted_energy;
            clearing_price = Some(offer.energy_rate);
            bid_energy -= accepted_energy;
            offer_energy -= accepted_energy;
        }

        clearing_price.map(|clearing_price| ClearingPoint {
            traded_energy,
            clearing_price,
        })
    }

    fn match_standard_pay_as_clear(
        &self,
        bids: Vec<Order>,
        offers: Vec<Order>,
    ) -> Vec<BidOfferMatch> {
        let Some(clearing_point) = self.calculate_clearing_point(&bids, &offers) else {
            return Vec::new();
        };

        self.match_standard_at_clearing_point(bids, offers, Some(clearing_point))
    }
}

impl PayAsBid for MatchingData {
    type Output = BidOfferMatch;

    fn pay_as_bid(&mut self) -> Vec<Self::Output> {
        let mut all_matches = Vec::new();
        let (bids, offers) = self.orders_for_market_slot();

        let (pref_matches, remaining_bids, remaining_offers) = self.match_preferences(bids, offers);
        all_matches.extend(pref_matches);

        let standard_matches = self.match_standard(remaining_bids, remaining_offers);
        all_matches.extend(standard_matches);

        all_matches
    }
}

impl PayAsClear for MatchingData {
    type Output = BidOfferMatch;

    fn pay_as_clear(&mut self) -> Vec<Self::Output> {
        let mut all_matches = Vec::new();
        let (bids, offers) = self.orders_for_market_slot();

        // Keep bilateral preference matching compatible with the existing
        // preference contract. The uniform clearing price applies to the
        // remaining merit-order market.
        let (preference_matches, remaining_bids, remaining_offers) =
            self.match_preferences(bids, offers);
        all_matches.extend(preference_matches);

        let standard_matches = self.match_standard_pay_as_clear(remaining_bids, remaining_offers);
        all_matches.extend(standard_matches);

        all_matches
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn order(id: u8, order_type: OrderEnum, area: u8, energy: u64, energy_rate: u64) -> Order {
        Order {
            order_id: H256::from([id; 32]),
            order_type,
            status: OrderStatus::Open,
            area_uuid: H256::from([area; 32]),
            market_id: H256::from([99; 32]),
            time_slot: 1,
            creation_time: 1,
            energy,
            energy_rate,
            created_by: AccountId32::from([id; 32]),
            requirements: None,
            attributes: None,
        }
    }

    #[test]
    fn pay_as_clear_uses_the_marginal_accepted_offer_price() {
        let unmatched_bid_id = H256::from([3; 32]);
        let unmatched_offer_id = H256::from([6; 32]);
        let mut matching_data = MatchingData {
            bids: vec![
                order(1, OrderEnum::Bid, 1, 3, 20),
                order(2, OrderEnum::Bid, 2, 4, 17),
                order(3, OrderEnum::Bid, 3, 1, 9),
            ],
            offers: vec![
                order(4, OrderEnum::Offer, 4, 3, 8),
                order(5, OrderEnum::Offer, 5, 4, 10),
                order(6, OrderEnum::Offer, 6, 1, 12),
            ],
            market_id: H256::from([99; 32]),
            time_slot: 1,
        };

        let matches = matching_data.pay_as_clear();

        assert_eq!(matches.len(), 2);
        assert_eq!(
            matches.iter().map(|item| item.selected_energy).sum::<u64>(),
            7
        );
        assert!(matches.iter().all(|item| item.energy_rate == 10));
        assert!(matches
            .iter()
            .all(|item| item.bid.order_id != unmatched_bid_id));
        assert!(matches
            .iter()
            .all(|item| item.offer.order_id != unmatched_offer_id));
    }

    #[test]
    fn pay_as_clear_stops_when_the_bid_curve_drops_below_the_offer_curve() {
        let rejected_bid_id = H256::from([3; 32]);
        let rejected_offer_id = H256::from([6; 32]);
        let mut matching_data = MatchingData {
            bids: vec![
                order(1, OrderEnum::Bid, 1, 1, 28),
                order(2, OrderEnum::Bid, 2, 1, 23),
                order(3, OrderEnum::Bid, 3, 1, 17),
            ],
            offers: vec![
                order(4, OrderEnum::Offer, 4, 1, 10),
                order(5, OrderEnum::Offer, 5, 1, 15),
                order(6, OrderEnum::Offer, 6, 1, 21),
            ],
            market_id: H256::from([99; 32]),
            time_slot: 1,
        };

        let clearing_point = matching_data
            .calculate_clearing_point(&matching_data.bids, &matching_data.offers)
            .expect("The first two bid/offer tranches should clear");
        let matches = matching_data.pay_as_clear();

        assert_eq!(
            clearing_point,
            ClearingPoint {
                traded_energy: 2,
                clearing_price: 15,
            }
        );
        assert_eq!(matches.len(), 2);
        assert!(matches.iter().all(|item| item.energy_rate == 15));
        assert!(matches
            .iter()
            .all(|item| item.bid.order_id != rejected_bid_id));
        assert!(matches
            .iter()
            .all(|item| item.offer.order_id != rejected_offer_id));
    }

    #[test]
    fn pay_as_clear_prices_preferences_before_the_standard_market() {
        let preferred_bid_id = H256::from([1; 32]);
        let preferred_offer_id = H256::from([2; 32]);
        let unmatched_bid_id = H256::from([5; 32]);
        let unmatched_offer_id = H256::from([8; 32]);

        let mut preferred_bid = order(1, OrderEnum::Bid, 1, 2, 20);
        preferred_bid.requirements = Some(Requirements {
            trading_partner_id: Some(AccountId32::from([2; 32])),
            energy_type: None,
            preferred_energy_rate: Some(11),
        });
        let preferred_offer = order(2, OrderEnum::Offer, 2, 2, 15);

        let mut matching_data = MatchingData {
            bids: vec![
                preferred_bid,
                order(3, OrderEnum::Bid, 3, 3, 20),
                order(4, OrderEnum::Bid, 4, 4, 17),
                order(5, OrderEnum::Bid, 5, 1, 9),
            ],
            offers: vec![
                preferred_offer,
                order(6, OrderEnum::Offer, 6, 3, 8),
                order(7, OrderEnum::Offer, 7, 4, 10),
                order(8, OrderEnum::Offer, 8, 1, 12),
            ],
            market_id: H256::from([99; 32]),
            time_slot: 1,
        };

        let matches = matching_data.pay_as_clear();
        let preferred_match = matches
            .iter()
            .find(|item| {
                item.bid.order_id == preferred_bid_id && item.offer.order_id == preferred_offer_id
            })
            .expect("Preferred pair should match before standard clearing");
        assert_eq!(preferred_match.selected_energy, 2);
        assert_eq!(preferred_match.energy_rate, 11);

        let standard_matches = matches
            .iter()
            .filter(|item| item.bid.order_id != preferred_bid_id)
            .collect::<Vec<_>>();
        assert_eq!(standard_matches.len(), 2);
        assert_eq!(
            standard_matches
                .iter()
                .map(|item| item.selected_energy)
                .sum::<u64>(),
            7
        );
        assert!(standard_matches.iter().all(|item| item.energy_rate == 10));
        assert!(matches
            .iter()
            .all(|item| item.bid.order_id != unmatched_bid_id));
        assert!(matches
            .iter()
            .all(|item| item.offer.order_id != unmatched_offer_id));
    }

    #[test]
    fn pay_as_bid_does_not_match_across_market_or_time_slot() {
        let mut different_market_offer = order(2, OrderEnum::Offer, 2, 1, 10);
        different_market_offer.market_id = H256::from([100; 32]);
        let mut different_slot_offer = order(3, OrderEnum::Offer, 3, 1, 10);
        different_slot_offer.time_slot = 2;
        let mut matching_data = MatchingData {
            bids: vec![order(1, OrderEnum::Bid, 1, 1, 20)],
            offers: vec![different_market_offer, different_slot_offer],
            market_id: H256::from([99; 32]),
            time_slot: 1,
        };

        assert!(matching_data.pay_as_bid().is_empty());
    }

    #[test]
    fn pay_as_clear_ignores_other_market_and_time_slot_prices() {
        let expected_bid = order(1, OrderEnum::Bid, 1, 1, 20);
        let expected_offer = order(2, OrderEnum::Offer, 2, 1, 10);
        let mut other_market_offer = order(3, OrderEnum::Offer, 3, 1, 1);
        other_market_offer.market_id = H256::from([100; 32]);
        let mut other_slot_offer = order(4, OrderEnum::Offer, 4, 1, 2);
        other_slot_offer.time_slot = 2;
        let mut matching_data = MatchingData {
            bids: vec![expected_bid],
            offers: vec![expected_offer, other_market_offer, other_slot_offer],
            market_id: H256::from([99; 32]),
            time_slot: 1,
        };

        let matches = matching_data.pay_as_clear();

        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].energy_rate, 10);
        assert_eq!(matches[0].market_id, H256::from([99; 32]));
        assert_eq!(matches[0].time_slot, 1);
    }

    #[test]
    fn rejects_a_mixed_market_or_time_slot_order_book() {
        let mut other_market_offer = order(2, OrderEnum::Offer, 2, 1, 10);
        other_market_offer.market_id = H256::from([100; 32]);
        let mut matching_data = MatchingData {
            bids: vec![order(1, OrderEnum::Bid, 1, 1, 20)],
            offers: vec![other_market_offer],
            market_id: H256::from([99; 32]),
            time_slot: 1,
        };

        let error = crate::MatchingAlgorithm::PayAsBid
            .match_orders(&mut matching_data)
            .expect_err("A mixed order book must be rejected");

        assert!(error.contains("expected market"));
    }
}
