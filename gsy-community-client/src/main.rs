use std::collections::{HashMap, HashSet};
use gsy_community_client::external_api::{ExternalForecast, ExternalMeasurement, GetLECBuildings, GetLECAssets, ExternalAreaTopology, ExternalCommunityTopology, map_fedecom_asset_type_to_asset_type};
use gsy_community_client::node_connector::orders::publish_orders;
use gsy_community_client::offchain_storage_connector::adapter::AreaMarketInfoAdapter;
use gsy_community_client::time_utils::{get_current_timestamp_in_secs, get_last_and_next_timeslot};
use gsy_offchain_primitives::db_api_schema::profiles::{ForecastSchema, MeasurementSchema};
use gsy_offchain_primitives::constants::GlobalConstants;
use reqwest::Client;
use std::time::Duration;
use subxt_signer::sr25519::dev;
use serde::Serialize;
use tokio::time::sleep;
use tracing::{error, info};
use gsy_offchain_primitives::utils::h256_to_string;
use rand::random;
use subxt::ext::sp_runtime::Deserialize;
use subxt::utils::H256;

#[derive(Clone)]
struct AppState {
	client: Client,
	api_adapter: AreaMarketInfoAdapter,
	gsy_node_url: String,
	forecast_url: String,
	measurements_url: String,
	topology_url: String,
	assets_url: String,
}

#[derive(Serialize)]
struct GetAssetsPostParameters {
	lec: String
}

impl AppState {
	fn new() -> Self {
		AppState {
			client: Client::new(),
			api_adapter: AreaMarketInfoAdapter::new(None),
			gsy_node_url: "http://gsy-node:9944/".to_string(),
			forecast_url: "http://localhost:8000/forecasts".to_string(),
			measurements_url: "http://localhost:8000/measurements".to_string(),
			topology_url: "https://fedecom.tekniker.es/services/queries/get_lecs_buildings".to_string(),
			assets_url: "https://fedecom.tekniker.es/services/queries/get_assets".to_string(),
		}
	}

	// Function to fetch an array of forecast data
	async fn fetch_forecasts(&self) -> Result<Vec<ExternalForecast>, reqwest::Error> {
		let response = self.client.get(&self.forecast_url).send().await?;
		response.json::<Vec<ExternalForecast>>().await
	}

	// Function to fetch an array of measurement data
	async fn fetch_measurements(&self) -> Result<Vec<ExternalMeasurement>, reqwest::Error> {
		let response = self.client.get(&self.measurements_url).send().await?;
		response.json::<Vec<ExternalMeasurement>>().await
	}

	async fn fetch_topology(&self) -> Result<GetLECBuildings, reqwest::Error> {
		let response = self.client.get(&self.topology_url).send().await?;
		response.json::<GetLECBuildings>().await
	}

	async fn fetch_assets(&self, community_name: String) -> Result<GetLECAssets, reqwest::Error> {
		let response = self.client.post(&self.assets_url).body(
			&GetAssetsPostParameters{lec: community_name}).send().await?;
		response.json::<GetLECAssets>().await
	}

	async fn get_all_assets_for_all_communities(
			&self, buildings: GetLECBuildings) -> Vec<ExternalCommunityTopology> {
		let mut communities: Vec<ExternalCommunityTopology> = Vec::new();
		let mut community_uuids: HashSet<String> = HashSet::new();
		for building in buildings.results.bindings {
			if !community_uuids.contains(&building.lec_name.value) {
				community_uuids.insert(building.lec_name.value.clone());
				communities.push(
					ExternalCommunityTopology {
						community_name: building.lec_alt_name.value,
						areas: vec![]
					}
				);
			}
		}

		communities.iter().map(async |community: ExternalCommunityTopology| {
			let assets = self.fetch_assets(community.community_name).await;
			let asset_objects = assets.unwrap().results.bindings.iter().map(
				|asset| {
					let asset_subtype = if asset.asset_sub_type.is_some() {
						Some(asset.asset_sub_type.unwrap().field_type) } else { None };
					ExternalAreaTopology {
						area_name: asset.asset_name.value,
						area_type: map_fedecom_asset_type_to_asset_type(
							asset.asset_type.field_type, asset_subtype)
				}
			}).collect();
			ExternalCommunityTopology {
				areas: asset_objects,
				community_name: community.community_name.clone()
			}

		}).collect()
	}

	async fn poll_and_forward(&self) {
		loop {
			let seconds_since_epoch = get_current_timestamp_in_secs();

			let (_last_timeslot, next_timeslot) = get_last_and_next_timeslot();

			// Fetch topology
			let external_topology_res = self.fetch_topology().await;
			if external_topology_res.is_err() {
				error!(
					"Failed to fetch external topology: {}",
					external_topology_res.unwrap_err().to_string()
				);
				continue;
			}

			let internal_topology = self
				.api_adapter
				.get_or_create_market_topology(
					self.get_all_assets_for_all_communities(external_topology_res.unwrap()).await,
					next_timeslot,
				)
				.await
				.unwrap();
			let area_uuid_to_hash: HashMap<String, String> = internal_topology.community_areas.iter().map(
				|area| {
					(area.area_uuid.clone(), area.area_hash.clone())
				}).collect();
			match self.fetch_forecasts().await {
				Ok(forecasts) => {
					let valid_forecasts: Vec<ForecastSchema> = forecasts
						.into_iter()
						.map(|forecast| {
							self.api_adapter.convert_forecast_to_internal_schema(
								&forecast, area_uuid_to_hash[&forecast.area_uuid].clone())
						})
						.filter(|forecast| {
							self.api_adapter.validate_forecast(forecast, seconds_since_epoch)
						})
						.collect();
					if !valid_forecasts.is_empty() {
						if let Err(e) =
							self.api_adapter.forward_forecast(valid_forecasts.clone()).await
						{
							info!("Failed to forward forecasts: {}", e);
						}
						publish_orders(
							self.gsy_node_url.clone(),
							valid_forecasts.clone(),
							internal_topology.clone(),
							&dev::alice(),
						)
						.await
						.unwrap();
					} else {
						info!("No valid forecasts to forward.");
					}
				},
				Err(e) => error!("Error fetching forecasts: {}", e),
			}

			// Fetch and forward measurements
			match self.fetch_measurements().await {
				Ok(measurements) => {
					let valid_measurements: Vec<MeasurementSchema> = measurements
						.into_iter()
						.map(|measurement| {
							self.api_adapter.convert_measurement_to_internal_schema(
								&measurement, area_uuid_to_hash[&measurement.area_uuid].clone())
						})
						.filter(|measurement| {
							self.api_adapter.validate_measurement(measurement, seconds_since_epoch)
						})
						.collect();
					if !valid_measurements.is_empty() {
						if let Err(e) =
							self.api_adapter.forward_measurement(valid_measurements).await
						{
							info!("Failed to forward measurements: {}", e);
						}
					} else {
						info!("No valid measurements to forward.");
					}
				},
				Err(e) => error!("Error fetching measurements: {}", e),
			}

			// Sleep for 15 minutes before polling again
			sleep(Duration::from_secs(GlobalConstants.TIME_SLOT_SEC)).await;
		}
	}
}

#[tokio::main]
async fn main() {
	let app_state = AppState::new();
	app_state.poll_and_forward().await;
}
