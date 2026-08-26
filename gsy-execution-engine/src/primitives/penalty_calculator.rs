use primitives::db_api_schema::{profiles::MeasurementSchema, trades::DbTradeSchema};
use primitives::utils::{bytes16_to_hex, parse_or_hash_bytes16};
use std::collections::HashMap;

#[derive(Debug)]
pub struct Penalty {
    pub penalized_account: String,
    pub market_id: String,
    pub trade_uuid: String,
    pub penalty_cost: u64,
}

/// Computes penalties for each trade based on the measured energy.
///
/// For each trade, the measurement is looked up (by using the area_uuid and market_id from the Bid).
/// The delta is computed as:
///   delta = measured_energy - traded_energy
/// If delta > 0.0, it indicates under-trading for consumption and the buyer is penalized.
/// If delta < 0.0, it indicates under-trading for production and the seller is penalized.
///
/// # Arguments
///
/// * `trades` - A slice of DbTradeSchema records.
/// * `measurements` - A slice of MeasurementSchema records.
/// * `penalty_rate` - The penalty rate as a f64 (e.g., 0.10 for 10%).
///
/// # Returns
///
/// A vector of Penalty structs.
pub fn compute_penalties(
    trades: &[DbTradeSchema],
    measurements: &[MeasurementSchema],
    penalty_rate: f64,
) -> Vec<Penalty> {
    let mut penalties = Vec::new();

    // Measurements are keyed by facility ids in the Intelligent profile layer, while EVM trades
    // reference actor/facility ids as bytes16 hex. Store both forms so the execution runtime can
    // bridge the two representations.
    let mut measurement_map: HashMap<String, f64> = HashMap::new();
    for meas in measurements {
        measurement_map.insert(meas.facility_id.clone(), meas.energy_kwh);
        measurement_map.insert(
            bytes16_to_hex(parse_or_hash_bytes16(meas.facility_id.as_str())),
            meas.energy_kwh,
        );
    }

    // Iterate over each trade and compute the penalty if a measurement exists.
    for trade in trades {
        if let Some(&measured_energy) = measurement_map
            .get(&trade.buyer)
            .or_else(|| measurement_map.get(&trade.seller))
        {
            let traded_energy = trade.parameters.selected_energy_kWh;

            // Compute delta = measured_energy - traded_energy.
            let delta = measured_energy - traded_energy;

            if delta > 0.0 {
                // This is a consumption trade: measured energy exceeds traded energy.
                // Penalize the buyer.

                let raw_penalty = delta * penalty_rate;

                // Scale and convert to u64: apply a scaling factor of 10,000.
                let penalty_cost = (raw_penalty * 10_000.0).round() as u64;

                penalties.push(Penalty {
                    penalized_account: trade.buyer.clone(),
                    market_id: trade.market_id.clone(),
                    trade_uuid: trade.trade_uuid.clone(),
                    penalty_cost,
                });
            } else if delta < 0.0 {
                // This is a production trade: measured energy is less than traded energy.
                // Penalize the seller.
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

#[cfg(test)]
mod tests {
    use super::*;
    use primitives::db_api_schema::{
        orders::{DbOrderSchema, OrderEnum, OrderStatus},
        profiles::MeasurementSchema,
        trades::{DbTradeSchema, TradeParameters, TradeStatus},
    };
    use primitives::utils::{bytes16_to_hex, parse_or_hash_bytes16};

    fn order(order_id: &str, facility_id: &str, is_bid: bool) -> DbOrderSchema {
        let actor_id = bytes16_to_hex(parse_or_hash_bytes16(facility_id));
        DbOrderSchema {
            order_id: order_id.to_string(),
            status: OrderStatus::Executed,
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
        let bid = order("bid-1", "areaalice", true);
        let offer = order("offer-1", "areabob", false);
        DbTradeSchema {
            trade_uuid: "trade-1".to_string(),
            status: TradeStatus::Settled,
            seller: offer.created_by.clone(),
            buyer: bid.created_by.clone(),
            market_id: "market-1".to_string(),
            time_slot: 1_000,
            creation_time: 950,
            offer_hash: "offer-1".to_string(),
            bid_hash: "bid-1".to_string(),
            residual_offer_id: None,
            residual_bid_id: None,
            parameters: TradeParameters {
                selected_energy_kWh: 10.0,
                energy_rate: 1.0,
            },
        }
    }

    #[test]
    fn compute_penalties_matches_facility_measurements_to_evm_actor_ids() {
        let measurements = vec![MeasurementSchema {
            facility_id: "areaalice".to_string(),
            community_uuid: "community1".to_string(),
            time_slot: 1_000,
            creation_time: 1_000,
            energy_kwh: 12.0,
        }];

        let penalties = compute_penalties(&[trade()], &measurements, 0.10);

        assert_eq!(penalties.len(), 1);
        assert_eq!(penalties[0].penalty_cost, 2_000);
        assert_eq!(
            penalties[0].penalized_account,
            bytes16_to_hex(parse_or_hash_bytes16("areaalice"))
        );
    }
}
