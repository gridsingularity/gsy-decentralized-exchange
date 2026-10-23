#[cfg(test)]
mod tests {
    use gsy_execution_engine::primitives::penalty_calculator::{
        build_measurement_map, compute_penalties, evaluated_trade_uuids, metering_point_index,
        MeasuredEnergy, Penalty, PenaltyReason,
    };
    use gsy_offchain_primitives::db_api_schema::orders::{DbBid, DbOffer, DbOrderComponent};
    use gsy_offchain_primitives::db_api_schema::profiles::{
        MeasurementCompleteness, MeasurementSchema, MeteringPointMeasurement,
    };
    use gsy_offchain_primitives::db_api_schema::trades::{
        TradeParameters, TradeSchema, TradeStatus,
    };
    use gsy_offchain_primitives::utils::{community_id_from_uuid, h256_to_string};
    use std::collections::HashMap;

    const TIME_SLOT: u64 = 100;
    const PENALTY_RATE: f64 = 0.10;

    fn measurement(area_hash: &str, community_uuid: &str, energy_kwh: f64) -> MeasurementSchema {
        MeasurementSchema {
            area_uuid: format!("{}_uuid", area_hash),
            area_hash: area_hash.to_string(),
            community_uuid: community_uuid.to_string(),
            time_slot: TIME_SLOT,
            creation_time: 0,
            energy_kwh,
            metering_point: None,
        }
    }

    fn order_component(area_uuid: &str, market_id: &str, energy: f64) -> DbOrderComponent {
        DbOrderComponent {
            area_uuid: area_uuid.to_string(),
            market_id: market_id.to_string(),
            time_slot: TIME_SLOT,
            creation_time: 0,
            energy,
            energy_rate: 1.0,
        }
    }

    fn trade(
        buyer: &str,
        seller: &str,
        bid_area: &str,
        market_id: &str,
        selected_energy: f64,
        trade_uuid: &str,
    ) -> TradeSchema {
        // Default creation_time 0; use `trade_at` when the waterfall order matters.
        trade_at(buyer, seller, bid_area, market_id, selected_energy, trade_uuid, 0)
    }

    #[allow(clippy::too_many_arguments)]
    fn trade_at(
        buyer: &str,
        seller: &str,
        bid_area: &str,
        market_id: &str,
        selected_energy: f64,
        trade_uuid: &str,
        creation_time: u64,
    ) -> TradeSchema {
        TradeSchema {
            _id: trade_uuid.to_string(),
            status: TradeStatus::Settled,
            seller: seller.to_string(),
            buyer: buyer.to_string(),
            market_id: market_id.to_string(),
            time_slot: TIME_SLOT,
            trade_uuid: trade_uuid.to_string(),
            creation_time,
            status_updated_at: None,
            offer: DbOffer {
                seller: seller.to_string(),
                nonce: 0,
                offer_component: order_component(
                    &format!("{}_seller", bid_area),
                    market_id,
                    selected_energy,
                ),
            },
            offer_hash: String::new(),
            bid: DbBid {
                buyer: buyer.to_string(),
                nonce: 0,
                bid_component: order_component(bid_area, market_id, selected_energy),
            },
            bid_hash: String::new(),
            residual_offer: None,
            residual_bid: None,
            parameters: TradeParameters {
                selected_energy,
                energy_rate: 1.0,
                trade_uuid: trade_uuid.to_string(),
            },
        }
    }

    /// Byte-for-byte copy of the pre-change `compute_penalties` (per-asset lookup only).
    /// Used to prove the spot settlement path is unchanged by the community-aggregate addition.
    fn legacy_compute_penalties(
        trades: &[TradeSchema],
        measurements: &[MeasurementSchema],
        penalty_rate: f64,
    ) -> Vec<Penalty> {
        let mut penalties = Vec::new();
        let mut measurement_map: HashMap<String, f64> = HashMap::new();
        for meas in measurements {
            measurement_map.insert(meas.area_hash.clone(), meas.energy_kwh);
        }
        for trade in trades {
            if let Some(&measured_energy) =
                measurement_map.get(&trade.bid.bid_component.area_uuid.clone())
            {
                let traded_energy = trade.parameters.selected_energy;
                let delta = measured_energy - traded_energy;
                if delta > 0.0 {
                    let raw_penalty = delta * penalty_rate;
                    let penalty_cost = (raw_penalty * 10_000.0).round() as u64;
                    penalties.push(Penalty {
                        penalized_account: trade.buyer.clone(),
                        market_id: trade.offer.offer_component.market_id.clone(),
                        trade_uuid: trade.trade_uuid.clone(),
                        penalty_cost,
                        reason: PenaltyReason::Deviation,
                    });
                } else if delta < 0.0 {
                    let raw_penalty = (-delta) * penalty_rate;
                    let penalty_cost = (raw_penalty * 10_000.0).round() as u64;
                    penalties.push(Penalty {
                        penalized_account: trade.seller.clone(),
                        market_id: trade.market_id.clone(),
                        trade_uuid: trade.trade_uuid.clone(),
                        penalty_cost,
                        reason: PenaltyReason::Deviation,
                    });
                }
            }
        }
        penalties
    }

    fn as_tuples(penalties: &[Penalty]) -> Vec<(String, String, String, u64)> {
        penalties
            .iter()
            .map(|p| {
                (
                    p.penalized_account.clone(),
                    p.market_id.clone(),
                    p.trade_uuid.clone(),
                    p.penalty_cost,
                )
            })
            .collect()
    }

