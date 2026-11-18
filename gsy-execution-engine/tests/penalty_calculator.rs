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

fn order(order_id: &str, owner_id: &str, is_bid: bool) -> DbOrderSchema {
    let actor_id = bytes16_to_hex(create_encrypted_bytes16_from_string(owner_id));
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
fn compute_penalties_matches_facility_measurements_to_owner_ids() {
    let measurements = vec![
        measurement("areaalice", 12.0),
        measurement("areabob", -10.0),
    ];

    // Facility records link each meter to the owner used in order creation.
    let facility_owner_mapping = HashMap::from([
        ("areaalice".to_string(), "alice".to_string()),
        ("areabob".to_string(), "bob".to_string()),
    ]);

    let penalties = compute_penalties(
        &[trade()],
        &measurements,
        &facility_owner_mapping,
        1_000,
        0.10,
        5.0,
    )
    .unwrap();

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
        ("buyer-meter".to_string(), "alice".to_string()),
        ("seller-meter".to_string(), "bob".to_string()),
    ]);
    let buyer = measurement("buyer-meter", 10.0);
    let seller = measurement("seller-meter", -10.0);
    assert!(
        compute_delivery_verifications(&[trade()], &[], &mapping, 1_000, 0.1, 5.0)
            .unwrap()
            .is_empty()
    );
    for only_one in [buyer.clone(), seller.clone()] {
        assert!(
            compute_delivery_verifications(&[trade()], &[only_one], &mapping, 1_000, 0.1, 5.0)
                .unwrap()
                .is_empty()
        );
    }
    let measurements = [buyer, seller];
    assert!(compute_delivery_verifications(
        &[trade()],
        &measurements,
        &HashMap::new(),
        1_000,
        0.1,
        5.0
    )
    .unwrap()
    .is_empty());
    let verdicts =
        compute_delivery_verifications(&[trade()], &measurements, &mapping, 1_000, 0.1, 5.0)
            .unwrap();
    assert_eq!(verdicts.len(), 1);
    assert_eq!(verdicts[0].buyer_penalty_cost, 0);
    assert_eq!(verdicts[0].seller_penalty_cost, 0);
}

#[test]
fn verification_penalizes_each_partys_own_deviation_in_both_directions() {
    let mapping = HashMap::from([
        ("buyer-meter".to_string(), "alice".to_string()),
        ("seller-meter".to_string(), "bob".to_string()),
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
        let verdicts =
            compute_delivery_verifications(&trades, &measurements, &mapping, 1_000, 0.1, 5.0)
                .unwrap();
        assert_eq!(verdicts.len(), 1);
        assert_eq!(
            (
                verdicts[0].buyer_penalty_cost,
                verdicts[0].seller_penalty_cost
            ),
            (expected_buyer, expected_seller),
            "buyer={buyer}, seller={seller}"
        );
        let penalties =
            compute_penalties(&trades, &measurements, &mapping, 1_000, 0.1, 5.0).unwrap();
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
        ("buyer-meter".to_string(), "alice".to_string()),
        ("seller-meter".to_string(), "bob".to_string()),
        ("other-meter".to_string(), "charlie".to_string()),
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
        assert!(compute_delivery_verifications(
            &[trade()],
            &measurements,
            &mapping,
            1_000,
            0.1,
            5.0
        )
        .unwrap()
        .is_empty());
    }
    for invalid in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        for (buyer, seller) in [(invalid, -10.0), (10.0, invalid)] {
            let measurements = [
                measurement("buyer-meter", buyer),
                measurement("seller-meter", seller),
            ];
            assert!(compute_delivery_verifications(
                &[trade()],
                &measurements,
                &mapping,
                1_000,
                0.1,
                5.0
            )
            .unwrap()
            .is_empty());
        }
    }
}

#[test]
fn verification_keeps_distinct_owners_measurements_separate() {
    // Each facility maps to one distinct owner in this deployment.
    let first_id = onchain_id("owner-1");
    let second_id = onchain_id("owner-2");
    let mapping = HashMap::from([
        ("first-meter".into(), "owner-1".into()),
        ("second-meter".into(), "owner-2".into()),
        ("seller-meter".into(), "bob".to_string()),
    ]);
    let mut first = trade();
    first.buyer = first_id.clone();
    let mut second = first.clone();
    second.trade_uuid = "trade-2".into();
    second.buyer = second_id;
    let trades = [first, second];
    let measurements = [
        measurement("first-meter", 12.0),
        measurement("seller-meter", -20.0),
    ];
    let verifications =
        compute_delivery_verifications(&trades, &measurements, &mapping, 1_000, 0.1, 5.0).unwrap();
    assert_eq!(verifications.len(), 1);
    assert_eq!(verifications[0].trade_uuid, "trade-1");
    let penalties = compute_penalties(&trades, &measurements, &mapping, 1_000, 0.1, 5.0).unwrap();
    assert_eq!(penalties.len(), 1);
    assert_eq!(penalties[0].penalized_account, first_id);
}

