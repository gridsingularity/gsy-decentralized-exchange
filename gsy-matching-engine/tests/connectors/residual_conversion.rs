use super::*;
use primitives::db_api_schema::orders::{DbAttributes, DbRequirements, EnergyType};

fn db_order(id: u8, side: OrderEnum, energy: f64) -> DbOrderSchema {
    let id = bytes16_to_hex([id; 16]);
    DbOrderSchema {
        order_id: id.clone(),
        created_by: id.clone(),
        area_uuid: id,
        market_id: bytes16_to_hex([99; 16]),
        time_slot: 100,
        creation_time: 90,
        energy_kWh: energy,
        energy_rate: if side == OrderEnum::Bid { 20.0 } else { 10.0 },
        order_type: side,
        status: OrderStatus::Submitted,
        requirements: None,
        attributes: None,
    }
}

fn match_book(
    algorithm: &MatchingAlgorithm,
    orders: &[DbOrderSchema],
) -> (Vec<BidOfferMatch>, HashMap<String, DbOrderSchema>) {
    let (bids, offers) = orders
        .iter()
        .map(|order| convert_db_order_to_canonical(order).unwrap())
        .partition(|order| order.order_type == OrderEnum::Bid);
    let mut book = MatchingData::new(orders[0].market_id.clone(), 100, bids, offers).unwrap();
    let matches = algorithm.match_orders(&mut book).unwrap();
    let lookup = orders
        .iter()
        .map(|order| (order.order_id.clone(), order.clone()))
        .collect();
    (matches, lookup)
}

#[test]
fn preserves_offer_requirements_and_attributes_during_conversion() {
    let mut offer = db_order(1, OrderEnum::Offer, 6.0);
    offer.requirements = Some(DbRequirements {
        trading_partner_id: Some("00000000-0000-0000-0000-000000000002".to_string()),
        preferred_energy_rate: Some(12.5),
        energy_type: Some(EnergyType::Green),
    });
    offer.attributes = Some(DbAttributes {
        energy_type: EnergyType::Pv,
    });

    let converted = convert_db_order_to_canonical(&offer).unwrap();
    assert_eq!(
        converted.requirements,
        Some(Requirements {
            trading_partner_id: Some("0x00000000000000000000000000000002".to_string()),
            preferred_energy_rate: Some((12.5 * NODE_FLOAT_SCALING_FACTOR).round() as u64),
            energy_type: Some(EnergyType::Green),
        })
    );
    assert_eq!(
        converted.attributes,
        Some(Attributes {
            energy_type: EnergyType::Pv
        })
    );

    offer.requirements.as_mut().unwrap().trading_partner_id = Some("invalid-id".to_string());
    assert!(convert_db_order_to_canonical(&offer).is_err());
}

#[test]
fn converted_offer_preferences_participate_in_both_matching_algorithms() {
    for algorithm in [MatchingAlgorithm::PayAsBid, MatchingAlgorithm::PayAsClear] {
        for case in [
            "seller-only",
            "reciprocal",
            "conflicting-partner",
            "different-rates",
        ] {
            let mut bid = db_order(1, OrderEnum::Bid, 6.0);
            let mut offer = db_order(2, OrderEnum::Offer, 6.0);
            offer.requirements = Some(DbRequirements {
                trading_partner_id: Some(bid.created_by.clone()),
                preferred_energy_rate: Some(15.0),
                energy_type: None,
            });
            if case == "seller-only" {
                // Normal prices do not cross; only the seller's preference can match.
                bid.energy_rate = 15.0;
                offer.energy_rate = 20.0;
            } else {
                bid.requirements = Some(DbRequirements {
                    trading_partner_id: Some(offer.created_by.clone()),
                    preferred_energy_rate: Some(15.0),
                    energy_type: None,
                });
                // Keep fallback unavailable so an invalid preference cannot pass as standard.
                offer.energy_rate = 25.0;
                if case == "conflicting-partner" || case == "different-rates" {
                    bid.energy_rate = 5.0;
                    offer.energy_rate = 15.0;
                }
                if case == "conflicting-partner" {
                    offer.requirements.as_mut().unwrap().trading_partner_id =
                        Some(bytes16_to_hex([3; 16]));
                } else if case == "different-rates" {
                    offer.requirements.as_mut().unwrap().preferred_energy_rate = Some(16.0);
                }
            }
            let (matches, lookup) = match_book(&algorithm, &[bid, offer]);
            if case == "conflicting-partner" || case == "different-rates" {
                assert!(matches.is_empty(), "{algorithm:?}: {case}");
            } else {
                assert_eq!(matches.len(), 1, "{algorithm:?}: {case}");
                assert_eq!(
                    matches[0].energy_rate,
                    (15.0 * NODE_FLOAT_SCALING_FACTOR) as u64
                );
                assert_eq!(
                    matches[0].selected_energy,
                    (6.0 * NODE_FLOAT_SCALING_FACTOR) as u64
                );
                assert_eq!(convert_matches(matches, &lookup).unwrap().len(), 1);
            }
        }
    }
}

