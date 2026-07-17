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
        TradeSchema {
            _id: trade_uuid.to_string(),
            status: TradeStatus::Settled,
            seller: seller.to_string(),
            buyer: buyer.to_string(),
            market_id: market_id.to_string(),
            time_slot: TIME_SLOT,
            trade_uuid: trade_uuid.to_string(),
            creation_time: 0,
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
}
