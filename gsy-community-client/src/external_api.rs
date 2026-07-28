use serde::{Deserialize, Serialize};

// Struct for forecast data received from external API
#[derive(Serialize, Deserialize, Debug)]
pub struct ExternalForecast {
    pub facility_id: String,
    pub community_uuid: String,
    pub time_slot: u64,
    pub creation_time: u64,
    pub energy_kwh: f64,
    pub confidence: f64,
}

// Struct for measurement data received from external API
#[derive(Serialize, Deserialize, Debug)]
pub struct ExternalMeasurement {
    pub facility_id: String,
    pub community_uuid: String,
    pub time_slot: u64,
    pub creation_time: u64,
    pub energy_kwh: f64,
}

// Struct for forecast data received from external API
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ExternalFacilityTopology {
    pub facility_id: String,
    pub facility_name: String,
}

// Struct for forecast data received from external API
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ExternalCommunityTopology {
    pub facilities: Vec<ExternalFacilityTopology>,
    pub community_uuid: String,
    pub community_name: String,
}