fn matched(id: &str, buyer: &str, seller: &str, energy: f64) -> DbTradeSchema {
    let mut trade = trade();
    trade.trade_uuid = id.into();
    trade.buyer = onchain_id(buyer);
    trade.seller = onchain_id(seller);
    trade.parameters.selected_energy_kWh = energy;
    trade
}

fn readings(values: &[(&str, f64)]) -> (Vec<MeasurementSchema>, HashMap<String, String>) {
    (
        values
            .iter()
            .map(|(id, value)| measurement(id, *value))
            .collect(),
        values
            .iter()
            .map(|(id, _)| ((*id).into(), (*id).into()))
            .collect(),
    )
}

#[test]
fn split_trades_use_the_facility_total_and_allocate_by_quantity() {
    for (reading, expected) in [
        (10.0, (0, 0)),
        (9.5, (0, 0)),
        (10.5, (0, 0)),
        (8.0, (800, 1200)),
        (12.0, (800, 1200)),
    ] {
        for is_buyer in [true, false] {
            let mut trades = if is_buyer {
                vec![
                    matched("a", "alice", "bob", 4.0),
                    matched("b", "alice", "charlie", 6.0),
                ]
            } else {
                vec![
                    matched("a", "bob", "alice", 4.0),
                    matched("b", "charlie", "alice", 6.0),
                ]
            };
            // All markets of the same slot contribute, not just one market.
            trades[1].market_id = "market-2".into();
            let sign = if is_buyer { 1.0 } else { -1.0 };
            let (measurements, mapping) = readings(&[
                ("alice", sign * reading),
                ("bob", -sign * 4.0),
                ("charlie", -sign * 6.0),
            ]);
            let verdicts =
                compute_delivery_verifications(&trades, &measurements, &mapping, 1_000, 0.1, 5.0)
                    .unwrap();
            assert_eq!(verdicts.len(), 2);
            let costs = verdicts
                .iter()
                .map(|v| {
                    if is_buyer {
                        assert_eq!(v.seller_penalty_cost, 0);
                        v.buyer_penalty_cost
                    } else {
                        assert_eq!(v.buyer_penalty_cost, 0);
                        v.seller_penalty_cost
                    }
                })
                .collect::<Vec<_>>();
            assert_eq!(
                costs,
                [expected.0, expected.1],
                "reading={reading}, is_buyer={is_buyer}"
            );
        }
    }
}

#[test]
fn mixed_roles_use_net_energy_and_allocate_over_purchases_and_sales() {
    for (bought, sold, measured, expected_buy, expected_sell) in [
        (10.0, 4.0, 6.0, 0, 0),
        (10.0, 4.0, 6.3, 0, 0),
        (10.0, 4.0, 7.0, 714, 286),
        (10.0, 4.0, 5.0, 714, 286),
        (4.0, 10.0, -6.3, 0, 0),
        (4.0, 10.0, -7.0, 286, 714),
        (4.0, 10.0, -5.0, 286, 714),
        (10.0, 10.0, 0.0, 0, 0),
        (10.0, 10.0, 1.0, 500, 500),
        (10.0, 10.0, -1.0, 500, 500),
    ] {
        let trades = [
            matched("buy", "alice", "bob", bought),
            matched("sell", "charlie", "alice", sold),
        ];
        let (measurements, mapping) =
            readings(&[("alice", measured), ("bob", -bought), ("charlie", sold)]);
        let verdicts =
            compute_delivery_verifications(&trades, &measurements, &mapping, 1_000, 0.1, 5.0)
                .unwrap();
        assert_eq!(verdicts.len(), 2);
        assert_eq!(
            (
                verdicts[0].buyer_penalty_cost,
                verdicts[1].seller_penalty_cost
            ),
            (expected_buy, expected_sell)
        );
        assert_eq!(
            (
                verdicts[0].seller_penalty_cost,
                verdicts[1].buyer_penalty_cost
            ),
            (0, 0)
        );
    }
}

#[test]
fn completed_and_unmeasured_trades_remain_in_the_allocation_baseline() {
    for (status, measured, first_cost, remaining_cost) in [
        (TradeStatus::Executed, 10.0, 0, 0),
        (TradeStatus::Settled, 10.0, 0, 0),
        (TradeStatus::Penalized, 12.0, 800, 1200),
    ] {
        let mut trades = [
            matched("a", "alice", "bob", 4.0),
            matched("b", "alice", "charlie", 6.0),
        ];
        let (mut measurements, mapping) =
            readings(&[("alice", measured), ("bob", -4.0), ("charlie", -6.0)]);
        let late = measurements.pop().unwrap();
        let initial =
            compute_delivery_verifications(&trades, &measurements, &mapping, 1_000, 0.1, 5.0)
                .unwrap();
        assert_eq!(initial.len(), 1);
        assert_eq!(initial[0].buyer_penalty_cost, first_cost);
        measurements.push(late);
        trades[0].status = status.clone();
        let verdicts =
            compute_delivery_verifications(&trades, &measurements, &mapping, 1_000, 0.1, 5.0)
                .unwrap();
        assert_eq!(
            verdicts.len(),
            if status == TradeStatus::Penalized {
                2
            } else {
                1
            }
        );
        assert_eq!(
            verdicts
                .iter()
                .find(|v| v.trade_uuid == "b")
                .unwrap()
                .buyer_penalty_cost,
            remaining_cost
        );
    }
}