    #[test]
    fn inter_community_trade_settles_against_community_net() {
        // Community "CommA" per-asset measurements: mixed +consumption / -production.
        // net import = 5.0 - 3.0 + 1.0 = 3.0 (Σ consumption − Σ production).
        let measurements = vec![
            measurement("assetA1", "CommA", 5.0),
            measurement("assetA2", "CommA", -3.0),
            measurement("assetA3", "CommA", 1.0),
        ];

        // One inter-community trade: its area_uuid is the community hash, not any area_hash.
        let community_area = h256_to_string(community_id_from_uuid("CommA"));
        let market_id = "inter_community_market";
        let traded_energy = 2.0;
        let trades = vec![trade(
            "buyer_acc",
            "seller_acc",
            &community_area,
            market_id,
            traded_energy,
            "trade-ic-1",
        )];

        let penalties = compute_penalties(&trades, &measurements, PENALTY_RATE);

        // delta = measured(net 3.0) - traded(2.0) = 1.0 > 0 -> buyer penalized.
        // penalty_cost = (1.0 * 0.10 * 10_000).round() = 1000.
        assert_eq!(penalties.len(), 1);
        assert_eq!(penalties[0].penalized_account, "buyer_acc");
        assert_eq!(penalties[0].market_id, market_id);
        assert_eq!(penalties[0].trade_uuid, "trade-ic-1");
        assert_eq!(penalties[0].penalty_cost, 1000);
    }

    #[test]
    fn spot_penalties_are_byte_identical_to_pre_change() {
        // A per-asset (spot) measurement and matching spot trade, PLUS community
        // measurements that now also produce community-aggregate entries. The spot
        // lookup keys off the per-asset area_hash, which lives in a different key space,
        // so the community aggregates must not perturb the spot penalties at all.
        let measurements = vec![
            measurement("spot_asset_hash", "SpotComm", 6.0),
            // Extra community members -> exercise the aggregate-insertion branch.
            measurement("other_asset_1", "SpotComm", -2.0),
            measurement("other_asset_2", "OtherComm", 4.0),
        ];

        let market_id = "spot_market";
        let traded_energy = 4.0;
        let trades = vec![trade(
            "spot_buyer",
            "spot_seller",
            "spot_asset_hash",
            market_id,
            traded_energy,
            "trade-spot-1",
        )];

        let new_penalties = compute_penalties(&trades, &measurements, PENALTY_RATE);
        let legacy_penalties = legacy_compute_penalties(&trades, &measurements, PENALTY_RATE);

        // Byte-identical to the pre-change implementation for the same inputs.
        assert_eq!(as_tuples(&new_penalties), as_tuples(&legacy_penalties));

        // And the concrete expected spot penalty: delta = 6.0 - 4.0 = 2.0 > 0 -> buyer,
        // penalty_cost = (2.0 * 0.10 * 10_000).round() = 2000.
        assert_eq!(new_penalties.len(), 1);
        assert_eq!(new_penalties[0].penalized_account, "spot_buyer");
        assert_eq!(new_penalties[0].market_id, market_id);
        assert_eq!(new_penalties[0].penalty_cost, 2000);
    }

    // The `trade` helper places the offer (seller) area at `format!("{bid_area}_seller")`,
    // so a measurement for that key exercises the seller-side lookup, while a measurement
    // for `bid_area` exercises the buyer-side lookup. The two are independent.

    /// (a) Seller underproduces (measured production < selected_energy) -> seller penalized
    /// on the shortfall, judged by the OFFER area's meter (the original bug used the bid meter).
    #[test]
    fn seller_underproduction_penalizes_seller_on_shortfall() {
        // Seller area = "buyerA_seller", measured net = -3.0 kWh -> production magnitude 3.0.
        // No buyer-side measurement, so only the seller check can fire.
        let measurements = vec![measurement("buyerA_seller", "Comm", -3.0)];

        let market_id = "prod_market";
        let selected_energy = 5.0;
        let trades = vec![trade(
            "buyer_acc",
            "seller_acc",
            "buyerA",
            market_id,
            selected_energy,
            "trade-a",
        )];

        let penalties = compute_penalties(&trades, &measurements, PENALTY_RATE);

        // shortfall = 5.0 - 3.0 = 2.0; penalty_cost = (2.0 * 0.10 * 10_000).round() = 2000.
        assert_eq!(penalties.len(), 1);
        assert_eq!(penalties[0].penalized_account, "seller_acc");
        assert_eq!(penalties[0].market_id, market_id); // seller uses trade.market_id
        assert_eq!(penalties[0].trade_uuid, "trade-a");
        assert_eq!(penalties[0].penalty_cost, 2000);
    }

    /// (b) Seller overproduces (measured production >= selected_energy) -> no penalty.
    #[test]
    fn seller_overproduction_yields_no_penalty() {
        // Seller area measured net = -10.0 -> production magnitude 10.0 >= 5.0 traded.
        let measurements = vec![measurement("buyerB_seller", "Comm", -10.0)];

        let trades = vec![trade(
            "buyer_acc",
            "seller_acc",
            "buyerB",
            "prod_market",
            5.0,
            "trade-b",
        )];

        let penalties = compute_penalties(&trades, &measurements, PENALTY_RATE);

        assert!(penalties.is_empty());
    }

    /// (c) Buyer over-consumes (measured consumption > selected_energy) -> buyer penalized
    /// on the excess (existing behavior preserved).
    #[test]
    fn buyer_overconsumption_penalizes_buyer_on_excess() {
        // Buyer (bid) area measured net = +8.0 (consumption) > 5.0 traded.
        let measurements = vec![measurement("buyerC", "Comm", 8.0)];

        let market_id = "cons_market";
        let trades = vec![trade(
            "buyer_acc",
            "seller_acc",
            "buyerC",
            market_id,
            5.0,
            "trade-c",
        )];

        let penalties = compute_penalties(&trades, &measurements, PENALTY_RATE);

        // excess = 8.0 - 5.0 = 3.0; penalty_cost = (3.0 * 0.10 * 10_000).round() = 3000.
        assert_eq!(penalties.len(), 1);
        assert_eq!(penalties[0].penalized_account, "buyer_acc");
        // buyer uses trade.offer.offer_component.market_id (same value as market_id here).
        assert_eq!(penalties[0].market_id, market_id);
        assert_eq!(penalties[0].trade_uuid, "trade-c");
        assert_eq!(penalties[0].penalty_cost, 3000);
    }

