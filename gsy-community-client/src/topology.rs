use crate::constants::CommunityClientConstants;
use crate::offchain_storage_connector::adapter::AreaMarketInfoAdapter;
use gsy_offchain_primitives::db_api_schema::market::{AssetType, MarketTopologySchema};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use tracing::error;

#[derive(Deserialize, Serialize)]
struct GetBuildingsPostParameters {
    params: HashMap<String, String>,
}

#[derive(Deserialize, Serialize)]
struct GetAssetsLECParameters {
    lec: String,
}

#[derive(Deserialize, Serialize)]
struct GetAssetsPostParameters {
    params: GetAssetsLECParameters,
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

#[derive(Clone)]
pub struct TopologyManager {
    client: Client,
    api_adapter: AreaMarketInfoAdapter,
    topology_url: String,
    assets_url: String,
}

impl TopologyManager {
    pub fn new(client: &Client, api_adapter: &AreaMarketInfoAdapter) -> Self {
        TopologyManager {
            client: client.clone(),
            api_adapter: api_adapter.clone(),
            topology_url: CommunityClientConstants.FEDECOM_ONTOLOGY_URL.clone(),
            assets_url: CommunityClientConstants.FEDECOM_ONTOLOGY_ASSETS_URL.clone(),
        }
    }

    fn map_fedecom_asset_type_to_asset_type(
        &self,
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
            _ => AssetType::AREA,
        }
    }

    pub async fn fetch_topology(&self) -> Result<LECCommunityMembersResults, reqwest::Error> {
        let params = GetBuildingsPostParameters {
            params: HashMap::new(),
        };
        let response = self
            .client
            .post(&self.topology_url)
            .json(&params)
            .send()
            .await?;
        response.json::<LECCommunityMembersResults>().await
    }

    async fn fetch_assets(
        &self,
        community_name: String,
    ) -> Result<LECCommunityAssetsResults, reqwest::Error> {
        let post_parameters = GetAssetsPostParameters {
            params: GetAssetsLECParameters {
                lec: community_name,
            },
        };

        let response = self
            .client
            .post(&self.assets_url)
            .json(&post_parameters)
            .send()
            .await?;
        response.json::<LECCommunityAssetsResults>().await
    }

    async fn get_all_assets_for_all_communities(
        &self,
        buildings: LECCommunityMembersResults,
    ) -> Vec<ExternalCommunityTopology> {
        let mut communities: Vec<ExternalCommunityTopology> = Vec::new();
        let mut community_uuids: HashSet<String> = HashSet::new();

        for building in buildings.results.bindings {
            if !community_uuids.contains(&building.lec_name.value) {
                community_uuids.insert(building.lec_name.value.clone());
                communities.push(ExternalCommunityTopology {
                    community_name: building.lec_name.value,
                    areas: vec![],
                });
            }
        }

        let mut external_topologies: Vec<ExternalCommunityTopology> = vec![];
        for community in communities {
            let assets = self.fetch_assets(community.community_name.clone()).await;
            let mut asset_objects: Vec<ExternalAreaTopology> = vec![];
            for asset in assets.unwrap().results.bindings {
                let asset_subtype = if asset.asset_sub_type.is_some() {
                    Some(asset.asset_sub_type.unwrap().field_type)
                } else {
                    None
                };
                asset_objects.push(ExternalAreaTopology {
                    area_name: asset.asset_name.value,
                    area_type: self.map_fedecom_asset_type_to_asset_type(
                        asset.asset_type.field_type,
                        asset_subtype,
                    ),
                });
            }

            external_topologies.push(ExternalCommunityTopology {
                areas: asset_objects,
                community_name: community.community_name.clone(),
            });
        }

        external_topologies
    }

    pub async fn get(&self, next_timeslot: u64) -> Vec<MarketTopologySchema> {
        // Fetch topology
        let external_topology_res = self.fetch_topology().await;
        match external_topology_res {
            Ok(topology) => {
                let all_assets = self.get_all_assets_for_all_communities(topology).await;
                let retval = self
                    .api_adapter
                    .get_or_create_market_topology(all_assets, next_timeslot)
                    .await;
                retval
            }
            Err(error) => {
                error!("Failed to fetch external topology: {}", error);
                vec![]
            }
        }
    }
}