#[test]
fn encodes_chained_fills_from_original_orders_only() {
    for algorithm in [MatchingAlgorithm::PayAsBid, MatchingAlgorithm::PayAsClear] {
        for side in [OrderEnum::Bid, OrderEnum::Offer] {
            let is_bid = side == OrderEnum::Bid;
            let other_side = if is_bid {
                OrderEnum::Offer
            } else {
                OrderEnum::Bid
            };
            let mut parent = db_order(1, side.clone(), 25.0);
            let mut first = db_order(2, other_side.clone(), 10.0);
            let mut second = db_order(3, other_side.clone(), 10.0);
            let last = db_order(4, other_side, 5.0);
            // Two preferred fills followed by a standard fill of the final residual.
            if is_bid {
                first.energy_rate = 15.0;
                second.energy_rate = 15.0;
                second.created_by = first.created_by.clone();
                parent.requirements = Some(DbRequirements {
                    trading_partner_id: Some(first.created_by.clone()),
                    preferred_energy_rate: Some(15.0),
                    energy_type: Some(EnergyType::Green),
                });
            } else {
                parent.attributes = Some(DbAttributes {
                    energy_type: EnergyType::Pv,
                });
                for bid in [&mut first, &mut second] {
                    bid.requirements = Some(DbRequirements {
                        trading_partner_id: Some(parent.created_by.clone()),
                        preferred_energy_rate: Some(parent.energy_rate),
                        energy_type: None,
                    });
                }
            }
            let (matches, lookup) = match_book(&algorithm, &[parent.clone(), first, second, last]);
            assert_eq!(matches.len(), 3);
            let encoded = convert_matches(matches, &lookup).unwrap();
            assert_eq!(
                encoded
                    .iter()
                    .map(|item| item.match_type)
                    .collect::<Vec<_>>(),
                vec![1, 1, 0]
            );
            let mut expected = to_evm_order_data(&parent, side).unwrap();
            for item in encoded {
                let (order, residual_id) = if is_bid {
                    (&item.bid, item.residual_bid_id)
                } else {
                    (&item.offer, item.residual_offer_id)
                };
                assert_eq!(order, &expected);
                expected.energy -= item.selected_energy.as_u64();
                expected.order_id = residual_id;
            }
            assert_eq!(expected.energy, 0);
            assert_eq!(expected.order_id, [0; 16]);
        }
    }
}

#[test]
fn rejects_unknown_out_of_order_and_repeated_consumption() {
    let orders = [
        db_order(1, OrderEnum::Bid, 20.0),
        db_order(2, OrderEnum::Offer, 10.0),
        db_order(3, OrderEnum::Offer, 10.0),
    ];
    let (matches, lookup) = match_book(&MatchingAlgorithm::PayAsBid, &orders);
    let mut reversed = matches.clone();
    reversed.reverse();
    assert!(convert_matches(reversed, &lookup).is_err());
    let mut replay = matches.clone();
    replay.push(matches[0].clone());
    assert!(convert_matches(replay, &lookup).is_err());
    let mut unknown = matches;
    unknown[0].bid.order_id = bytes16_to_hex([42; 16]);
    assert!(convert_matches(unknown, &lookup).is_err());
}

#[test]
fn rejects_invalid_residual_ids_and_quantities() {
    let orders = [
        db_order(1, OrderEnum::Bid, 20.0),
        db_order(2, OrderEnum::Offer, 10.0),
    ];
    let (matches, lookup) = match_book(&MatchingAlgorithm::PayAsBid, &orders);
    for invalid in [
        "missing",
        "zero-id",
        "existing-id",
        "quantity",
        "zero-fill",
        "overfill",
        "full-fill-residual",
    ] {
        let mut invalid_matches = matches.clone();
        let item = &mut invalid_matches[0];
        match invalid {
            "missing" => item.residual_bid = None,
            "zero-id" => item.residual_bid.as_mut().unwrap().order_id = bytes16_to_hex([0; 16]),
            "existing-id" => {
                item.residual_bid.as_mut().unwrap().order_id = item.offer.order_id.clone()
            }
            "quantity" => item.residual_bid.as_mut().unwrap().energy += 1,
            "zero-fill" => item.selected_energy = 0,
            "overfill" => item.selected_energy = item.bid.energy + 1,
            "full-fill-residual" => item.residual_offer = Some(item.offer.clone()),
            _ => unreachable!(),
        }
        assert!(
            convert_matches(invalid_matches, &lookup).is_err(),
            "{invalid}"
        );
    }
}

#[test]
fn preserves_exact_integer_energy_when_encoding_a_residual() {
    let orders = [
        db_order(1, OrderEnum::Bid, 0.3),
        db_order(2, OrderEnum::Offer, 0.1),
        db_order(3, OrderEnum::Offer, 0.2),
    ];
    let (matches, lookup) = match_book(&MatchingAlgorithm::PayAsBid, &orders);
    let encoded = convert_matches(matches, &lookup).unwrap();
    assert_eq!(encoded.len(), 2);
    assert_eq!(encoded[1].bid.order_id, encoded[0].residual_bid_id);
    assert_eq!(
        encoded[1].bid.energy,
        encoded[0].bid.energy - encoded[0].selected_energy.as_u64()
    );
    assert_eq!(encoded[1].residual_bid_id, [0; 16]);
}

// Exercise the production market-batch encoder while keeping assertions per match.
fn convert_matches(
    matches: Vec<BidOfferMatch>,
    lookup: &HashMap<String, DbOrderSchema>,
) -> Result<Vec<Match>> {
    let clearing_result = ClearingResult {
        market_id: matches.first().map(|item| item.market_id.clone()),
        traded_quantity: Some(matches.iter().map(|item| item.selected_energy).sum()),
        num_trades: Some(matches.len() as u32),
        ..Default::default()
    };
    Ok(to_evm_matches(
        vec![MarketMatches {
            bid_offer_matches: matches,
            clearing_result,
        }],
        lookup,
    )?
    .into_iter()
    .flat_map(|market| market.matches)
    .collect())
}