    /// (d) Production trade with NO buyer measurement but a seller measurement present ->
    /// the seller penalty is still computed. Guards the original bug where a missing buyer
    /// measurement skipped the whole trade and the seller was never checked.
    #[test]
    fn seller_penalty_computed_without_buyer_measurement() {
        // Only the seller (offer) area has a measurement: net = -2.0 -> production 2.0 < 5.0.
        let measurements = vec![measurement("buyerD_seller", "Comm", -2.0)];

        let market_id = "prod_market";
        let trades = vec![trade(
            "buyer_acc",
            "seller_acc",
            "buyerD",
            market_id,
            5.0,
            "trade-d",
        )];

        let penalties = compute_penalties(&trades, &measurements, PENALTY_RATE);

        // shortfall = 5.0 - 2.0 = 3.0; penalty_cost = (3.0 * 0.10 * 10_000).round() = 3000.
        assert_eq!(penalties.len(), 1);
        assert_eq!(penalties[0].penalized_account, "seller_acc");
        assert_eq!(penalties[0].market_id, market_id);
        assert_eq!(penalties[0].trade_uuid, "trade-d");
        assert_eq!(penalties[0].penalty_cost, 3000);
    }

    /// (e) Buyer under-consumes while the seller delivered in full -> NO penalty for anyone.
    /// Guards against the old conflated `delta < 0` branch, which penalized the SELLER for the
    /// BUYER's under-consumption (single signed delta on the buyer's meter).
    #[test]
    fn buyer_underconsumption_with_full_delivery_yields_no_penalty() {
        let measurements = vec![
            // Buyer area consumed only 2.0 (< 5.0 traded) -> buyer check must NOT fire.
            measurement("buyerE", "Comm", 2.0),
            // Seller delivered in full: production magnitude 5.0 >= 5.0 -> seller check must NOT fire.
            measurement("buyerE_seller", "Comm", -5.0),
        ];

        let trades = vec![trade(
            "buyer_acc",
            "seller_acc",
            "buyerE",
            "spot_market",
            5.0,
            "trade-e",
        )];

        let penalties = compute_penalties(&trades, &measurements, PENALTY_RATE);

        assert!(penalties.is_empty());
    }

    /// (1) A PV offer of 5 kWh matched into two trades (2 + 3), actual production 4 kWh.
    /// Per-trade checks would have emitted ZERO (4 >= 2 and 4 >= 3). Under the new
    /// waterfall the measured production fills the trades in `(creation_time, trade_uuid)`
    /// order, so the earlier (2.0) trade is fully covered and the LATER (3.0) trade absorbs
    /// the whole 1 kWh shortfall.
    #[test]
    fn seller_aggregate_shortfall_is_waterfalled_to_later_trade() {
        // Single seller-area measurement: net -4.0 -> production magnitude 4.0.
        let measurements = vec![measurement("buyerAgg_seller", "Comm", -4.0)];

        let market_id = "prod_market";
        // The 2.0 trade is EARLIER (creation_time 1), the 3.0 trade is LATER (creation_time 2).
        let trades = vec![
            trade_at("buyer_acc", "seller_acc", "buyerAgg", market_id, 2.0, "trade-1", 1),
            trade_at("buyer_acc", "seller_acc", "buyerAgg", market_id, 3.0, "trade-2", 2),
        ];

        let penalties = compute_penalties(&trades, &measurements, PENALTY_RATE);

        // Waterfall: production 4.0 covers the earlier 2.0 trade in full (no penalty),
        // then covers 2.0 of the later 3.0 trade -> uncovered 1.0 on the later trade only.
        // penalty_cost = (1.0 * 0.10 * 10_000).round() = 1000 on trade-2.
        // (The old pro-rata behavior would have emitted 400 on trade-1 + 600 on trade-2.)
        assert_eq!(penalties.len(), 1);
        assert_eq!(penalties[0].penalized_account, "seller_acc");
        assert_eq!(penalties[0].market_id, market_id);
        assert_eq!(penalties[0].trade_uuid, "trade-2");
        assert_eq!(penalties[0].penalty_cost, 1000);
    }

    /// (2) Aggregate sold exactly equals production -> no penalty.
    #[test]
    fn seller_aggregate_exactly_met_yields_no_penalty() {
        // Production magnitude 5.0 == total sold 5.0.
        let measurements = vec![measurement("buyerMet_seller", "Comm", -5.0)];

        let trades = vec![
            trade("buyer_acc", "seller_acc", "buyerMet", "prod_market", 2.0, "trade-1"),
            trade("buyer_acc", "seller_acc", "buyerMet", "prod_market", 3.0, "trade-2"),
        ];

        let penalties = compute_penalties(&trades, &measurements, PENALTY_RATE);

        assert!(penalties.is_empty());
    }

    /// (3) Buyer mirror: two bids for the same buyer area, aggregate consumption exceeds
    /// the summed bought energy -> aggregate excess penalized and apportioned.
    #[test]
    fn buyer_aggregate_excess_penalized_and_apportioned() {
        // Buyer (bid) area measured net = +6.0 (consumption); total bought = 5.0.
        let measurements = vec![measurement("buyerXs", "Comm", 6.0)];

        let market_id = "cons_market";
        let trades = vec![
            trade("buyer_acc", "seller_acc", "buyerXs", market_id, 2.0, "trade-1"),
            trade("buyer_acc", "seller_acc", "buyerXs", market_id, 3.0, "trade-2"),
        ];

        let penalties = compute_penalties(&trades, &measurements, PENALTY_RATE);

        // aggregate_excess = 6.0 - 5.0 = 1.0; aggregate_penalty_cost = 1000.
        // Apportioned 2/5 and 3/5 -> 400 and 600 to the buyer account.
        assert_eq!(penalties.len(), 2);
        for p in &penalties {
            assert_eq!(p.penalized_account, "buyer_acc");
            assert_eq!(p.market_id, market_id);
        }
        let tuples = as_tuples(&penalties);
        assert!(tuples.contains(&(
            "buyer_acc".to_string(),
            market_id.to_string(),
            "trade-1".to_string(),
            400
        )));
        assert!(tuples.contains(&(
            "buyer_acc".to_string(),
            market_id.to_string(),
            "trade-2".to_string(),
            600
        )));
        let total: u64 = penalties.iter().map(|p| p.penalty_cost).sum();
        assert_eq!(total, 1000);
    }

