#[cfg(test)]
mod tests {
    use gsy_execution_engine::primitives::penalty_calculator::{compute_penalties, Penalty};
    use gsy_offchain_primitives::db_api_schema::orders::{DbBid, DbOffer, DbOrderComponent};
    use gsy_offchain_primitives::db_api_schema::profiles::MeasurementSchema;
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
                    });
                } else if delta < 0.0 {
                    let raw_penalty = (-delta) * penalty_rate;
                    let penalty_cost = (raw_penalty * 10_000.0).round() as u64;
                    penalties.push(Penalty {
                        penalized_account: trade.seller.clone(),
                        market_id: trade.market_id.clone(),
                        trade_uuid: trade.trade_uuid.clone(),
                        penalty_cost,
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
}
