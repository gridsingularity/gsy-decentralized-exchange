//! Measurements Storage schemas, as specified in D3.2 section 5.2.
//!
//! `MeasurementPointSchema` and `TimeseriesSchema` are the canonical
//! ontology-aligned storage records. `MeasurementSchema` and `ForecastSchema`
//! remain compatibility DTOs for EVM callers and are converted at the API edge;
//! they are never stored, and their times are RFC 3339 strings in JSON.

#![allow(non_snake_case)]

use bson::serde_helpers::chrono_datetime_as_bson_datetime;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct MeasurementMetadataSchema {
    pub facility_id: String,
    pub community_uuid: String,
    pub asset_type: String,
    pub time_slot: u64,
    pub creation_time: u64,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct MeasurementSchema {
    pub facility_id: String,
    pub community_uuid: String,
    #[serde(with = "crate::utils::rfc3339")]
    pub time_slot: DateTime<Utc>,
    #[serde(with = "crate::utils::rfc3339")]
    pub creation_time: DateTime<Utc>,
    pub energy_kwh: f64,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct PVMeasurementSchema {
    pub metadata: MeasurementMetadataSchema,
    pub current_A: f64,
    pub power_kW: f64,
    pub voltage_V: f64,
    pub energy_kWh: f64,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct BatteryMeasurementSchema {
    pub metadata: MeasurementMetadataSchema,
    pub current_A: f64,
    pub power_kW: f64,
    pub power_charge_kW: f64,
    pub power_discharge_kW: f64,
    pub soc: f64,
    pub temperature_C: f64,
    pub voltage_V: f64,
    pub energy_charge_kWh: f64,
    pub energy_discharge_kWh: f64,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct SmartMeterMeasurementSchema {
    pub metadata: MeasurementMetadataSchema,
    pub energy_grid_injection_kWh: f64,
    pub energy_grid_injection_day_kWh: f64,
    pub grid_frequency: f64,
    pub current_A_p1: f64,
    pub current_A_p2: f64,
    pub current_A_p3: f64,
    pub power_kW_p1: f64,
    pub power_kW_p2: f64,
    pub power_kW_p3: f64,
    pub power_kW_pv: f64,
    pub reactive_power_kvar_p1: f64,
    pub reactive_power_kvar_p2: f64,
    pub reactive_power_kvar_p3: f64,
    pub voltage_V_p1: f64,
    pub voltage_V_p2: f64,
    pub voltage_V_p3: f64,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct TransformerMeasurementSchema {
    pub metadata: MeasurementMetadataSchema,
    pub energy_kWh: f64,
    pub grid_frequency: f64,
    pub current_A_p1: f64,
    pub current_A_p2: f64,
    pub current_A_p3: f64,
    pub power_kW_p1: f64,
    pub power_kW_p2: f64,
    pub power_kW_p3: f64,
    pub reactive_power_kvar_p1: f64,
    pub reactive_power_kvar_p2: f64,
    pub reactive_power_kvar_p3: f64,
    pub voltage_V_p1: f64,
    pub voltage_V_p2: f64,
    pub voltage_V_p3: f64,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct ForecastSchema {
    pub facility_id: String,
    pub community_uuid: String,
    #[serde(with = "crate::utils::rfc3339")]
    pub time_slot: DateTime<Utc>,
    #[serde(with = "crate::utils::rfc3339")]
    pub creation_time: DateTime<Utc>,
    pub energy_kwh: f64,
    pub confidence: f64,
}

/// Differentiates a `MeasurementPoint` between measurements from the field and forecasts.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum MeasurementPointType {
    Measurement,
    Forecast,
}

/// Direction of the energy flow relative to the measured asset.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum FlowDirection {
    Import,
    Export,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct MeasurementPointSchema {
    #[serde(rename = "type")]
    pub point_type: MeasurementPointType,
    pub measurement_id: String,
    pub property_measured: String,
    pub unit: String,
    pub direction: FlowDirection,
    pub energy_accumulated: bool,
    pub time_resolution: String,
    pub phase: u8,
    pub asset_name: String,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub datasource_name: Option<String>,
}

impl MeasurementPointSchema {
    /// Positive consumption and negative production for the execution engine.
    pub fn signed_energy_kwh(&self, value: f64) -> f64 {
        // Compatibility profiles already store signed values; their point's
        // direction reflects the latest write and may differ from an older slot.
        if self.property_measured == "energy_measured" || self.direction == FlowDirection::Import {
            value
        } else {
            -value.abs()
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct TimeseriesSchema {
    pub measurement_point: String,
    #[serde(with = "chrono_datetime_as_bson_datetime")]
    pub timestamp: DateTime<Utc>,
    pub value: f64,
}
