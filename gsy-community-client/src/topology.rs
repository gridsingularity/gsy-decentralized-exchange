use crate::constants::CommunityClientConstants;
use crate::offchain_storage_connector::adapter::AreaMarketInfoAdapter;
use crate::sites::SiteIndex;
use gsy_offchain_primitives::db_api_schema::market::{AssetType, MarketTopologySchema};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap, HashSet};
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

/// The raw ontology responses: the buildings (`get_lecs_buildings`), and the assets
/// (`get_assets`) per LEC name.
#[derive(Debug, Clone)]
pub struct RawOntology {
    pub buildings: LECCommunityMembersResults,
    pub assets: Vec<(String, LECCommunityAssetsResults)>,
}

impl RawOntology {
    /// The LECs named by the buildings whose assets are absent (their query failed), in
    /// the order the buildings name them.
    pub fn missing_lecs(&self) -> Vec<String> {
        let mut missing: Vec<String> = Vec::new();
        for building in &self.buildings.results.bindings {
            let lec = &building.lec_name.value;
            if !self.assets.iter().any(|(fetched, _)| fetched == lec) && !missing.contains(lec) {
                missing.push(lec.clone());
            }
        }
        missing
    }
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
        // Compared without the URI scheme. The ontology serves these as `https://`, but every
        // arm except the heat pump was written as `http://`, so Meter and PVSystem silently
        // fell through to AREA -- which made `is_forecastable_meter` and `is_pv_asset` always
        // false and left the forecast path unable to produce a single point.
        let strip_scheme = |uri: &str| -> String {
            uri.split_once("://").map(|(_, rest)| rest).unwrap_or(uri).to_string()
        };
        match strip_scheme(&external_asset_type).as_str() {
            "w3id.org/fedecom/battery#Battery" => AssetType::BATTERY,
            "w3id.org/fedecom/energyasset#Meter" => {
                // The assets endpoint currently omits `assetSubType` entirely, so meters
                // resolve to SMART_METER; both types are forecastable.
                if external_asset_subtype.map(|subtype| strip_scheme(&subtype)).as_deref()
                    == Some("w3id.org/fedecom/energyasset#GridMeter")
                {
                    AssetType::GRID_METER
                } else {
                    AssetType::SMART_METER
                }
            }
            "w3id.org/fedecom/energyasset#Boiler" => AssetType::BOILER,
            "w3id.org/fedecom/energyasset#EVCharger" => AssetType::EV,
            "w3id.org/hpont#HeatPumpSystem" => AssetType::HEAT_PUMP,
            "w3id.org/fedecom/energyasset#PVSystem" => AssetType::PV,
            _ => AssetType::AREA,
        }
    }

    pub async fn fetch_topology(&self) -> Result<LECCommunityMembersResults, reqwest::Error> {
        let params = GetBuildingsPostParameters {
            params: HashMap::new(),
        };
        // Status checked before decoding, so a 5xx is reported as such rather than as
        // "error decoding response body".
        let response = self
            .client
            .post(&self.topology_url)
            .json(&params)
            .send()
            .await?
            .error_for_status()?;
        response.json::<LECCommunityMembersResults>().await
    }

    pub async fn fetch_assets(
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
            .await?
            .error_for_status()?;
        response.json::<LECCommunityAssetsResults>().await
    }

    /// Fetch the raw ontology: the buildings, then the assets of every LEC they name. A LEC
    /// whose asset query fails is logged and left out; the others are still returned. Only
    /// a failing buildings query is an error.
    pub async fn fetch_raw_ontology(&self) -> Result<RawOntology, reqwest::Error> {
        let buildings = self.fetch_topology().await?;

        let mut lecs: Vec<String> = Vec::new();
        let mut seen_lecs: HashSet<&str> = HashSet::new();
        for building in &buildings.results.bindings {
            if seen_lecs.insert(building.lec_name.value.as_str()) {
                lecs.push(building.lec_name.value.clone());
            }
        }

        let mut assets = Vec::with_capacity(lecs.len());
        for lec in lecs {
            match self.fetch_assets(lec.clone()).await {
                Ok(lec_assets) => assets.push((lec, lec_assets)),
                Err(error) => {
                    error!("Failed to fetch the assets of community {}: {}", lec, error);
                }
            }
        }

        Ok(RawOntology { buildings, assets })
    }

    /// One community per ontology site (`community_name` = the `siteName`), sorted by name.
    /// An asset belongs to its site as resolved by [`SiteIndex`]; an asset without a site is
    /// logged and left out. This is the grouping markets, forecasts and orders use.
    pub fn communities_by_site(&self, raw: &RawOntology) -> Vec<ExternalCommunityTopology> {
        let index = SiteIndex::new(&raw.buildings);
        let mut bindings_per_site: BTreeMap<String, Vec<ExternalCommunityAsset>> = BTreeMap::new();
        for (lec, lec_assets) in &raw.assets {
            for asset in &lec_assets.results.bindings {
                match index.site_of_asset(lec, asset) {
                    Some(site) => bindings_per_site
                        .entry(site.to_string())
                        .or_default()
                        .push(asset.clone()),
                    None => error!(
                        "{}: asset {} has location {}, which is neither a building nor a site \
                         of {}; it is left out of every market",
                        lec, asset.asset_name.value, asset.location.value, lec
                    ),
                }
            }
        }
        bindings_per_site
            .into_iter()
            .map(|(site, bindings)| ExternalCommunityTopology {
                areas: self.map_assets_to_topology(LECCommunityAssetsResults {
                    results: _LECCommunityAssetResults { bindings },
                }),
                community_name: site,
            })
            .collect()
    }

    /// One community per ontology LEC (`Pilot1`, ...), in the order the buildings name
    /// them. Asset DIDs are keyed on these communities.
    pub fn communities_by_lec(&self, raw: &RawOntology) -> Vec<ExternalCommunityTopology> {
        let mut communities: Vec<ExternalCommunityTopology> = Vec::new();
        for (lec, lec_assets) in &raw.assets {
            let areas = self.map_assets_to_topology(lec_assets.clone());
            match communities
                .iter_mut()
                .find(|community| &community.community_name == lec)
            {
                Some(community) => community.areas.extend(areas),
                None => communities.push(ExternalCommunityTopology {
                    community_name: lec.clone(),
                    areas,
                }),
            }
        }
        communities
    }

    /// Map a parsed `LECCommunityAssetsResults` response into the internal topology
    /// representation. This is a pure transformation (no I/O) and is `pub` so that
    /// unit tests can exercise the full field-selection + mapping path without making
    /// HTTP calls.
    pub fn map_assets_to_topology(
        &self,
        assets: LECCommunityAssetsResults,
    ) -> Vec<ExternalAreaTopology> {
        let mut asset_objects: Vec<ExternalAreaTopology> = vec![];
        for asset in assets.results.bindings {
            let asset_subtype = if asset.asset_sub_type.is_some() {
                Some(asset.asset_sub_type.unwrap().value)
            } else {
                None
            };
            asset_objects.push(ExternalAreaTopology {
                area_name: asset.asset_name.value,
                area_type: self
                    .map_fedecom_asset_type_to_asset_type(asset.asset_type.value, asset_subtype),
            });
        }
        asset_objects
    }

    /// Fetch the external ontology topology once, one community per site,
    /// timeslot-independent (no per-timeslot market is created or looked up). Used by the
    /// day-ahead ingestion loop, which derives its own deterministic area/community ids
    /// straight from this topology instead of going through
    /// [`Self::get`]/[`Self::get_for_timeslots`].
    pub async fn fetch_all_topology(&self) -> Vec<ExternalCommunityTopology> {
        match self.fetch_raw_ontology().await {
            Ok(raw) => self.communities_by_site(&raw),
            Err(error) => {
                error!("Failed to fetch external topology: {}", error);
                vec![]
            }
        }
    }

    /// Like [`Self::fetch_all_topology`], but one community per ontology LEC.
    pub async fn fetch_all_topology_by_lec(&self) -> Vec<ExternalCommunityTopology> {
        match self.fetch_raw_ontology().await {
            Ok(raw) => self.communities_by_lec(&raw),
            Err(error) => {
                error!("Failed to fetch external topology: {}", error);
                vec![]
            }
        }
    }

    pub async fn get(&self, next_timeslot: u64) -> Vec<MarketTopologySchema> {
        // Fetch topology
        match self.fetch_raw_ontology().await {
            Ok(raw) => {
                let all_assets = self.communities_by_site(&raw);
                self.api_adapter
                    .get_or_create_market_topology(all_assets, next_timeslot)
                    .await
            }
            Err(error) => {
                error!("Failed to fetch external topology: {}", error);
                vec![]
            }
        }
    }

    /// Fetch the external topology once and recreate the per-site markets for every
    /// requested delivery timeslot, returning the markets paired with their timeslot.
    pub async fn get_for_timeslots(
        &self,
        timeslots: &[u64],
    ) -> Vec<(u64, Vec<MarketTopologySchema>)> {
        match self.fetch_raw_ontology().await {
            Ok(raw) => {
                let all_assets = self.communities_by_site(&raw);
                let mut markets_per_timeslot = Vec::with_capacity(timeslots.len());
                for &timeslot in timeslots {
                    let markets = self
                        .api_adapter
                        .get_or_create_market_topology(all_assets.clone(), timeslot)
                        .await;
                    markets_per_timeslot.push((timeslot, markets));
                }
                markets_per_timeslot
            }
            Err(error) => {
                error!("Failed to fetch external topology: {}", error);
                vec![]
            }
        }
    }
}
