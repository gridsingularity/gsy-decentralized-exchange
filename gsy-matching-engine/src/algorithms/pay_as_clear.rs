use super::{ClearingPoint, PayAsClear};
use crate::models::{BidOfferMatch, MatchingData, Order};
use std::{fmt, str::FromStr};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum PayAsClearPricing {
    #[default]
    MaxOffer,
    MinBid,
    Midpoint,
}

impl PayAsClearPricing {
    fn clearing_price(self, max_offer: u64, min_bid: u64) -> u64 {
        match self {
            Self::MaxOffer => max_offer,
            Self::MinBid => min_bid,
            // Round down in fixed-point units without overflowing the sum.
            Self::Midpoint => max_offer + (min_bid - max_offer) / 2,
        }
    }
}

impl fmt::Display for PayAsClearPricing {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::MaxOffer => "max_offer",
            Self::MinBid => "min_bid",
            Self::Midpoint => "midpoint",
        })
    }
}

impl FromStr for PayAsClearPricing {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim().to_ascii_lowercase().as_str() {
            "max_offer" => Ok(Self::MaxOffer),
            "min_bid" => Ok(Self::MinBid),
            "midpoint" => Ok(Self::Midpoint),
            _ => Err(format!(
                "Unsupported pay-as-clear pricing '{}'. Expected max_offer, min_bid, or midpoint",
                value
            )),
        }
    }
}

struct ClearingVolume {
    traded_energy: u64,
    max_accepted_offer: u64,
    min_accepted_bid: u64,
    demand_remaining: bool,
    supply_remaining: bool,
}

impl ClearingVolume {
    fn pricing_with_scarcity_override(&self, configured: PayAsClearPricing) -> PayAsClearPricing {
        match (self.demand_remaining, self.supply_remaining) {
            (true, false) => PayAsClearPricing::MinBid,
            (false, true) => PayAsClearPricing::MaxOffer,
            // A price crossing or simultaneous exhaustion keeps the configured policy.
            _ => configured,
        }
    }
}

impl MatchingData {
    /// Calculate standard-market volume and price; trade allocation happens afterwards.
    fn calculate_clearing_point(
        &self,
        bids: &[Order],
        offers: &[Order],
        pricing: PayAsClearPricing,
    ) -> Option<ClearingPoint> {
        let mut bids = bids.iter().collect::<Vec<_>>();
        let mut offers = offers.iter().collect::<Vec<_>>();
        bids.sort_by(|left, right| right.energy_rate.cmp(&left.energy_rate));
        offers.sort_by(|left, right| left.energy_rate.cmp(&right.energy_rate));

        let volume = walk_sorted_curves(&bids, &offers)?;
        let effective_pricing = volume.pricing_with_scarcity_override(pricing);

        Some(ClearingPoint {
            traded_energy: volume.traded_energy,
            clearing_price: effective_pricing
                .clearing_price(volume.max_accepted_offer, volume.min_accepted_bid),
        })
    }

    fn match_standard_pay_as_clear(
        &self,
        bids: Vec<Order>,
        offers: Vec<Order>,
        pricing: PayAsClearPricing,
    ) -> Vec<BidOfferMatch> {
        let Some(clearing_point) = self.calculate_clearing_point(&bids, &offers, pricing) else {
            return Vec::new();
        };

        self.match_standard_at_clearing_point(bids, offers, Some(clearing_point))
    }

    /// Select standard-market pricing without changing the preceding preference phase.
    pub fn pay_as_clear_with_pricing(&mut self, pricing: PayAsClearPricing) -> Vec<BidOfferMatch> {
        let bids = self.bids().to_vec();
        let offers = self.offers().to_vec();
        let (mut matches, remaining_bids, remaining_offers) = self.match_preferences(bids, offers);
        matches.extend(self.match_standard_pay_as_clear(remaining_bids, remaining_offers, pricing));
        matches
    }
}

impl PayAsClear for MatchingData {
    type Output = BidOfferMatch;

    fn pay_as_clear(&mut self) -> Vec<Self::Output> {
        self.pay_as_clear_with_pricing(PayAsClearPricing::default())
    }
}

