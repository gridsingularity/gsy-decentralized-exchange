use gsy_execution_engine::primitives::penalty_calculator::{
    compute_delivery_verifications, compute_penalties,
};
use primitives::db_api_schema::{
    orders::{DbOrderSchema, OrderEnum, OrderStatus},
    profiles::MeasurementSchema,
    trades::{DbTradeSchema, TradeParameters, TradeStatus},
};
use primitives::utils::{bytes16_to_hex, create_encrypted_bytes16_from_string};
use std::collections::HashMap;

fn onchain_id(id: &str) -> String {
    bytes16_to_hex(create_encrypted_bytes16_from_string(id))
}

fn order(order_id: &str, facility_id: &str, is_bid: bool) -> DbOrderSchema {
    let actor_id = bytes16_to_hex(create_encrypted_bytes16_from_string(facility_id));
    DbOrderSchema {
        order_id: order_id.to_string(),
        status: OrderStatus::Executed,
        status_updated_at: None,
        order_type: if is_bid {
            OrderEnum::Bid
        } else {
            OrderEnum::Offer
        },
        area_uuid: actor_id.clone(),
        market_id: "market-1".to_string(),
        time_slot: 1_000,
        creation_time: 900,
        energy_kWh: 10.0,
        energy_rate: 1.0,
        created_by: actor_id,
        requirements: None,
        attributes: None,
    }
}

fn trade() -> DbTradeSchema {
    let bid = order("bid-1", "alice", true);
    let offer = order("offer-1", "bob", false);
    DbTradeSchema {
        trade_uuid: "trade-1".to_string(),
        status: TradeStatus::Matched,
        seller: offer.created_by.clone(),
        buyer: bid.created_by.clone(),
        market_id: "market-1".to_string(),
        creation_time: 950,
        offer_hash: "offer-1".to_string(),
        bid_hash: "bid-1".to_string(),
        residual_offer_id: None,
        residual_bid_id: None,
        parameters: TradeParameters {
            selected_energy_kWh: 10.0,
            energy_rate: 1.0,
        },
        status_updated_at: None,
    }
}

fn measurement(facility_id: &str, energy_kwh: f64) -> MeasurementSchema {
    MeasurementSchema {
        facility_id: facility_id.to_string(),
        community_uuid: "community1".to_string(),
        time_slot: 1_000,
        creation_time: 1_000,
        energy_kwh,
    }
}

#[test]
fn compute_penalties_matches_facility_measurements_to_onchain_ids() {
    let measurements = vec![
        measurement("areaalice", 12.0),
        measurement("areabob", -10.0),
    ];

    // The mapping comes from the same ID service used by the community client.
    let facility_onchain_ids = HashMap::from([
        ("areaalice".to_string(), onchain_id("alice")),
        ("areabob".to_string(), onchain_id("bob")),
    ]);

    let penalties = compute_penalties(&[trade()], &measurements, &facility_onchain_ids, 0.10, 5.0);

    assert_eq!(penalties.len(), 1);
    assert_eq!(penalties[0].penalty_cost, 2_000);
    assert_eq!(
        penalties[0].penalized_account,
        bytes16_to_hex(create_encrypted_bytes16_from_string("alice"))
    );
}

#[test]
fn verification_distinguishes_missing_measurements_from_zero_penalties() {
    let mapping = HashMap::from([
        ("buyer-meter".to_string(), onchain_id("alice")),
        ("seller-meter".to_string(), onchain_id("bob")),
    ]);
    let buyer = measurement("buyer-meter", 10.0);
    let seller = measurement("seller-meter", -10.0);
    assert!(compute_delivery_verifications(&[trade()], &[], &mapping, 0.1, 5.0).is_empty());
    for only_one in [buyer.clone(), seller.clone()] {
        assert!(
            compute_delivery_verifications(&[trade()], &[only_one], &mapping, 0.1, 5.0).is_empty()
        );
    }
    let measurements = [buyer, seller];
    assert!(
        compute_delivery_verifications(&[trade()], &measurements, &HashMap::new(), 0.1, 5.0)
            .is_empty()
    );
    let verdicts = compute_delivery_verifications(&[trade()], &measurements, &mapping, 0.1, 5.0);
    assert_eq!(verdicts.len(), 1);
    assert_eq!(verdicts[0].buyer_penalty_cost, 0);
    assert_eq!(verdicts[0].seller_penalty_cost, 0);
}

#[test]
fn verification_penalizes_each_partys_own_deviation_in_both_directions() {
    let mapping = HashMap::from([
        ("buyer-meter".to_string(), onchain_id("alice")),
        ("seller-meter".to_string(), onchain_id("bob")),
    ]);
    for (buyer, seller, expected_buyer, expected_seller) in [
        (12.0, -10.0, 2_000, 0),
        (8.0, -10.0, 2_000, 0),
        (10.0, -12.0, 0, 2_000),
        (10.0, -8.0, 0, 2_000),
        (12.0, -8.0, 2_000, 2_000),
        (8.0, -12.0, 2_000, 2_000),
        (0.0, -10.0, 10_000, 0),
        (10.0, 0.0, 0, 10_000),
        (-10.0, -10.0, 20_000, 0),
        (10.0, 10.0, 0, 20_000),
    ] {
        let measurements = [
            measurement("buyer-meter", buyer),
            measurement("seller-meter", seller),
        ];
        let trades = [trade()];
        let verdicts = compute_delivery_verifications(&trades, &measurements, &mapping, 0.1, 5.0);
        assert_eq!(verdicts.len(), 1);
        assert_eq!(
            (
                verdicts[0].buyer_penalty_cost,
                verdicts[0].seller_penalty_cost
            ),
            (expected_buyer, expected_seller),
            "buyer={buyer}, seller={seller}"
        );
        let penalties = compute_penalties(&trades, &measurements, &mapping, 0.1, 5.0);
        for (party, cost) in [("alice", expected_buyer), ("bob", expected_seller)] {
            let penalty = penalties
                .iter()
                .find(|penalty| penalty.penalized_account == onchain_id(party));
            assert_eq!(
                penalty.map(|penalty| penalty.penalty_cost),
                (cost > 0).then_some(cost)
            );
        }
    }
}

