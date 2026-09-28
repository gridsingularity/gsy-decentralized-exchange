use codec::{Encode, Decode};
use serde::{Deserialize, Serialize};


#[derive(Serialize, Deserialize, Debug, Encode, Decode, Clone, PartialEq)]
pub struct MeasurementSchema {
    pub area_uuid: String,
    pub area_hash: String,
    pub community_uuid: String,
    pub time_slot: u64,
    pub creation_time: u64,
    pub energy_kwh: f64,
    /// `None` on a plain per-area measurement; `Some` on a metering point's row (one building,
    /// or the site-level assets of a site).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metering_point: Option<MeteringPointMeasurement>,
}


/// The metering point a measurement row stands for, and how complete its data is for the slot.
#[derive(Serialize, Deserialize, Debug, Encode, Decode, Clone, PartialEq)]
pub struct MeteringPointMeasurement {
    /// Building `participantName`, or the `siteName` of an unmetered site point.
    pub name: String,
    /// `deterministic_area_hash(community, asset)` of every member asset.
    pub member_area_hashes: Vec<String>,
    pub completeness: MeasurementCompleteness,
    /// Expected Influx meters lacking `import` or `export` for the slot; empty when complete.
    pub missing_meters: Vec<String>,
}


/// Whether a metering point's expected meters all reported for the slot. For `Incomplete` and
/// `Missing`, the row's `energy_kwh` is `0.0` and must not be used.
#[derive(Serialize, Deserialize, Debug, Encode, Decode, Clone, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum MeasurementCompleteness {
    /// Every expected meter reported `import` and `export`; `energy_kwh` is their summed net
    /// grid exchange in kWh, import minus export.
    Complete,
    /// Some expected meters lack data for the slot; they are listed in `missing_meters`.
    Incomplete,
    /// No expected meter reported, or the point has no meters (an unmetered site point).
    Missing,
}


#[derive(Serialize, Deserialize, Debug, Encode, Decode, Clone, PartialEq)]
pub struct ForecastSchema {
    pub area_uuid: String,
    pub area_hash: String,
    pub community_uuid: String,
    pub time_slot: u64,
    pub creation_time: u64,
    pub energy_kwh: f64,
    pub confidence: f64
}
