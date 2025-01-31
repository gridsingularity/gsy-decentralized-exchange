use gsy_offchain_primitives::db_api_schema::{
    profiles::MeasurementSchema,
    trades::TradeSchema,
};
use std::collections::HashMap;

#[derive(Debug)]
pub struct Penalty {
    pub area_uuid: String,
    pub market_uuid: String,
    pub penalty_energy: f64,
}

pub fn compute_penalties(
    trades: &[TradeSchema], 
    measurements: &[MeasurementSchema],
    penalty_rate: f64
) -> Vec<Penalty> {
    let mut penalties = Vec::new();

    // Aggregate consumed and produced energy per (area_uuid, market_uuid)
    // Key: (area_uuid, market_uuid)
    // Value: (total_consumed_energy, total_produced_energy)
    let mut energy_map: HashMap<(String, String), (f64, f64)> = HashMap::new();

    for trade in trades {
        // Extract consumer details from Bid
        let consumer_area_uuid = trade.bid.bid_component.area_uuid.clone();
        let consumer_market_uuid = trade.bid.bid_component.market_uuid.clone();
        let consumed_energy = trade.parameters.selected_energy;

        // Extract producer details from Offer
        let producer_area_uuid = trade.offer.offer_component.area_uuid.clone();
        let producer_market_uuid = trade.offer.offer_component.market_uuid.clone();
        let produced_energy = trade.parameters.selected_energy;

        // Update consumed energy
        let consumer_key = (consumer_area_uuid.clone(), consumer_market_uuid.clone());
        let consumer_entry = energy_map.entry(consumer_key).or_insert((0.0, 0.0));
        consumer_entry.0 += consumed_energy;

        // Update produced energy
        let producer_key = (producer_area_uuid.clone(), producer_market_uuid.clone());
        let producer_entry = energy_map.entry(producer_key).or_insert((0.0, 0.0));
        producer_entry.1 += produced_energy;
    }

    // Iterate over each measurement
    for measurement in measurements {
        let key = (measurement.area_uuid.clone(), measurement.community_uuid.clone());
        let (consumed_energy, produced_energy) = energy_map.get(&key)
            .unwrap_or(&(0.0, 0.0));

        let measured_energy = measurement.energy_kwh;

        if measured_energy > 0.0 {
            // Consumption measurement
            // Calculate delta_consumed = measured_energy - consumed_energy
            let delta_consumed = measured_energy - consumed_energy;
            let delta_consumed = if delta_consumed > 0.0 { delta_consumed } else { 0.0 };

            if delta_consumed > 0.0 {
                // Consumers consumed more than traded
                let penalty = delta_consumed * penalty_rate;
                penalties.push(Penalty {
                    area_uuid: measurement.area_uuid.clone(),
                    market_uuid: measurement.community_uuid.clone(),
                    penalty_energy: penalty,
                });
            }
        } else if measured_energy < 0.0 {
            // Production measurement
            // Calculate delta_produced = produced_energy + measured_energy (since measured_energy is negative)
            let delta_produced = produced_energy + measured_energy;
            let delta_produced = if delta_produced > 0.0 { delta_produced } else { 0.0 };

            if delta_produced > 0.0 {
                // Producers produced more than traded
                let penalty = delta_produced * penalty_rate;
                penalties.push(Penalty {
                    area_uuid: measurement.area_uuid.clone(),
                    market_uuid: measurement.community_uuid.clone(),
                    penalty_energy: penalty,
                });
            }
        }
    }

    penalties
}
