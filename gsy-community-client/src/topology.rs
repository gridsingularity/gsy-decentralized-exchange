use std::collections::HashSet;
use crate::external_api::{
    ExternalAreaTopology, ExternalCommunityTopology, LECCommunityAssetsResults,
    LECCommunityMembersResults, map_fedecom_asset_type_to_asset_type,
};
use crate::offchain_storage_connector::adapter::AreaMarketInfoAdapter;
use gsy_offchain_primitives::db_api_schema::market::MarketTopologySchema;
use reqwest::Client;
use serde::{Serialize, Deserialize};
use tracing::error;


#[derive(Deserialize, Serialize)]
struct GetAssetsPostParameters {
    lec: String,
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
            topology_url: "https://fedecom.tekniker.es/services/queries/get_lecs_buildings"
                .to_string(),
            assets_url: "https://fedecom.tekniker.es/services/queries/get_assets".to_string(),
        }
    }

    async fn fetch_topology(&self) -> Result<LECCommunityMembersResults, reqwest::Error> {
        let response = self.client.post(&self.topology_url).send().await?;
        response.json::<LECCommunityMembersResults>().await
    }

    async fn fetch_assets(
        &self,
        community_name: String,
    ) -> Result<LECCommunityAssetsResults, reqwest::Error> {
        let post_parameters = GetAssetsPostParameters {
            lec: community_name,
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
                    community_name: building.lec_alt_name.value,
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
                    area_type: map_fedecom_asset_type_to_asset_type(
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
        if external_topology_res.is_err() {
            error!(
                    "Failed to fetch external topology: {}",
                    external_topology_res.unwrap_err().to_string()
                );
            return vec![]
        }

        self
            .api_adapter
            .get_or_create_market_topology(
                self.get_all_assets_for_all_communities(external_topology_res.unwrap())
                    .await,
                next_timeslot,
            ).await
    }
}

