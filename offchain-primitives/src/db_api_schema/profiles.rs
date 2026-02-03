#![allow(non_snake_case)]

use codec::{Encode, Decode};
use serde::{Deserialize, Serialize};


#[derive(Serialize, Deserialize, Debug, Encode, Decode, Clone, PartialEq)]
pub struct MeasurementMetadataSchema {
    pub area_uuid: String,
    pub community_uuid: String,
    pub asset_type: String,
    pub time_slot: u64,
    pub creation_time: u64,
}


#[derive(Serialize, Deserialize, Debug, Encode, Decode, Clone, PartialEq)]
pub struct MeasurementSchema {
    pub area_uuid: String,
    pub community_uuid: String,
    pub time_slot: u64,
    pub creation_time: u64,
    pub energy_kwh: f64
}


#[derive(Serialize, Deserialize, Debug, Encode, Decode, Clone, PartialEq)]
pub struct PVMeasurementSchema {
    pub metadata: MeasurementMetadataSchema,
    pub current_A: f64,
    pub power_kW: f64,
    pub voltage_V: f64,
    pub energy_kWh: f64,
}


#[derive(Serialize, Deserialize, Debug, Encode, Decode, Clone, PartialEq)]
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
    pub energy_discharge_kWh: f64
}


#[derive(Serialize, Deserialize, Debug, Encode, Decode, Clone, PartialEq)]
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


#[derive(Serialize, Deserialize, Debug, Encode, Decode, Clone, PartialEq)]
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


#[derive(Serialize, Deserialize, Debug, Encode, Decode, Clone, PartialEq)]
pub struct ForecastSchema {
    pub area_uuid: String,
    pub community_uuid: String,
    pub time_slot: u64,
    pub creation_time: u64,
    pub energy_kwh: f64,
    pub confidence: f64
}