#[test]
fn verification_ignores_unrelated_or_invalid_measurements() {
    let mapping = HashMap::from([
        ("buyer-meter".to_string(), onchain_id("alice")),
        ("seller-meter".to_string(), onchain_id("bob")),
        ("other-meter".to_string(), onchain_id("charlie")),
    ]);
    for measurements in [
        vec![
            measurement("buyer-meter", 10.0),
            measurement("other-meter", -10.0),
        ],
        vec![
            measurement("unknown-meter", 10.0),
            measurement("seller-meter", -10.0),
        ],
    ] {
        assert!(
            compute_delivery_verifications(&[trade()], &measurements, &mapping, 0.1, 5.0)
                .is_empty()
        );
    }
    for invalid in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        for (buyer, seller) in [(invalid, -10.0), (10.0, invalid)] {
            let measurements = [
                measurement("buyer-meter", buyer),
                measurement("seller-meter", seller),
            ];
            assert!(
                compute_delivery_verifications(&[trade()], &measurements, &mapping, 0.1, 5.0)
                    .is_empty()
            );
        }
    }
}

#[test]
fn verification_keeps_distinct_facilities_separate_and_uses_stored_ids() {
    // Two facilities may share an owner; only their mapped facility IDs identify trades.
    let first_id = "0x11111111111111111111111111111111";
    let second_id = "0x22222222222222222222222222222222";
    let mapping = HashMap::from([
        ("first-meter".into(), first_id.into()),
        ("second-meter".into(), second_id.into()),
        ("seller-meter".into(), onchain_id("bob")),
    ]);
    let mut first = trade();
    first.buyer = first_id.into();
    let mut second = first.clone();
    second.trade_uuid = "trade-2".into();
    second.buyer = second_id.into();
    let trades = [first, second];
    let measurements = [
        measurement("first-meter", 12.0),
        measurement("seller-meter", -10.0),
    ];
    let verifications = compute_delivery_verifications(&trades, &measurements, &mapping, 0.1, 5.0);
    assert_eq!(verifications.len(), 1);
    assert_eq!(verifications[0].trade_uuid, "trade-1");
    let penalties = compute_penalties(&trades, &measurements, &mapping, 0.1, 5.0);
    assert_eq!(penalties.len(), 1);
    assert_eq!(penalties[0].penalized_account, first_id);
}

#[test]
fn tolerance_is_inclusive_and_penalties_use_the_full_deviation() {
    let mapping = HashMap::from([
        ("buyer-meter".to_string(), onchain_id("alice")),
        ("seller-meter".to_string(), onchain_id("bob")),
    ]);
    for (traded, measured, tolerance, expected_cost) in [
        (100.0, 100.0, 5.0, 0),
        (100.0, 96.0, 5.0, 0),
        (100.0, 104.0, 5.0, 0),
        (100.0, 95.0, 5.0, 0),
        (100.0, 105.0, 5.0, 0),
        (100.0, 94.9, 5.0, 5_100),
        (100.0, 105.1, 5.0, 5_100),
        (100.0, 94.0, 5.0, 6_000),
        (100.0, 106.0, 5.0, 6_000),
        (1.0, 0.95, 5.0, 0),
        (1.0, 1.05, 5.0, 0),
        (100.0, 100.0, 0.0, 0),
        (100.0, 99.0, 0.0, 1_000),
        (100.0, 101.0, 0.0, 1_000),
        (100.0, 94.0, 10.0, 0),
        (100.0, 106.0, 10.0, 0),
        (100.0, 110.0, 10.0, 0),
        (100.0, 110.1, 10.0, 10_100),
        (100.0, 102.5, 2.5, 0),
        (100.0, 102.6, 2.5, 2_600),
        (100.0, 200.0, 100.0, 0),
        (100.0, 201.0, 100.0, 101_000),
    ] {
        let mut trade = trade();
        trade.parameters.selected_energy_kWh = traded;
        let trades = [trade];
        for (buyer, seller) in [(measured, -traded), (traded, -measured)] {
            let measurements = [
                measurement("buyer-meter", buyer),
                measurement("seller-meter", seller),
            ];
            let verifications =
                compute_delivery_verifications(&trades, &measurements, &mapping, 0.1, tolerance);
            assert_eq!(
                verifications.len(),
                1,
                "Measured trades must still complete"
            );
            assert_eq!(
                verifications[0].buyer_penalty_cost + verifications[0].seller_penalty_cost,
                expected_cost,
                "traded={traded}, measured={measured}, tolerance={tolerance}"
            );
            let penalties = compute_penalties(&trades, &measurements, &mapping, 0.1, tolerance);
            assert_eq!(penalties.len(), usize::from(expected_cost > 0));
            assert_eq!(
                penalties
                    .iter()
                    .map(|penalty| penalty.penalty_cost)
                    .sum::<u64>(),
                expected_cost
            );
        }
    }
}