    /// (4) Seller waterfall across three trades where production covers the first fully,
    /// the second partially, and the third not at all. Verifies the fill order
    /// `(creation_time, trade_uuid)` and that only the uncovered tail is penalized.
    #[test]
    fn seller_waterfall_partial_then_full_shortfall() {
        // Three trades of 1.0 each on the same seller area, creation_times 1, 2, 3.
        // Production magnitude 1.5 (net -1.5): covers trade-1 in full, 0.5 of trade-2,
        // and 0.0 of trade-3.
        let measurements = vec![measurement("buyerLR_seller", "Comm", -1.5)];

        let market_id = "prod_market";
        let trades = vec![
            trade_at("buyer_acc", "seller_acc", "buyerLR", market_id, 1.0, "trade-1", 1),
            trade_at("buyer_acc", "seller_acc", "buyerLR", market_id, 1.0, "trade-2", 2),
            trade_at("buyer_acc", "seller_acc", "buyerLR", market_id, 1.0, "trade-3", 3),
        ];

        let penalties = compute_penalties(&trades, &measurements, PENALTY_RATE);

        // trade-1: covered 1.0 -> uncovered 0.0 -> no penalty.
        // trade-2: covered 0.5 -> uncovered 0.5 -> penalty (0.5 * 0.10 * 10_000) = 500.
        // trade-3: covered 0.0 -> uncovered 1.0 -> penalty (1.0 * 0.10 * 10_000) = 1000.
        assert_eq!(penalties.len(), 2);
        for p in &penalties {
            assert_eq!(p.penalized_account, "seller_acc");
            assert_eq!(p.market_id, market_id);
        }
        let by_uuid: HashMap<String, u64> = penalties
            .iter()
            .map(|p| (p.trade_uuid.clone(), p.penalty_cost))
            .collect();
        assert!(!by_uuid.contains_key("trade-1"));
        assert_eq!(by_uuid["trade-2"], 500);
        assert_eq!(by_uuid["trade-3"], 1000);
    }

    /// (5) Two inter-community trades under the same community hash / slot: their summed
    /// selected_energy is compared ONCE against the community net-import aggregate, and the
    /// aggregate penalty is apportioned across the two trades.
    #[test]
    fn inter_community_multiple_trades_aggregate() {
        // Community "CommAgg" per-asset measurements:
        // net import = 5.0 - 1.0 + 1.0 = 5.0 (Σ consumption − Σ production).
        let measurements = vec![
            measurement("assetB1", "CommAgg", 5.0),
            measurement("assetB2", "CommAgg", -1.0),
            measurement("assetB3", "CommAgg", 1.0),
        ];

        let community_area = h256_to_string(community_id_from_uuid("CommAgg"));
        let market_id = "inter_community_market";
        let trades = vec![
            trade("buyer_acc", "seller_acc", &community_area, market_id, 1.0, "trade-ic-1"),
            trade("buyer_acc", "seller_acc", &community_area, market_id, 3.0, "trade-ic-2"),
        ];

        let penalties = compute_penalties(&trades, &measurements, PENALTY_RATE);

        // Buyer side: total_bought = 4.0; measured net import 5.0 > 4.0 -> excess 1.0.
        // aggregate_penalty_cost = 1000. Apportioned 1/4 and 3/4 -> 250 and 750.
        assert_eq!(penalties.len(), 2);
        for p in &penalties {
            assert_eq!(p.penalized_account, "buyer_acc");
            assert_eq!(p.market_id, market_id);
        }
        let by_uuid: HashMap<String, u64> = penalties
            .iter()
            .map(|p| (p.trade_uuid.clone(), p.penalty_cost))
            .collect();
        assert_eq!(by_uuid["trade-ic-1"], 250);
        assert_eq!(by_uuid["trade-ic-2"], 750);
        let total: u64 = penalties.iter().map(|p| p.penalty_cost).sum();
        assert_eq!(total, 1000);
    }

    #[test]
    fn build_measurement_map_keys_areas_and_community_aggregate() {
        let measurements = vec![
            measurement("assetA1", "CommA", 5.0),
            measurement("assetA2", "CommA", -3.0),
            measurement("assetA3", "CommA", 1.0),
        ];

        let map = build_measurement_map(&measurements);

        let key = |hash: &str| (hash.to_string(), TIME_SLOT);
        assert_eq!(map[&key("assetA1")], MeasuredEnergy::Energy(5.0));
        assert_eq!(map[&key("assetA2")], MeasuredEnergy::Energy(-3.0));
        assert_eq!(map[&key("assetA3")], MeasuredEnergy::Energy(1.0));
        // net import = 5.0 - 3.0 + 1.0 = 3.0 (Σ consumption − Σ production).
        let community_key = key(&h256_to_string(community_id_from_uuid("CommA")));
        assert_eq!(map[&community_key], MeasuredEnergy::Energy(3.0));
        assert_eq!(map.len(), 4);
    }

    #[test]
    fn evaluated_trade_uuids_includes_trade_measured_on_seller_side_only() {
        // Mirrors pv_penalty: PV (seller) is measured, the buyer's meter is not.
        let measurements = vec![measurement("buyerA_seller", "Comm", -3.0)];
        let trades = vec![trade(
            "buyer_acc",
            "seller_acc",
            "buyerA",
            "prod_market",
            5.0,
            "trade-a",
        )];

        let evaluated = evaluated_trade_uuids(&trades, &measurements);

        assert_eq!(evaluated, vec!["trade-a".to_string()]);
    }

    #[test]
    fn evaluated_trade_uuids_excludes_trades_with_no_measurement_on_either_side() {
        // Regression test for the premature-`Executed` bug: an early polling cycle sees no
        // measurements for a slot's trades yet, and must not report any of them as evaluated
        // (which would let the offchain listener mark them `Executed` before they are judged).
        let measurements = vec![measurement("unrelated_area", "Comm", 4.0)];
        let trades = vec![trade(
            "buyer_acc",
            "seller_acc",
            "buyerA",
            "prod_market",
            5.0,
            "trade-a",
        )];

        let evaluated = evaluated_trade_uuids(&trades, &measurements);

        assert!(evaluated.is_empty());

        let evaluated_no_measurements = evaluated_trade_uuids(&trades, &[]);
        assert!(evaluated_no_measurements.is_empty());
    }

