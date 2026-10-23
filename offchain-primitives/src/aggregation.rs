use crate::db_api_schema::profiles::{ForecastSchema, MeasurementSchema};

pub const RESIDUAL_ENERGY_TOLERANCE_KWH: f64 = 1e-9;

pub trait CommunityEnergyPoint {
    fn community_uuid(&self) -> &str;
    fn energy_kwh(&self) -> f64;
    fn time_slot(&self) -> u64;
}

impl CommunityEnergyPoint for ForecastSchema {
    fn community_uuid(&self) -> &str {
        &self.community_uuid
    }

    fn energy_kwh(&self) -> f64 {
        self.energy_kwh
    }

    fn time_slot(&self) -> u64 {
        self.time_slot
    }
}

impl CommunityEnergyPoint for MeasurementSchema {
    fn community_uuid(&self) -> &str {
        &self.community_uuid
    }

    fn energy_kwh(&self) -> f64 {
        self.energy_kwh
    }

    fn time_slot(&self) -> u64 {
        self.time_slot
    }
}

pub fn aggregate_net_import<T: CommunityEnergyPoint>(
    points: &[T],
    community_uuid: &str,
    time_slot: u64,
) -> f64 {
    points
        .iter()
        .filter(|point| point.community_uuid() == community_uuid && point.time_slot() == time_slot)
        .map(|point| point.energy_kwh())
        .sum()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrderType {
    Bid,
    Offer,
    None,
}

pub fn net_to_order_type(net_import_kwh: f64, tol: f64) -> OrderType {
    if net_import_kwh.abs() <= tol {
        OrderType::None
    } else if net_import_kwh > tol {
        OrderType::Bid
    } else {
        OrderType::Offer
    }
}

