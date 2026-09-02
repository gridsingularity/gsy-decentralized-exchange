use super::PayAsBid;
use crate::models::{BidOfferMatch, MatchingData, Order};
use uuid::Uuid;

impl MatchingData {
    fn match_standard(&self, bids: Vec<Order>, offers: Vec<Order>) -> Vec<BidOfferMatch> {
        self.match_standard_at_clearing_point(bids, offers, None)
    }
}

impl PayAsBid for MatchingData {
    type Output = BidOfferMatch;

    fn pay_as_bid(&mut self) -> Vec<Self::Output> {
        let bids = self.bids().to_vec();
        let offers = self.offers().to_vec();
        let (mut matches, remaining_bids, remaining_offers) = self.match_preferences(bids, offers);
        matches.extend(self.match_standard(remaining_bids, remaining_offers));
        matches
    }
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
        let mut data =
            MatchingData::new(bid.market_id.clone(), bid.time_slot, vec![bid], vec![offer])
                .expect("orders should belong to one market slot");

        let matches = data.pay_as_bid();

        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].selected_energy, 60);
        let residual_id = &matches[0]
            .residual_bid
            .as_ref()
            .expect("bid should be partially filled")
            .order_id;
        Uuid::parse_str(&residual_id).expect("not a valid uuid");
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
        let mut data =
            MatchingData::new(bid.market_id.clone(), bid.time_slot, vec![bid], vec![offer])
                .expect("orders should belong to one market slot");

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
        let mut data =
            MatchingData::new(bid.market_id.clone(), bid.time_slot, vec![bid], vec![offer])
                .expect("orders should belong to one market slot");

        let matches = data.pay_as_bid();

        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].energy_rate, 50);
    }
}
