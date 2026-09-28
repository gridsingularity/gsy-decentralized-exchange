#[cfg(test)]
mod tests {

    use gsy_execution_engine::primitives::penalty_calculator::{
        compute_penalties, dedupe_trades, evaluated_trade_uuids, PenaltyReason,
    };
    use gsy_execution_engine::services::execution_orchestrator::retain_settled;
    use gsy_offchain_primitives::db_api_schema::orders::{DbBid, DbOffer, DbOrderComponent};
    use gsy_offchain_primitives::db_api_schema::profiles::{
        MeasurementCompleteness, MeasurementSchema, MeteringPointMeasurement,
    };
    use gsy_offchain_primitives::db_api_schema::trades::{
        TradeParameters, TradeSchema, TradeStatus,
    };
    use gsy_offchain_primitives::utils::{community_id_from_uuid, h256_to_string};

    const TIME_SLOT: u64 = 1_700_000_000;
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

    fn component(area_uuid: &str, market_id: &str, energy: f64) -> DbOrderComponent {
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
        seller_area: &str,
        market_id: &str,
        selected_energy: f64,
        trade_uuid: &str,
    ) -> TradeSchema {
        TradeSchema {
            _id: trade_uuid.to_string(),
            status: TradeStatus::Settled,
            seller: seller.to_string(),
            buyer: buyer.to_string(),
            market_id: market_id.to_string(),
            time_slot: TIME_SLOT,
            trade_uuid: trade_uuid.to_string(),
            creation_time: 0,
            status_updated_at: None,
            offer: DbOffer {
                seller: seller.to_string(),
                nonce: 0,
                offer_component: component(seller_area, market_id, selected_energy),
            },
            offer_hash: String::new(),
            bid: DbBid {
                buyer: buyer.to_string(),
                nonce: 0,
                bid_component: component(bid_area, market_id, selected_energy),
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

    #[test]
    fn execution_cycle_settles_inter_community_and_spot_trades_together() {
        // --- Simulated output of fetch_trades_and_measurements_for_timeslot ---
        //
        // Community "CommA" (the buyer/deficit side of the inter-community trade):
        //   per-asset measurements net = 6.0 - 4.0 + 1.0 = 3.0 kWh (Σ signed energy).
        // Community "CommB" (the seller/surplus side):
        //   per-asset measurements net = 2.0 - 9.0 = -7.0 kWh.
        // "SpotComm" carries an ordinary per-asset spot measurement for a spot trade.
        let measurements = vec![
            // CommA (aggregated → deficit, net +3.0)
            measurement("commA_load", "CommA", 6.0),
            measurement("commA_pv", "CommA", -4.0),
            measurement("commA_extra", "CommA", 1.0),
            // CommB (aggregated → surplus, net -7.0)
            measurement("commB_load", "CommB", 2.0),
            measurement("commB_pv", "CommB", -9.0),
            // A plain spot asset (per-asset settlement, must be untouched by aggregation)
            measurement("spot_asset_hash", "SpotComm", 5.0),
        ];

        let comm_a_area = h256_to_string(community_id_from_uuid("CommA"));
        let comm_b_area = h256_to_string(community_id_from_uuid("CommB"));
        let inter_market_id = "inter_community_market";
        let spot_market_id = "spot_market";

        let trades = vec![
            // Inter-community trade: bid.area_uuid = community hash of CommA (buyer),
            // offer.area_uuid = community hash of CommB (seller). Settlement keys on the
            // bid area, i.e. CommA's aggregated net (+3.0).
            trade(
                "commA_account",
                "commB_account",
                &comm_a_area,
                &comm_b_area,
                inter_market_id,
                2.0,
                "trade-inter-1",
            ),
            // Spot trade: bid.area_uuid is a per-asset area_hash.
            trade(
                "spot_buyer",
                "spot_seller",
                "spot_asset_hash",
                "spot_seller_hash",
                spot_market_id,
                4.0,
                "trade-spot-1",
            ),
        ];

        let penalties = compute_penalties(&trades, &measurements, PENALTY_RATE);

        // Two settlements: one against CommA's aggregate, one against the spot asset.
        assert_eq!(penalties.len(), 2, "expected one penalty per trade");

        // Inter-community: delta = CommA net (3.0) - traded (2.0) = 1.0 > 0 → buyer penalized.
        // penalty_cost = (1.0 * 0.10 * 10_000).round() = 1000.
        let inter = penalties
            .iter()
            .find(|p| p.trade_uuid == "trade-inter-1")
            .expect("inter-community trade must be settled against the community aggregate");
        assert_eq!(inter.penalized_account, "commA_account");
        assert_eq!(inter.market_id, inter_market_id);
        assert_eq!(inter.penalty_cost, 1000);

        // Spot: delta = measured (5.0) - traded (4.0) = 1.0 > 0 → buyer penalized.
        // penalty_cost = (1.0 * 0.10 * 10_000).round() = 1000.
        let spot = penalties
            .iter()
            .find(|p| p.trade_uuid == "trade-spot-1")
            .expect("spot trade must be settled against its per-asset measurement");
        assert_eq!(spot.penalized_account, "spot_buyer");
        assert_eq!(spot.market_id, spot_market_id);
        assert_eq!(spot.penalty_cost, 1000);
    }

    #[test]
    fn retain_settled_submits_first_verdicts_while_judged_trades_keep_their_budget() {
        // Two 2 kWh sales from one building's PV; the earlier one was already judged
        // (`Executed`) in a previous cycle. The building exported 3 kWh, so the offers keep
        // 3 kWh in time priority: the earlier trade is covered, the later one is short 1 kWh.
        let mut earlier =
            trade("out_buyer", "pv_seller", "outside_load", "house_pv", "spot", 2.0, "t-earlier");
        earlier.creation_time = 1;
        earlier.status = TradeStatus::Executed;
        let mut later =
            trade("out_buyer", "pv_seller", "outside_load", "house_pv", "spot", 2.0, "t-later");
        later.creation_time = 2;
        // A settled trade with no measurement at all stays unjudged.
        let unmeasured =
            trade("x_buyer", "x_seller", "x_load", "x_pv", "spot", 1.0, "t-unmeasured");
        let trades = vec![earlier, later, unmeasured];
        let measurements = vec![MeasurementSchema {
            area_uuid: "house_uuid".to_string(),
            area_hash: "house_hash".to_string(),
            community_uuid: "Site".to_string(),
            time_slot: TIME_SLOT,
            creation_time: 0,
            energy_kwh: -3.0,
            metering_point: Some(MeteringPointMeasurement {
                name: "House".to_string(),
                member_area_hashes: vec!["house_pv".to_string()],
                completeness: MeasurementCompleteness::Complete,
                missing_meters: Vec::new(),
            }),
        }];

        let penalties = compute_penalties(&trades, &measurements, PENALTY_RATE);
        let evaluated = evaluated_trade_uuids(&trades, &measurements);
        let (penalties, evaluated) = retain_settled(&trades, penalties, evaluated);

        assert_eq!(penalties.len(), 1);
        assert_eq!(penalties[0].trade_uuid, "t-later");
        assert_eq!(penalties[0].penalized_account, "pv_seller");
        assert_eq!(penalties[0].penalty_cost, 1000);
        assert_eq!(penalties[0].reason, PenaltyReason::Deviation);
        assert_eq!(evaluated, vec!["t-later".to_string()]);

        // Judging the later trade alone would have found no shortfall (sold 2, exported 3).
        assert!(compute_penalties(&trades[1..2], &measurements, PENALTY_RATE).is_empty());
    }

    #[test]
    fn retain_settled_drops_verdicts_of_already_judged_trades() {
        let mut executed = trade("b", "s", "b_area", "s_area", "spot", 1.0, "t-executed");
        executed.status = TradeStatus::Executed;
        let mut penalized = trade("b", "s", "b_area", "s_area", "spot", 1.0, "t-penalized");
        penalized.status = TradeStatus::Penalized;
        let settled = trade("b", "s", "b_area", "s_area", "spot", 1.0, "t-settled");
        let trades = vec![executed, penalized, settled];
        let measurements = vec![measurement("b_area", "Comm", 5.0)];

        let penalties = compute_penalties(&trades, &measurements, PENALTY_RATE);
        assert_eq!(penalties.len(), 3);
        let evaluated = evaluated_trade_uuids(&trades, &measurements);
        let (penalties, evaluated) = retain_settled(&trades, penalties, evaluated);

        let uuids: Vec<&str> = penalties.iter().map(|p| p.trade_uuid.as_str()).collect();
        assert_eq!(uuids, vec!["t-settled"]);
        assert_eq!(evaluated, vec!["t-settled".to_string()]);
    }

    /// A copy of `original` as a re-post stored it: same trade, another storage `_id`.
    fn stored_again(original: &TradeSchema, status: TradeStatus) -> TradeSchema {
        TradeSchema {
            _id: format!("{}-copy", original._id),
            status,
            ..original.clone()
        }
    }

    #[test]
    fn duplicated_trade_is_counted_once_and_judged_once() {
        // A 2 kWh sale from a building that exported 3 kWh, stored twice. Counted twice it
        // would be 4 kWh sold against 3 exported: a false 1 kWh under-delivery on one copy.
        let sale = trade("out_buyer", "pv_seller", "outside_load", "house_pv", "spot", 2.0, "t-sale");
        // A 3 kWh purchase by a load that consumed 5 kWh, stored twice. Counted twice it would
        // cover the whole consumption and hide the 2 kWh over-consumption.
        let purchase = trade("load_buyer", "x_seller", "load", "x_pv", "spot", 3.0, "t-purchase");
        let trades = vec![
            sale.clone(),
            purchase.clone(),
            stored_again(&sale, TradeStatus::Settled),
            stored_again(&purchase, TradeStatus::Settled),
        ];
        let measurements = vec![
            MeasurementSchema {
                area_uuid: "house_uuid".to_string(),
                area_hash: "house_hash".to_string(),
                community_uuid: "Site".to_string(),
                time_slot: TIME_SLOT,
                creation_time: 0,
                energy_kwh: -3.0,
                metering_point: Some(MeteringPointMeasurement {
                    name: "House".to_string(),
                    member_area_hashes: vec!["house_pv".to_string()],
                    completeness: MeasurementCompleteness::Complete,
                    missing_meters: Vec::new(),
                }),
            },
            measurement("load", "Comm", 5.0),
        ];

        let deduped = dedupe_trades(&trades);
        assert_eq!(deduped, vec![sale, purchase]);

        let penalties = compute_penalties(&trades, &measurements, PENALTY_RATE);
        let evaluated = evaluated_trade_uuids(&trades, &measurements);
        let (penalties, evaluated) = retain_settled(&trades, penalties, evaluated);

        assert_eq!(penalties.len(), 1, "one penalty, for the over-consumption: {:?}", penalties);
        assert_eq!(penalties[0].trade_uuid, "t-purchase");
        assert_eq!(penalties[0].penalized_account, "load_buyer");
        assert_eq!(penalties[0].penalty_cost, 2000);
        assert_eq!(evaluated, vec!["t-sale".to_string(), "t-purchase".to_string()]);
    }

    #[test]
    fn trade_with_one_judged_copy_is_not_judged_again() {
        // The Settled copy comes first, as a re-post stored before the verdict can.
        for verdict in [TradeStatus::Executed, TradeStatus::Penalized] {
            let unjudged = trade("b", "s", "b_area", "s_area", "spot", 1.0, "t-judged");
            let judged = stored_again(&unjudged, verdict.clone());
            let trades = vec![unjudged, judged.clone()];
            let measurements = vec![measurement("b_area", "Comm", 5.0)];

            assert_eq!(dedupe_trades(&trades), vec![judged]);

            let penalties = compute_penalties(&trades, &measurements, PENALTY_RATE);
            assert_eq!(penalties.len(), 1, "the trade is still counted once");
            let evaluated = evaluated_trade_uuids(&trades, &measurements);
            let (penalties, evaluated) = retain_settled(&trades, penalties, evaluated);

            assert!(penalties.is_empty(), "{:?} copy: no second verdict", verdict);
            assert!(evaluated.is_empty(), "{:?} copy: no second verdict", verdict);
        }
    }
}