    #[test]
    fn evaluated_trade_uuids_includes_inter_community_trades_via_the_community_aggregate_key() {
        let measurements = vec![
            measurement("assetA1", "CommA", 5.0),
            measurement("assetA2", "CommA", -3.0),
            measurement("assetA3", "CommA", 1.0),
        ];

        let community_area = h256_to_string(community_id_from_uuid("CommA"));
        let trades = vec![trade(
            "buyer_acc",
            "seller_acc",
            &community_area,
            "inter_community_market",
            2.0,
            "trade-ic-1",
        )];

        let evaluated = evaluated_trade_uuids(&trades, &measurements);

        assert_eq!(evaluated, vec!["trade-ic-1".to_string()]);
    }

    #[test]
    fn evaluated_trade_uuids_deduplicates() {
        let measurements = vec![measurement("buyerA", "Comm", 4.0)];
        let trades = vec![
            trade("buyer_acc", "seller_acc", "buyerA", "market", 5.0, "trade-a"),
            trade("buyer_acc", "seller_acc", "buyerA", "market", 5.0, "trade-a"),
        ];

        let evaluated = evaluated_trade_uuids(&trades, &measurements);

        assert_eq!(evaluated, vec!["trade-a".to_string()]);
    }

    #[test]
    fn evaluated_and_penalized_sets_partition_the_pv_waterfall_case() {
        // Same setup as `seller_aggregate_shortfall_is_waterfalled_to_later_trade`: production
        // (4.0) covers the earlier (2.0) trade in full and leaves the later (3.0) trade with a
        // 1.0 shortfall. Both trades were measured (same seller area), so both must be
        // evaluated, while only the later trade is penalized -- proving the evaluated and
        // penalized sets partition correctly, which is what the offchain listener relies on to
        // classify each trade exactly once.
        let measurements = vec![measurement("buyerAgg_seller", "Comm", -4.0)];

        let market_id = "prod_market";
        let trades = vec![
            trade_at("buyer_acc", "seller_acc", "buyerAgg", market_id, 2.0, "trade-1", 1),
            trade_at("buyer_acc", "seller_acc", "buyerAgg", market_id, 3.0, "trade-2", 2),
        ];

        let evaluated = evaluated_trade_uuids(&trades, &measurements);
        assert_eq!(
            evaluated,
            vec!["trade-1".to_string(), "trade-2".to_string()]
        );

        let penalties = compute_penalties(&trades, &measurements, PENALTY_RATE);
        assert_eq!(penalties.len(), 1);
        assert_eq!(penalties[0].trade_uuid, "trade-2");
    }

    // --- Per-slot keys, metering points and unreliable community aggregates ---

    const NEXT_SLOT: u64 = TIME_SLOT + 900;
    const SPOT_MARKET: &str = "spot_market";

    fn measurement_at(
        area_hash: &str,
        community_uuid: &str,
        energy_kwh: f64,
        time_slot: u64,
    ) -> MeasurementSchema {
        MeasurementSchema {
            time_slot,
            ..measurement(area_hash, community_uuid, energy_kwh)
        }
    }

    /// A metering point's row. Incomplete and missing rows carry 0.0, as storage posts them.
    fn point_row(
        name: &str,
        community_uuid: &str,
        members: &[&str],
        completeness: MeasurementCompleteness,
        energy_kwh: f64,
        time_slot: u64,
    ) -> MeasurementSchema {
        let missing_meters = match completeness {
            MeasurementCompleteness::Complete => Vec::new(),
            _ => vec![format!("{}_meter", name)],
        };
        MeasurementSchema {
            area_uuid: format!("{}_uuid", name),
            area_hash: format!("{}_hash", name),
            community_uuid: community_uuid.to_string(),
            time_slot,
            creation_time: 0,
            energy_kwh,
            metering_point: Some(MeteringPointMeasurement {
                name: name.to_string(),
                member_area_hashes: members.iter().map(|m| m.to_string()).collect(),
                completeness,
                missing_meters,
            }),
        }
    }

    fn complete_point(
        name: &str,
        members: &[&str],
        energy_kwh: f64,
        time_slot: u64,
    ) -> MeasurementSchema {
        point_row(name, "Site", members, MeasurementCompleteness::Complete, energy_kwh, time_slot)
    }

    /// A trade between `bid_area` and `offer_area` in `time_slot`. The buyer account is
    /// `{uuid}-buyer` and the seller account `{uuid}-seller`.
    fn side_trade(
        uuid: &str,
        bid_area: &str,
        offer_area: &str,
        selected_energy: f64,
        time_slot: u64,
        creation_time: u64,
    ) -> TradeSchema {
        let mut trade = trade_at(
            &format!("{}-buyer", uuid),
            &format!("{}-seller", uuid),
            bid_area,
            SPOT_MARKET,
            selected_energy,
            uuid,
            creation_time,
        );
        trade.time_slot = time_slot;
        trade.bid.bid_component.time_slot = time_slot;
        trade.offer.offer_component.area_uuid = offer_area.to_string();
        trade.offer.offer_component.time_slot = time_slot;
        trade
    }

    fn buyer_penalty(uuid: &str, penalty_cost: u64, reason: PenaltyReason) -> Penalty {
        Penalty {
            penalized_account: format!("{}-buyer", uuid),
            market_id: SPOT_MARKET.to_string(),
            trade_uuid: uuid.to_string(),
            penalty_cost,
            reason,
        }
    }

    fn seller_penalty(uuid: &str, penalty_cost: u64, reason: PenaltyReason) -> Penalty {
        Penalty {
            penalized_account: format!("{}-seller", uuid),
            market_id: SPOT_MARKET.to_string(),
            trade_uuid: uuid.to_string(),
            penalty_cost,
            reason,
        }
    }

