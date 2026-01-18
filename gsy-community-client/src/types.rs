use serde::{Deserialize, Serialize};

// Struct for forecast data received from external API
#[derive(Serialize, Deserialize, Debug)]
pub struct ExternalForecast {
    pub area_uuid: String,
    pub community_uuid: String,
    pub time_slot: u64,
    pub creation_time: u64,
    pub energy_kwh: f64,
    pub confidence: f64,
}

// Struct for measurement data received from external API
#[derive(Serialize, Deserialize, Debug)]
pub struct ExternalMeasurement {
    pub area_uuid: String,
    pub community_uuid: String,
    pub time_slot: u64,
    pub creation_time: u64,
    pub energy_kwh: f64,
}