/// Walk descending bids and ascending offers until prices cross or one side ends.
fn walk_sorted_curves(bids: &[&Order], offers: &[&Order]) -> Option<ClearingVolume> {
    let mut bid_index = 0;
    let mut offer_index = 0;
    let mut bid_energy = bids.first().map(|bid| bid.energy).unwrap_or_default();
    let mut offer_energy = offers.first().map(|offer| offer.energy).unwrap_or_default();
    let mut traded_energy = 0u64;
    let mut marginal_rates = None;

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

        let bid = bids[bid_index];
        let offer = offers[offer_index];
        if bid.energy_rate < offer.energy_rate {
            break;
        }

        let accepted_energy = bid_energy.min(offer_energy);
        traded_energy += accepted_energy;
        marginal_rates = Some((offer.energy_rate, bid.energy_rate));
        bid_energy -= accepted_energy;
        offer_energy -= accepted_energy;
    }

    let (max_accepted_offer, min_accepted_bid) = marginal_rates?;
    // The loop can stop before advancing both cursors. Include later positive
    // quantities so simultaneous exhaustion and empty trailing orders are handled.
    let demand_remaining =
        bid_energy > 0 || bids.iter().skip(bid_index + 1).any(|bid| bid.energy > 0);
    let supply_remaining =
        offer_energy > 0 || offers.iter().skip(offer_index + 1).any(|offer| offer.energy > 0);

    Some(ClearingVolume {
        traded_energy,
        max_accepted_offer,
        min_accepted_bid,
        demand_remaining,
        supply_remaining,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::Requirements;
    use primitives::db_api_schema::orders::{OrderEnum, OrderStatus};

    fn id(value: u8) -> String {
        format!("0x{value:032x}")
    }

    fn order(id_value: u8, order_type: OrderEnum, area: u8, energy: u64, rate: u64) -> Order {
        Order {
            order_id: id(id_value),
            order_type,
            status: OrderStatus::Submitted,
            area_uuid: id(area),
            market_id: id(99),
            time_slot: 1,
            creation_time: 1,
            energy,
            energy_rate: rate,
            created_by: id(id_value),
            requirements: None,
            attributes: None,
        }
    }

    #[test]
    fn uses_the_marginal_accepted_offer_price() {
        let mut matching_data = MatchingData::new(
            id(99),
            1,
            vec![
                order(1, OrderEnum::Bid, 1, 3, 20),
                order(2, OrderEnum::Bid, 2, 4, 17),
                order(3, OrderEnum::Bid, 3, 1, 9),
            ],
            vec![
                order(4, OrderEnum::Offer, 4, 3, 8),
                order(5, OrderEnum::Offer, 5, 4, 10),
                order(6, OrderEnum::Offer, 6, 1, 12),
            ],
        )
        .expect("orders should belong to one market slot");

        let matches = matching_data.pay_as_clear();

        assert_eq!(matches.len(), 2);
        assert_eq!(
            matches.iter().map(|item| item.selected_energy).sum::<u64>(),
            7
        );
        assert!(matches.iter().all(|item| item.energy_rate == 10));
        assert!(matches.iter().all(|item| item.bid.order_id != id(3)));
        assert!(matches.iter().all(|item| item.offer.order_id != id(6)));
    }

    #[test]
    fn stops_when_the_bid_curve_drops_below_the_offer_curve() {
        let mut matching_data = MatchingData::new(
            id(99),
            1,
            vec![
                order(1, OrderEnum::Bid, 1, 1, 28),
                order(2, OrderEnum::Bid, 2, 1, 23),
                order(3, OrderEnum::Bid, 3, 1, 17),
            ],
            vec![
                order(4, OrderEnum::Offer, 4, 1, 10),
                order(5, OrderEnum::Offer, 5, 1, 15),
                order(6, OrderEnum::Offer, 6, 1, 21),
            ],
        )
        .expect("orders should belong to one market slot");

        let matches = matching_data.pay_as_clear();

        assert_eq!(matches.len(), 2);
        assert!(matches.iter().all(|item| item.energy_rate == 15));
        assert!(matches.iter().all(|item| item.bid.order_id != id(3)));
        assert!(matches.iter().all(|item| item.offer.order_id != id(6)));
    }

    #[test]
    fn settles_preferences_before_the_standard_market() {
        let mut preferred_bid = order(1, OrderEnum::Bid, 1, 2, 20);
        preferred_bid.requirements = Some(Requirements {
            trading_partner_id: Some(id(2)),
            energy_type: None,
            preferred_energy_rate: Some(11),
        });
        let mut matching_data = MatchingData::new(
            id(99),
            1,
            vec![
                preferred_bid,
                order(3, OrderEnum::Bid, 3, 3, 20),
                order(4, OrderEnum::Bid, 4, 4, 17),
                order(5, OrderEnum::Bid, 5, 1, 9),
            ],
            vec![
                order(2, OrderEnum::Offer, 2, 2, 10),
                order(6, OrderEnum::Offer, 6, 3, 8),
                order(7, OrderEnum::Offer, 7, 4, 10),
                order(8, OrderEnum::Offer, 8, 1, 12),
            ],
        )
        .expect("orders should belong to one market slot");

        let matches = matching_data.pay_as_clear();
        let preferred_match = matches
            .iter()
            .find(|item| item.bid.order_id == id(1) && item.offer.order_id == id(2))
            .expect("preferred pair should match first");

        assert_eq!(preferred_match.selected_energy, 2);
        assert_eq!(preferred_match.energy_rate, 11);
        let standard_matches = matches
            .iter()
            .filter(|item| item.bid.order_id != id(1))
            .collect::<Vec<_>>();
        assert_eq!(standard_matches.len(), 2);
        assert!(standard_matches.iter().all(|item| item.energy_rate == 10));
    }
}