    fn missing(source: &str) -> PenaltyReason {
        PenaltyReason::MissingMeasurement {
            source: source.to_string(),
        }
    }

    /// A 3 kWh purchase by the building's load from outside and a 2 kWh sale by the building's
    /// PV to outside: committed net import 3 - 2 = 1 kWh.
    fn d1_trades(suffix: &str, time_slot: u64) -> Vec<TradeSchema> {
        let bid_uuid = format!("bid-trade{}", suffix);
        let offer_uuid = format!("offer-trade{}", suffix);
        vec![
            side_trade(&bid_uuid, "house_load", "outside_pv", 3.0, time_slot, 0),
            side_trade(&offer_uuid, "outside_load", "house_pv", 2.0, time_slot, 0),
        ]
    }

    const HOUSE: &[&str] = &["house_load", "house_pv"];

    #[test]
    fn complete_point_d1_vectors() {
        let trades = d1_trades("", TIME_SLOT);
        let cases = vec![
            // deviation 0 -> clean.
            (1.0, vec![]),
            // deviation 1 -> the offer keeps 1 of its 2 kWh.
            (2.0, vec![seller_penalty("offer-trade", 1000, PenaltyReason::Deviation)]),
            // deviation 3 -> the offer is charged its full 2 kWh, the other 1 kWh goes to the bid.
            (
                4.0,
                vec![
                    seller_penalty("offer-trade", 2000, PenaltyReason::Deviation),
                    buyer_penalty("bid-trade", 1000, PenaltyReason::Deviation),
                ],
            ),
            // deviation -2 (under-consumption / over-production) -> clean.
            (-1.0, vec![]),
        ];
        for (measured, expected) in cases {
            let measurements = vec![complete_point("House", HOUSE, measured, TIME_SLOT)];
            let penalties = compute_penalties(&trades, &measurements, PENALTY_RATE);
            assert_eq!(penalties, expected, "measured {}", measured);
            assert_eq!(
                evaluated_trade_uuids(&trades, &measurements),
                vec!["bid-trade".to_string(), "offer-trade".to_string()]
            );
        }
    }

    #[test]
    fn complete_point_waterfall_charges_the_later_offer_first() {
        // Two 2 kWh sales, the later one listed first. Measured net export 3: deviation
        // -3 - (-4) = 1, so the offers keep 3 kWh in time priority.
        let trades = vec![
            side_trade("late", "outside_load", "house_pv", 2.0, TIME_SLOT, 2),
            side_trade("early", "outside_load", "house_pv", 2.0, TIME_SLOT, 1),
        ];
        let measurements = vec![complete_point("House", HOUSE, -3.0, TIME_SLOT)];

        let penalties = compute_penalties(&trades, &measurements, PENALTY_RATE);

        assert_eq!(penalties, vec![seller_penalty("late", 1000, PenaltyReason::Deviation)]);
    }

    #[test]
    fn complete_point_remainder_without_member_bids_is_not_charged() {
        // Sold 4, measured net import 1: deviation 5. The offers are charged their full 4 kWh;
        // the remaining 1 kWh has no member bid to go to.
        let trades = vec![
            side_trade("early", "outside_load", "house_pv", 2.0, TIME_SLOT, 1),
            side_trade("late", "outside_load", "house_pv", 2.0, TIME_SLOT, 2),
        ];
        let measurements = vec![complete_point("House", HOUSE, 1.0, TIME_SLOT)];

        let penalties = compute_penalties(&trades, &measurements, PENALTY_RATE);

        assert_eq!(
            penalties,
            vec![
                seller_penalty("early", 2000, PenaltyReason::Deviation),
                seller_penalty("late", 2000, PenaltyReason::Deviation),
            ]
        );
    }

    #[test]
    fn complete_point_self_consumption_nets_out() {
        // The building's PV sells 2 kWh to its own load: bought and sold net to zero.
        let trades = vec![side_trade("self", "house_load", "house_pv", 2.0, TIME_SLOT, 0)];

        let clean = vec![complete_point("House", HOUSE, 0.0, TIME_SLOT)];
        assert!(compute_penalties(&trades, &clean, PENALTY_RATE).is_empty());
        assert_eq!(evaluated_trade_uuids(&trades, &clean), vec!["self".to_string()]);

        // The building imports 1 kWh more: the offer side is charged first.
        let importing = vec![complete_point("House", HOUSE, 1.0, TIME_SLOT)];
        assert_eq!(
            compute_penalties(&trades, &importing, PENALTY_RATE),
            vec![seller_penalty("self", 1000, PenaltyReason::Deviation)]
        );
    }

    #[test]
    fn consecutive_slots_at_a_point_are_judged_independently() {
        // Same trades in both slots; the building's net is 1 in the first slot (clean) and 4 in
        // the second.
        let mut trades = d1_trades("-1", TIME_SLOT);
        trades.extend(d1_trades("-2", NEXT_SLOT));
        let measurements = vec![
            complete_point("House", HOUSE, 1.0, TIME_SLOT),
            complete_point("House", HOUSE, 4.0, NEXT_SLOT),
        ];

        let penalties = compute_penalties(&trades, &measurements, PENALTY_RATE);

        assert_eq!(
            penalties,
            vec![
                seller_penalty("offer-trade-2", 2000, PenaltyReason::Deviation),
                buyer_penalty("bid-trade-2", 1000, PenaltyReason::Deviation),
            ]
        );
    }

    #[test]
    fn per_area_measurement_only_judges_its_own_slot() {
        // Regression for the area-only key, where the last row won for every slot.
        let trade_next_slot = vec![side_trade("next", "buyerZ", "sellerZ", 5.0, NEXT_SLOT, 0)];
        let only_first_slot = vec![measurement_at("buyerZ", "Comm", 8.0, TIME_SLOT)];
        assert!(compute_penalties(&trade_next_slot, &only_first_slot, PENALTY_RATE).is_empty());
        assert!(evaluated_trade_uuids(&trade_next_slot, &only_first_slot).is_empty());

        // Both slots measured: 8 kWh (excess 3) in the first, exactly 5 kWh in the second.
        let trades = vec![
            side_trade("first", "buyerZ", "sellerZ", 5.0, TIME_SLOT, 0),
            side_trade("next", "buyerZ", "sellerZ", 5.0, NEXT_SLOT, 0),
        ];
        let measurements = vec![
            measurement_at("buyerZ", "Comm", 8.0, TIME_SLOT),
            measurement_at("buyerZ", "Comm", 5.0, NEXT_SLOT),
        ];
        assert_eq!(
            compute_penalties(&trades, &measurements, PENALTY_RATE),
            vec![buyer_penalty("first", 3000, PenaltyReason::Deviation)]
        );
    }

