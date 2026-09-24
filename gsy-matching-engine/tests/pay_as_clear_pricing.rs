use gsy_matching_engine::algorithms::{MatchOrders, PayAsClear, PayAsClearPricing};
use gsy_matching_engine::models::{MatchingData, Order, Requirements};
use primitives::db_api_schema::orders::{OrderEnum, OrderStatus};
use primitives::MatchingAlgorithm;

const POLICIES: [PayAsClearPricing; 3] = [
    PayAsClearPricing::MaxOffer,
    PayAsClearPricing::MinBid,
    PayAsClearPricing::Midpoint,
];

fn order(id: &str, order_type: OrderEnum, energy: u64, energy_rate: u64) -> Order {
    Order {
        order_id: id.into(),
        order_type,
        status: OrderStatus::Submitted,
        area_uuid: id.into(),
        market_id: "market".into(),
        time_slot: 1,
        creation_time: 1,
        energy,
        energy_rate,
        created_by: id.into(),
        requirements: None,
        attributes: None,
    }
}

fn book(bids: Vec<Order>, offers: Vec<Order>) -> MatchingData {
    MatchingData::new("market".into(), 1, bids, offers).unwrap()
}

fn crossing_book() -> MatchingData {
    // Deliberately unsorted, with rejected prices outside the accepted range.
    book(
        vec![
            order("bid-low", OrderEnum::Bid, 1, 9),
            order("bid-high", OrderEnum::Bid, 3, 20),
            order("bid-marginal", OrderEnum::Bid, 4, 17),
        ],
        vec![
            order("offer-high", OrderEnum::Offer, 1, 30),
            order("offer-marginal", OrderEnum::Offer, 4, 10),
            order("offer-low", OrderEnum::Offer, 3, 8),
        ],
    )
}

#[test]
fn policies_change_price_without_changing_accepted_volume_or_pairs() {
    for (policy, expected_price) in POLICIES.into_iter().zip([10, 17, 13]) {
        let matches = crossing_book().pay_as_clear_with_pricing(policy);
        assert_eq!(matches.len(), 2);
        assert_eq!(matches[0].bid.order_id, "bid-high");
        assert_eq!(matches[0].offer.order_id, "offer-low");
        assert_eq!(matches[0].selected_energy, 3);
        assert_eq!(matches[1].bid.order_id, "bid-marginal");
        assert_eq!(matches[1].offer.order_id, "offer-marginal");
        assert_eq!(matches[1].selected_energy, 4);
        for item in matches {
            assert_eq!(item.energy_rate, expected_price, "{policy:?}");
            assert!(item.offer.energy_rate <= item.energy_rate);
            assert!(item.energy_rate <= item.bid.energy_rate);
        }
    }
}

#[test]
fn algorithm_dispatch_uses_pricing_only_for_pay_as_clear() {
    for (policy, expected_price) in POLICIES.into_iter().zip([10, 17, 13]) {
        let matches = MatchingAlgorithm::PayAsClear
            .match_orders_with_pricing(&mut crossing_book(), policy)
            .unwrap();
        assert!(matches
            .iter()
            .all(|item| item.energy_rate == expected_price));
        assert_eq!(
            MatchingAlgorithm::PayAsBid
                .match_orders_with_pricing(&mut crossing_book(), policy)
                .unwrap(),
            MatchingAlgorithm::PayAsBid
                .match_orders(&mut crossing_book())
                .unwrap()
        );
    }
}

#[test]
fn default_entry_point_preserves_max_offer_pricing() {
    assert_eq!(PayAsClearPricing::default(), PayAsClearPricing::MaxOffer);
    assert_eq!(
        crossing_book().pay_as_clear(),
        crossing_book().pay_as_clear_with_pricing(PayAsClearPricing::MaxOffer)
    );
}

#[test]
fn midpoint_rounds_down_without_overflow() {
    for (offer_rate, bid_rate, expected_price) in [
        (0, 1, 0),
        (10, 16, 13),
        (10, 17, 13),
        (u64::MAX - 3, u64::MAX, u64::MAX - 2),
        (0, u64::MAX, u64::MAX / 2),
    ] {
        let matches = book(
            vec![order("bid", OrderEnum::Bid, 1, bid_rate)],
            vec![order("offer", OrderEnum::Offer, 1, offer_rate)],
        )
        .pay_as_clear_with_pricing(PayAsClearPricing::Midpoint);
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].energy_rate, expected_price);
    }
}

