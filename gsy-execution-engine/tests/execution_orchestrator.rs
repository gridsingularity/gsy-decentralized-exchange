#[cfg(test)]
mod tests {

    use gsy_execution_engine::primitives::penalty_calculator::compute_penalties;
    use gsy_offchain_primitives::db_api_schema::orders::{DbBid, DbOffer, DbOrderComponent};
    use gsy_offchain_primitives::db_api_schema::profiles::MeasurementSchema;
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
}
