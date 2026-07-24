use codec::{Decode, Encode};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Encode, Decode, Clone, PartialEq)]
pub enum AssetType {
    EnergyAsset,
    Battery,
    PV,
    Heatpump,
    DistrictHeating,
    EVCharger,
    EnergyInfrastructure,
}

#[derive(Serialize, Deserialize, Debug, Encode, Decode, Clone, PartialEq)]
pub struct AssetSchema {
    pub asset_type: AssetType,
    pub uuid: String,
    pub asset_name: String,
    pub facility_name: String,
    pub creation_time: u64,

    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub installed_power: Option<f64>,

    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub asset_subtype: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub technology_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub phase_connection: Option<String>,

    // Battery-specific.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub energy_capacity: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub maximum_soc: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub minimum_soc: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub roundtrip_efficiency: Option<f64>,

    // Heatpump / DistrictHeating / EVCharger.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub target_service: Option<String>,

    // EnergyInfrastructure-specific.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub grid_connection_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub max_rated_current: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub has_smart_meter: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub tariff_name: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Encode, Decode, Clone, PartialEq)]
pub struct PilotSiteSchema {
    pub pilot_name: String,
    pub pilot_description: String,
    pub start_date: String,
    pub end_date: String,
    pub latitude: f64,
    pub longitude: f64,
    pub communities: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug, Encode, Decode, Clone, PartialEq)]
pub struct EnergyCommunitySchema {
    pub community_name: String,
    pub sites: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug, Encode, Decode, Clone, PartialEq)]
pub struct SiteSchema {
    pub site_name: String,
    pub site_description: String,
    pub facilities: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug, Encode, Decode, Clone, PartialEq)]
pub struct FacilitySchema {
    pub facility_id: String,
    pub facility_name: String,
    pub site_id: String,
    pub owner_id: String,
}