#[test]
fn equal_rates_clear_at_the_same_price_for_every_policy() {
    for policy in POLICIES {
        let matches = book(
            vec![order("bid", OrderEnum::Bid, 1, 10)],
            vec![order("offer", OrderEnum::Offer, 1, 10)],
        )
        .pay_as_clear_with_pricing(policy);
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].energy_rate, 10);
    }
}

#[test]
fn no_price_is_selected_without_compatible_positive_volume() {
    for policy in POLICIES {
        for (bid_energy, offer_energy, bid_rate, offer_rate) in
            [(1, 1, 9, 10), (0, 1, 20, 10), (1, 0, 20, 10)]
        {
            assert!(book(
                vec![order("bid", OrderEnum::Bid, bid_energy, bid_rate)],
                vec![order("offer", OrderEnum::Offer, offer_energy, offer_rate)],
            )
            .pay_as_clear_with_pricing(policy)
            .is_empty());
        }
        for (bids, offers) in [
            (vec![], vec![]),
            (vec![order("bid", OrderEnum::Bid, 1, 20)], vec![]),
            (vec![], vec![order("offer", OrderEnum::Offer, 1, 10)]),
        ] {
            assert!(book(bids, offers)
                .pay_as_clear_with_pricing(policy)
                .is_empty());
        }
    }
}

#[test]
fn standard_pricing_policy_does_not_change_preference_prices() {
    for (policy, expected_standard_price) in POLICIES.into_iter().zip([10, 17, 13]) {
        let mut preferred_bid = order("preferred-bid", OrderEnum::Bid, 2, 20);
        preferred_bid.requirements = Some(Requirements {
            trading_partner_id: Some("preferred-offer".into()),
            energy_type: None,
            preferred_energy_rate: Some(11),
        });
        let standard = crossing_book();
        let mut bids = standard.bids().to_vec();
        let mut offers = standard.offers().to_vec();
        bids.push(preferred_bid);
        offers.push(order("preferred-offer", OrderEnum::Offer, 2, 10));

        let matches = book(bids, offers).pay_as_clear_with_pricing(policy);
        assert_eq!(matches.len(), 3);
        assert_eq!(matches[0].bid.order_id, "preferred-bid");
        assert_eq!(matches[0].offer.order_id, "preferred-offer");
        assert_eq!(matches[0].selected_energy, 2);
        assert_eq!(matches[0].energy_rate, 11);
        assert!(matches[1..]
            .iter()
            .all(|item| item.energy_rate == expected_standard_price));
    }
}

#[test]
fn supply_exhaustion_uses_minimum_accepted_bid_under_every_policy() {
    for policy in POLICIES {
        // Exercise both an exact order boundary and a partially filled last bid.
        for marginal_bid_energy in [4, 5] {
            let matches = book(
                vec![
                    order("bid-high", OrderEnum::Bid, 3, 20),
                    order("bid-marginal", OrderEnum::Bid, marginal_bid_energy, 17),
                    order("bid-unmatched", OrderEnum::Bid, 1, 5),
                ],
                vec![
                    order("offer-low", OrderEnum::Offer, 3, 8),
                    order("offer-marginal", OrderEnum::Offer, 4, 10),
                    order("offer-empty", OrderEnum::Offer, 0, 30),
                ],
            )
            .pay_as_clear_with_pricing(policy);

            assert_eq!(matches.len(), 2);
            assert_eq!(
                matches.iter().map(|item| item.selected_energy).sum::<u64>(),
                7
            );
            assert!(matches.iter().all(|item| item.energy_rate == 17));
            assert!(matches
                .iter()
                .all(|item| item.bid.order_id != "bid-unmatched"));
            assert_eq!(
                matches[1].residual_bid.as_ref().map(|order| order.energy),
                if marginal_bid_energy == 5 {
                    Some(1)
                } else {
                    None
                }
            );
            assert!(matches.iter().all(|item| item.residual_offer.is_none()));
        }
    }
}