    #[test]
    fn incomplete_or_missing_point_penalizes_every_member_side_on_full_energy() {
        let trades = d1_trades("", TIME_SLOT);
        let not_complete = [MeasurementCompleteness::Incomplete, MeasurementCompleteness::Missing];
        for completeness in not_complete {
            let measurements =
                vec![point_row("House", "Site", HOUSE, completeness.clone(), 0.0, TIME_SLOT)];

            let penalties = compute_penalties(&trades, &measurements, PENALTY_RATE);

            assert_eq!(
                penalties,
                vec![
                    buyer_penalty("bid-trade", 3000, missing("House")),
                    seller_penalty("offer-trade", 2000, missing("House")),
                ],
                "{:?}",
                completeness
            );
            assert_eq!(
                evaluated_trade_uuids(&trades, &measurements),
                vec!["bid-trade".to_string(), "offer-trade".to_string()]
            );
        }
    }

    #[test]
    fn cross_point_trade_is_judged_at_each_side_point() {
        // The bid is at HouseA (complete), the offer at HouseB (missing).
        let trades = vec![side_trade("cross", "houseA_load", "houseB_pv", 3.0, TIME_SLOT, 0)];
        let house_b = point_row(
            "HouseB",
            "Site",
            &["houseB_pv"],
            MeasurementCompleteness::Missing,
            0.0,
            TIME_SLOT,
        );

        // HouseA consumed exactly what it bought: only the seller is penalized.
        let measurements = vec![
            complete_point("HouseA", &["houseA_load"], 3.0, TIME_SLOT),
            house_b.clone(),
        ];
        assert_eq!(
            compute_penalties(&trades, &measurements, PENALTY_RATE),
            vec![seller_penalty("cross", 3000, missing("HouseB"))]
        );

        // HouseA consumed 1 kWh more: the trade gets a buyer and a seller penalty.
        let measurements = vec![
            complete_point("HouseA", &["houseA_load"], 4.0, TIME_SLOT),
            house_b,
        ];
        assert_eq!(
            compute_penalties(&trades, &measurements, PENALTY_RATE),
            vec![
                buyer_penalty("cross", 1000, PenaltyReason::Deviation),
                seller_penalty("cross", 3000, missing("HouseB")),
            ]
        );
    }

    #[test]
    fn inter_community_side_uses_the_aggregate_only_when_every_point_is_complete() {
        let comm_a = h256_to_string(community_id_from_uuid("CommA"));
        let comm_b = h256_to_string(community_id_from_uuid("CommB"));
        let trades = vec![side_trade("ic", &comm_a, &comm_b, 2.0, TIME_SLOT, 0)];
        let house_a2 = |completeness: MeasurementCompleteness, energy_kwh: f64| {
            point_row("HouseA2", "CommA", &["a2_load"], completeness, energy_kwh, TIME_SLOT)
        };
        let house_a1 = point_row(
            "HouseA1",
            "CommA",
            &["a1_load"],
            MeasurementCompleteness::Complete,
            2.0,
            TIME_SLOT,
        );

        // All complete: CommA's net import is 2 + 1 = 3 against 2 bought -> excess 1.
        let measurements = vec![house_a1.clone(), house_a2(MeasurementCompleteness::Complete, 1.0)];
        assert_eq!(
            compute_penalties(&trades, &measurements, PENALTY_RATE),
            vec![buyer_penalty("ic", 1000, PenaltyReason::Deviation)]
        );
        assert_eq!(evaluated_trade_uuids(&trades, &measurements), vec!["ic".to_string()]);

        // One point incomplete or missing: the buyer side is penalized on its full energy.
        let not_complete = [MeasurementCompleteness::Incomplete, MeasurementCompleteness::Missing];
        for completeness in not_complete {
            let measurements = vec![house_a1.clone(), house_a2(completeness, 0.0)];
            // A building with a data gap lists its meters, so it is not an unmetered point.
            assert!(!measurements[1].metering_point.as_ref().unwrap().missing_meters.is_empty());
            assert_eq!(
                build_measurement_map(&measurements)[&(comm_a.clone(), TIME_SLOT)],
                MeasuredEnergy::Unreliable {
                    community_uuid: "CommA".to_string()
                }
            );
            assert_eq!(
                compute_penalties(&trades, &measurements, PENALTY_RATE),
                vec![buyer_penalty("ic", 2000, missing("CommA"))]
            );
            assert_eq!(evaluated_trade_uuids(&trades, &measurements), vec!["ic".to_string()]);
        }

        // No rows for either community: unjudged.
        let measurements = vec![point_row(
            "HouseC",
            "CommC",
            &["c_load"],
            MeasurementCompleteness::Missing,
            0.0,
            TIME_SLOT,
        )];
        assert!(compute_penalties(&trades, &measurements, PENALTY_RATE).is_empty());
        assert!(evaluated_trade_uuids(&trades, &measurements).is_empty());
    }

    /// The row of a metering point that expects no meter (the community client's site-level
    /// point): always `Missing`, with no `missing_meters`.
    fn unmetered_point(
        name: &str,
        community_uuid: &str,
        members: &[&str],
        time_slot: u64,
    ) -> MeasurementSchema {
        let mut row = point_row(
            name,
            community_uuid,
            members,
            MeasurementCompleteness::Missing,
            0.0,
            time_slot,
        );
        row.metering_point.as_mut().unwrap().missing_meters.clear();
        row
    }

