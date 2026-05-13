//! Measurements Storage schemas, as specified in D3.2 §5.2.
//!
//! Measurement and forecast metadata is kept separate from the value
//! timeseries: `MeasurementPoint` documents describe what is being
//! measured (and serve as the parent document referenced by the
//! timeseries), while `Timeseries` documents hold the actual values.
//! `asset_name` is indexed on `MeasurementPoint`; `measurement_point`
//! and `timestamp` are indexed on `Timeseries`.

#![allow(non_snake_case)]

use codec::{Decode, Encode};
use serde::{Deserialize, Serialize};

/// Discriminates a `MeasurementPoint` between measurements (telemetry
/// from the field) and forecasts (predicted values).
#[derive(Serialize, Deserialize, Debug, Encode, Decode, Clone, PartialEq)]
pub enum MeasurementPointType {
    Measurement,
    Forecast,
}

/// Direction of an energy flow relative to the measured asset.
#[derive(Serialize, Deserialize, Debug, Encode, Decode, Clone, PartialEq)]
pub enum FlowDirection {
    Import,
    Export,
}

#[derive(Serialize, Deserialize, Debug, Encode, Decode, Clone, PartialEq)]
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

#[derive(Serialize, Deserialize, Debug, Encode, Decode, Clone, PartialEq)]
pub struct TimeseriesSchema {
    pub measurement_point: String,
    pub timestamp: String,
    pub value: f64,
}