#[test]
fn demand_exhaustion_uses_maximum_accepted_offer_under_every_policy() {
    for policy in POLICIES {
        for marginal_offer_energy in [4, 5] {
            let matches = book(
                vec![
                    order("bid-high", OrderEnum::Bid, 3, 20),
                    order("bid-marginal", OrderEnum::Bid, 4, 17),
                    order("bid-empty", OrderEnum::Bid, 0, 5),
                ],
                vec![
                    order("offer-low", OrderEnum::Offer, 3, 8),
                    order(
                        "offer-marginal",
                        OrderEnum::Offer,
                        marginal_offer_energy,
                        10,
                    ),
                    order("offer-unmatched", OrderEnum::Offer, 1, 30),
                ],
            )
            .pay_as_clear_with_pricing(policy);

            assert_eq!(matches.len(), 2);
            assert_eq!(
                matches.iter().map(|item| item.selected_energy).sum::<u64>(),
                7
            );
            assert!(matches.iter().all(|item| item.energy_rate == 10));
            assert!(matches
                .iter()
                .all(|item| item.offer.order_id != "offer-unmatched"));
            assert_eq!(
                matches[1].residual_offer.as_ref().map(|order| order.energy),
                if marginal_offer_energy == 5 {
                    Some(1)
                } else {
                    None
                }
            );
            assert!(matches.iter().all(|item| item.residual_bid.is_none()));
        }
    }
}

#[test]
fn scarcity_detects_remaining_energy_within_the_last_order() {
    for policy in POLICIES {
        for (bid_energy, offer_energy, expected_price) in [(2, 1, 17), (1, 2, 10)] {
            let matches = book(
                vec![order("bid", OrderEnum::Bid, bid_energy, 17)],
                vec![order("offer", OrderEnum::Offer, offer_energy, 10)],
            )
            .pay_as_clear_with_pricing(policy);
            assert_eq!(matches.len(), 1);
            assert_eq!(matches[0].selected_energy, 1);
            assert_eq!(matches[0].energy_rate, expected_price);
        }
    }
}

#[test]
fn simultaneous_exhaustion_uses_configured_policy_despite_empty_orders() {
    for (policy, expected_price) in POLICIES.into_iter().zip([10, 17, 13]) {
        let matches = book(
            vec![
                order("bid-empty-high", OrderEnum::Bid, 0, 30),
                order("bid-high", OrderEnum::Bid, 3, 20),
                order("bid-marginal", OrderEnum::Bid, 4, 17),
                order("bid-empty-low", OrderEnum::Bid, 0, 5),
            ],
            vec![
                order("offer-empty-low", OrderEnum::Offer, 0, 0),
                order("offer-low", OrderEnum::Offer, 3, 8),
                order("offer-marginal", OrderEnum::Offer, 4, 10),
                order("offer-empty-high", OrderEnum::Offer, 0, 30),
            ],
        )
        .pay_as_clear_with_pricing(policy);
        assert_eq!(matches.len(), 2);
        assert_eq!(
            matches.iter().map(|item| item.selected_energy).sum::<u64>(),
            7
        );
        assert!(matches
            .iter()
            .all(|item| item.energy_rate == expected_price));
    }
}

#[test]
fn crossing_with_unequal_total_energy_is_not_scarcity() {
    for (policy, expected_price) in POLICIES.into_iter().zip([10, 17, 13]) {
        for (unmatched_bid_energy, unmatched_offer_energy) in [(100, 1), (1, 100)] {
            let original = crossing_book();
            let mut bids = original.bids().to_vec();
            let mut offers = original.offers().to_vec();
            bids[0].energy = unmatched_bid_energy;
            offers[0].energy = unmatched_offer_energy;
            let matches = book(bids, offers).pay_as_clear_with_pricing(policy);
            assert_eq!(
                matches.iter().map(|item| item.selected_energy).sum::<u64>(),
                7
            );
            assert!(matches
                .iter()
                .all(|item| item.energy_rate == expected_price));
        }
    }
}