    #[test]
    fn unmetered_point_does_not_make_the_community_aggregate_unreliable() {
        let comm_a = h256_to_string(community_id_from_uuid("CommA"));
        let comm_b = h256_to_string(community_id_from_uuid("CommB"));
        let trades = vec![side_trade("ic", &comm_a, &comm_b, 2.0, TIME_SLOT, 0)];
        let measurements = vec![
            complete_point_in("HouseA1", "CommA", &["a1_load"], 2.0),
            complete_point_in("HouseA2", "CommA", &["a2_load"], 1.0),
            unmetered_point("CommASite", "CommA", &["a_site_batt"], TIME_SLOT),
        ];

        // The aggregate is the sum of the building rows only.
        let map = build_measurement_map(&measurements);
        assert_eq!(map[&(comm_a.clone(), TIME_SLOT)], MeasuredEnergy::Energy(3.0));

        // CommA's net import is 3 against 2 bought -> excess 1, not the full 2 kWh.
        assert_eq!(
            compute_penalties(&trades, &measurements, PENALTY_RATE),
            vec![buyer_penalty("ic", 1000, PenaltyReason::Deviation)]
        );
        assert_eq!(evaluated_trade_uuids(&trades, &measurements), vec!["ic".to_string()]);

        // A community with only an unmetered point has no aggregate at all.
        let only_unmetered =
            vec![unmetered_point("CommASite", "CommA", &["a_site_batt"], TIME_SLOT)];
        assert!(!build_measurement_map(&only_unmetered).contains_key(&(comm_a, TIME_SLOT)));
    }

    #[test]
    fn member_of_an_unmetered_point_is_penalized_on_full_energy() {
        let trades = vec![side_trade("site", "a1_load", "a_site_batt", 2.0, TIME_SLOT, 0)];
        let measurements = vec![
            complete_point_in("HouseA1", "CommA", &["a1_load"], 2.0),
            unmetered_point("CommASite", "CommA", &["a_site_batt"], TIME_SLOT),
        ];

        assert_eq!(
            compute_penalties(&trades, &measurements, PENALTY_RATE),
            vec![seller_penalty("site", 2000, missing("CommASite"))]
        );
        assert_eq!(evaluated_trade_uuids(&trades, &measurements), vec!["site".to_string()]);
    }

    /// A complete metering point's row in `community_uuid`, in `TIME_SLOT`.
    fn complete_point_in(
        name: &str,
        community_uuid: &str,
        members: &[&str],
        energy_kwh: f64,
    ) -> MeasurementSchema {
        point_row(
            name,
            community_uuid,
            members,
            MeasurementCompleteness::Complete,
            energy_kwh,
            TIME_SLOT,
        )
    }

    #[test]
    fn build_measurement_map_keeps_point_rows_out_of_the_per_area_entries() {
        use MeasurementCompleteness::{Complete, Incomplete};
        let measurements = vec![
            measurement("assetA1", "CommA", 5.0),
            point_row("HouseA", "CommA", &["a_load"], Complete, 2.0, TIME_SLOT),
            point_row("HouseB", "CommB", &["b_load"], Incomplete, 0.0, TIME_SLOT),
        ];

        let map = build_measurement_map(&measurements);

        let key = |hash: &str| (hash.to_string(), TIME_SLOT);
        assert_eq!(map[&key("assetA1")], MeasuredEnergy::Energy(5.0));
        assert!(!map.contains_key(&key("HouseA_hash")));
        assert!(!map.contains_key(&key("HouseB_hash")));
        assert_eq!(
            map[&key(&h256_to_string(community_id_from_uuid("CommA")))],
            MeasuredEnergy::Energy(7.0)
        );
        assert_eq!(
            map[&key(&h256_to_string(community_id_from_uuid("CommB")))],
            MeasuredEnergy::Unreliable {
                community_uuid: "CommB".to_string()
            }
        );
        assert_eq!(map.len(), 3);
    }

    #[test]
    fn metering_point_index_keeps_the_first_row_claiming_a_member() {
        let measurements = vec![
            complete_point("First", &["shared", "first_only"], 1.0, TIME_SLOT),
            complete_point("Second", &["shared"], 2.0, TIME_SLOT),
            complete_point("First", &["shared"], 3.0, NEXT_SLOT),
            measurement("plain_area", "Comm", 1.0),
        ];

        let index = metering_point_index(&measurements);

        let name = |hash: &str, slot: u64| {
            index[&(hash.to_string(), slot)]
                .metering_point
                .as_ref()
                .unwrap()
                .name
                .clone()
        };
        assert_eq!(name("shared", TIME_SLOT), "First");
        assert_eq!(name("first_only", TIME_SLOT), "First");
        assert_eq!(index[&("shared".to_string(), NEXT_SLOT)].energy_kwh, 3.0);
        assert_eq!(index.len(), 3);
    }

    #[test]
    fn evaluated_trade_uuids_covers_points_of_any_completeness() {
        let trades = vec![
            side_trade("at-complete", "c_load", "outside_pv", 1.0, TIME_SLOT, 0),
            side_trade("at-incomplete", "outside_load", "i_pv", 1.0, TIME_SLOT, 0),
            side_trade("at-missing", "m_load", "outside_pv", 1.0, TIME_SLOT, 0),
            side_trade("no-row", "other_load", "other_pv", 1.0, TIME_SLOT, 0),
            // A member area, but in a slot the point has no row for.
            side_trade("other-slot", "c_load", "outside_pv", 1.0, NEXT_SLOT, 0),
        ];
        let measurements = vec![
            complete_point("C", &["c_load"], 1.0, TIME_SLOT),
            point_row("I", "Site", &["i_pv"], MeasurementCompleteness::Incomplete, 0.0, TIME_SLOT),
            point_row("M", "Site", &["m_load"], MeasurementCompleteness::Missing, 0.0, TIME_SLOT),
        ];

        assert_eq!(
            evaluated_trade_uuids(&trades, &measurements),
            vec![
                "at-complete".to_string(),
                "at-incomplete".to_string(),
                "at-missing".to_string(),
            ]
        );
    }
}