#[test]
fn allocation_preserves_rounding_units_and_is_independent_of_query_order() {
    let mut trades = [
        matched("c", "alice", "bob", 1.0),
        matched("b", "alice", "bob", 1.0),
        matched("a", "alice", "bob", 1.0),
    ];
    let (mut measurements, mapping) = readings(&[("alice", 3.0001), ("bob", -3.0)]);
    for _ in 0..2 {
        let verdicts =
            compute_delivery_verifications(&trades, &measurements, &mapping, 1_000, 1.0, 0.0)
                .unwrap();
        assert_eq!(
            verdicts.iter().map(|v| v.buyer_penalty_cost).sum::<u64>(),
            1
        );
        assert_eq!(
            verdicts
                .iter()
                .find(|v| v.trade_uuid == "a")
                .unwrap()
                .buyer_penalty_cost,
            1
        );
        trades.reverse();
        measurements.reverse();
    }
    let trades = [
        matched("a", "alice", "bob", 1.0),
        matched("b", "alice", "bob", 2.0),
    ];
    let verdicts =
        compute_delivery_verifications(&trades, &measurements, &mapping, 1_000, 1.0, 0.0).unwrap();
    assert_eq!(
        (
            verdicts[0].buyer_penalty_cost,
            verdicts[1].buyer_penalty_cost
        ),
        (0, 1)
    );
}

#[test]
fn measurements_from_other_slots_cannot_verify_or_change_this_slot() {
    let trades = [trade()];
    let (mut measurements, mapping) = readings(&[("alice", 10.0), ("bob", -10.0)]);
    for reading in &mut measurements {
        reading.time_slot = 1_900;
    }
    assert!(
        compute_delivery_verifications(&trades, &measurements, &mapping, 1_000, 0.1, 5.0)
            .unwrap()
            .is_empty()
    );
    measurements.extend([measurement("alice", 12.0), measurement("bob", -10.0)]);
    for _ in 0..2 {
        let verdicts =
            compute_delivery_verifications(&trades, &measurements, &mapping, 1_000, 0.1, 5.0)
                .unwrap();
        assert_eq!(verdicts.len(), 1);
        assert_eq!(verdicts[0].buyer_penalty_cost, 2000);
        measurements.reverse();
    }
}

#[test]
fn duplicate_readings_cannot_double_charge_or_choose_a_conflicting_value() {
    let (mut measurements, mapping) = readings(&[("alice", 12.0), ("bob", -10.0)]);
    measurements.push(measurements[0].clone());
    let verdicts =
        compute_delivery_verifications(&[trade()], &measurements, &mapping, 1_000, 0.1, 5.0)
            .unwrap();
    assert_eq!(verdicts[0].buyer_penalty_cost, 2000);
    for invalid in [11.0, f64::NAN] {
        measurements.push(measurement("alice", invalid));
        for _ in 0..2 {
            assert!(compute_delivery_verifications(
                &[trade()],
                &measurements,
                &mapping,
                1_000,
                0.1,
                5.0
            )
            .unwrap()
            .is_empty());
            measurements.reverse();
        }
        measurements.pop();
    }
}

#[test]
fn invalid_trade_quantities_and_duplicates_fail_instead_of_shrinking_the_baseline() {
    let (measurements, mapping) = readings(&[("alice", 12.0), ("bob", -10.0)]);
    for energy in [0.0, -1.0, f64::NAN, f64::INFINITY, f64::MAX] {
        let invalid = matched("invalid", "alice", "bob", energy);
        assert!(compute_delivery_verifications(
            &[trade(), invalid],
            &measurements,
            &mapping,
            1_000,
            0.1,
            5.0
        )
        .is_err());
    }
    assert!(compute_delivery_verifications(
        &[trade(), trade()],
        &measurements,
        &mapping,
        1_000,
        0.1,
        5.0
    )
    .is_err());
}

#[test]
fn tolerance_is_inclusive_and_penalties_use_the_full_deviation() {
    let mapping = HashMap::from([
        ("buyer-meter".to_string(), "alice".to_string()),
        ("seller-meter".to_string(), "bob".to_string()),
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
            let verifications = compute_delivery_verifications(
                &trades,
                &measurements,
                &mapping,
                1_000,
                0.1,
                tolerance,
            )
            .unwrap();
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
            let penalties =
                compute_penalties(&trades, &measurements, &mapping, 1_000, 0.1, tolerance).unwrap();
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
