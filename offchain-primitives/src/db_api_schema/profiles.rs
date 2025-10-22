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
    pub power_W: f64,
    pub voltage_V: f64,
    pub energy_yield_total_Wh: f64,
}


#[derive(Serialize, Deserialize, Debug, Encode, Decode, Clone, PartialEq)]
pub struct BatteryMeasurementSchema {
    pub metadata: MeasurementMetadataSchema,
    pub current_A: f64,
    pub power_W: f64,
    pub power_charge_W: f64,
    pub power_discharge_W: f64,
    pub soc: f64,
    pub temperature_C: f64,
    pub voltage_V: f64
}


#[derive(Serialize, Deserialize, Debug, Encode, Decode, Clone, PartialEq)]
pub struct SmartMeterMeasurementSchema {
    pub metadata: MeasurementMetadataSchema,
    pub energy_grid_injection_Wh: f64,
    pub energy_grid_injection_day_Wh: f64,
    pub grid_frequency: f64,
    pub current_A_p1: f64,
    pub current_A_p2: f64,
    pub current_A_p3: f64,
    pub power_W_p1: f64,
    pub power_W_p2: f64,
    pub power_W_p3: f64,
    pub power_W_pv: f64,
    pub reactive_power_var_p1: f64,
    pub reactive_power_var_p2: f64,
    pub reactive_power_var_p3: f64,
    pub voltage_V_p1: f64,
    pub voltage_V_p2: f64,
    pub voltage_V_p3: f64,
}


#[derive(Serialize, Deserialize, Debug, Encode, Decode, Clone, PartialEq)]
pub struct TransformerMeasurementSchema {

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
