use serde::{Serialize, Deserialize};


// Struct for forecast data received from external API
#[derive(Serialize, Deserialize, Debug)]
pub struct ExternalForecast {
    pub area_uuid: String,
    pub community_uuid: String,
    pub time_slot: u64,
    pub creation_time: u64,
    pub energy_kwh: f64,
    pub confidence: f64
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


// Struct for forecast data received from external API
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ExternalAreaTopology {
    pub area_uuid: String,
    pub area_name: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct NameField {
    pub field_type: String,
    pub value: String,
}


// Struct for forecast data received from external API
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ExternalCommunityMemberTopology {
    pub lec_name: NameField,
    pub lec_alt_name: NameField,
    pub site_name: NameField,
    pub participant_name: NameField
}


#[derive(Deserialize, Debug, Clone)]
pub struct LECCommunityMemberResults {
    pub bindings: Vec<ExternalCommunityMemberTopology>
}


#[derive(Deserialize, Debug, Clone)]
pub struct GetLECBuildings {
    pub results: LECCommunityMemberResults
}

#[derive(Deserialize, Debug, Clone)]
pub struct ExternalCommunityAsset {
    pub location: NameField,
    pub asset_name: NameField,
    pub asset_type: NameField,
    pub asset_sub_type: Option<NameField>
}

#[derive(Deserialize, Debug, Clone)]
pub struct LECCommunityAssetResults {
    pub bindings: Vec<ExternalCommunityAsset>
}

#[derive(Deserialize, Debug, Clone)]
pub struct GetLECAssets {
    pub results: LECCommunityAssetResults
}