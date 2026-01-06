use gsy_offchain_primitives::db_api_schema::market::AssetType;
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

pub fn map_fedecom_asset_type_to_asset_type(
    external_asset_type: String,
    external_asset_subtype: Option<String>,
) -> AssetType {
    match external_asset_type.as_str() {
        "http://w3id.org/fedecom/battery#Battery" => AssetType::BATTERY,
        "http://w3id.org/fedecom/energyasset#Meter" => {
            if external_asset_subtype.is_some()
                && external_asset_subtype
                    .unwrap()
                    .eq("http://w3id.org/fedecom/energyasset#GridMeter")
            {
                AssetType::GRID_METER
            } else {
                AssetType::SMART_METER
            }
        }
        "http://w3id.org/fedecom/energyasset#Boiler" => AssetType::BOILER,
        "http://w3id.org/fedecom/energyasset#EVCharger" => AssetType::EV,
        "https://w3id.org/hpont#HeatPumpSystem" => AssetType::HEAT_PUMP,
        "http://w3id.org/fedecom/energyasset#PVSystem" => AssetType::PV,
        _ => AssetType::UNKNOWN,
    }
}

// Struct for forecast data received from external API
#[derive(Serialize, Deserialize, Debug, Clone, Hash, Eq, PartialEq)]
pub struct ExternalAreaTopology {
    pub area_name: String,
    pub area_type: AssetType,
}

// Struct for forecast data received from external API
#[derive(Serialize, Deserialize, Debug, Clone, Hash, Eq, PartialEq)]
pub struct ExternalCommunityTopology {
    pub community_name: String,
    pub areas: Vec<ExternalAreaTopology>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct NameField {
    #[serde(rename = "type")]
    pub field_type: String,
    pub value: String,
}

// Struct for forecast data received from external API
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ExternalCommunityMemberTopology {
    #[serde(rename = "lecName")]
    pub lec_name: NameField,
    #[serde(rename = "lecAltName")]
    pub lec_alt_name: NameField,
    #[serde(rename = "siteName")]
    pub site_name: NameField,
    #[serde(rename = "participantName")]
    pub participant_name: NameField,
}

#[derive(Deserialize, Debug, Clone)]
pub struct _LECCommunityMemberResults {
    pub bindings: Vec<ExternalCommunityMemberTopology>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct LECCommunityMembersResults {
    pub results: _LECCommunityMemberResults,
}

#[derive(Deserialize, Debug, Clone)]
pub struct ExternalCommunityAsset {
    pub location: NameField,
    #[serde(rename = "assetName")]
    pub asset_name: NameField,
    #[serde(rename = "assetType")]
    pub asset_type: NameField,
    #[serde(rename = "assetSubType")]
    pub asset_sub_type: Option<NameField>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct _LECCommunityAssetResults {
    pub bindings: Vec<ExternalCommunityAsset>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct LECCommunityAssetsResults {
    pub results: _LECCommunityAssetResults,
}
