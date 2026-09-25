//! Turns offchain-storage documents into the KPI input records: attributes trades to
//! communities and keys meter readings by the same on-chain id trades use for the buyer.

use crate::model::{CommunityRef, MeterReading, TradeRecord};
use crate::period::Window;
use primitives::db_api_schema::profiles::{
    MeasurementPointSchema, MeasurementPointType, TimeseriesSchema,
};
use primitives::db_api_schema::trades::DbTradeSchema;
use primitives::utils::{bytes16_to_hex, create_encrypted_bytes16_from_string, generate_market_id};
use primitives::MarketType;
use std::collections::HashMap;

const MARKET_TYPES: [MarketType; 3] = [MarketType::Spot, MarketType::Flex, MarketType::Settlement];

/// On-chain (bytes16 hex) representation of an offchain id, as used for trade buyers/sellers.
pub fn onchain_id(offchain_id: &str) -> String {
    bytes16_to_hex(create_encrypted_bytes16_from_string(offchain_id))
}

/// Market id → community id for every market the orchestrator opens in the window.
/// Market ids are derived from `(community_id, market_type, delivery_start)`.
#[derive(Debug, Default)]
pub struct MarketIndex(HashMap<String, String>);

impl MarketIndex {
    pub fn build(communities: &[CommunityRef], window: &Window) -> Self {
        let mut index = HashMap::new();
        for community in communities {
            for slot in window.slots() {
                for market_type in MARKET_TYPES {
                    let market_id =
                        generate_market_id(&community.community_id, market_type, slot as u64);
                    index.insert(bytes16_to_hex(market_id), community.community_id.clone());
                }
            }
        }
        MarketIndex(index)
    }

    pub fn community_for(&self, market_id: &str) -> Option<&str> {
        self.0
            .get(&market_id.to_ascii_lowercase())
            .map(String::as_str)
    }
}

pub fn trade_record(trade: DbTradeSchema, markets: &MarketIndex) -> TradeRecord {
    TradeRecord {
        community_id: markets.community_for(&trade.market_id).map(str::to_string),
        trade_uuid: trade.trade_uuid,
        buyer: trade.buyer,
        time_slot: trade.time_slot as i64,
        energy_kwh: trade.parameters.selected_energy_kWh,
        energy_rate: trade.parameters.energy_rate,
        status: trade.status,
    }
}

/// Counts of timeseries data that could not be used, for the tick log.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ReadingStats {
    /// Measurement points that are not per-slot kWh values or have no community.
    pub skipped_points: usize,
    /// Values whose facility (`asset_name`) has no owner in the `facilities` collection.
    pub values_without_owner: usize,
    pub invalid_timestamps: usize,
}

/// Whether a point holds per-slot net energy in kWh for a known community. Values are signed:
/// positive = import, negative = export, as written by offchain-storage's measurement routes.
fn usable_point(point: &MeasurementPointSchema) -> bool {
    point.point_type == MeasurementPointType::Measurement
        && point.unit.eq_ignore_ascii_case("kWh")
        && !point.energy_accumulated
        && point.time_resolution == "PT15M"
        && point
            .datasource_name
            .as_deref()
            .is_some_and(|name| !name.trim().is_empty())
}

/// Converts timeseries values into meter readings, mirroring the execution engine:
/// the community is the point's `datasource_name`, and the facility (`asset_name`) is mapped
/// to its owner, whose on-chain id is what trades carry as `buyer`.
/// Values of points not in `points` (e.g. forecasts) are ignored.
pub fn meter_readings(
    points: &[MeasurementPointSchema],
    values: Vec<TimeseriesSchema>,
    facility_owners: &HashMap<String, String>,
) -> (Vec<MeterReading>, ReadingStats) {
    let mut stats = ReadingStats::default();
    let mut usable: HashMap<&str, &MeasurementPointSchema> = HashMap::new();
    for point in points {
        if usable_point(point) {
            usable.insert(point.measurement_id.as_str(), point);
        } else {
            stats.skipped_points += 1;
        }
    }

    let mut readings = Vec::new();
    for value in values {
        let Some(point) = usable.get(value.measurement_point.as_str()) else {
            continue;
        };
        let Ok(time_slot) = value.timestamp.parse::<i64>() else {
            stats.invalid_timestamps += 1;
            continue;
        };
        let Some(owner_id) = facility_owners.get(&point.asset_name) else {
            stats.values_without_owner += 1;
            continue;
        };
        readings.push(MeterReading {
            community_id: point.datasource_name.clone().unwrap_or_default(),
            facility: onchain_id(owner_id),
            time_slot,
            energy_kwh: value.value,
        });
    }
    (readings, stats)
}

/// Ids of the points whose values [`meter_readings`] would use.
pub fn usable_point_ids(points: &[MeasurementPointSchema]) -> Vec<String> {
    points
        .iter()
        .filter(|point| usable_point(point))
        .map(|point| point.measurement_id.clone())
        .collect()
}
