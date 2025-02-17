use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::{error, info};
use std::time::Duration;
use tokio::time::sleep;
use chrono::Local;
use gsy_offchain_primitives::db_api_schema::profiles::{MeasurementSchema, ForecastSchema};
use gsy_offchain_primitives::db_api_schema::market::{AreaTopologySchema, MarketTopologySchema};
use subxt::utils::H256;
use gsy_community_client::node_connector::orders::publish_orders;
use gsy_offchain_primitives::db_api_schema::orders::Order;

// Struct for forecast data received from external API
#[derive(Serialize, Deserialize, Debug)]
struct ExternalForecast {
	asset_uuid: String,
	community_uuid: String,
	time_slot: u64,
	creation_time: u64,
	energy_kwh: f64,
	confidence: f64
}

// Struct for measurement data received from external API
#[derive(Serialize, Deserialize, Debug)]
struct ExternalMeasurement {
	asset_uuid: String,
	community_uuid: String,
	time_slot: u64,
	creation_time: u64,
	energy_kwh: f64,
}


// Struct for forecast data received from external API
#[derive(Serialize, Deserialize, Debug, Clone)]
struct ExternalAreaTopology {
	area_uuid: String,
	area_name: String,
}

// Struct for forecast data received from external API
#[derive(Serialize, Deserialize, Debug, Clone)]
struct ExternalCommunityTopology {
	areas: Vec<ExternalAreaTopology>,
	community_uuid: String,
	community_name: String,
}


#[derive(Clone)]
struct AppState {
	client: Client,
	gsy_node_url: String,
	forecast_url: String,
	measurements_url: String,
	topology_url: String,
	internal_forecast_url: String,
	internal_measurements_url: String,
	internal_topology_url: String,
	internal_community_market_url: String,
}

impl AppState {
	fn new() -> Self {
		AppState {
			client: Client::new(),
			gsy_node_url: "http://gsy-node:9944/".to_string(),
			forecast_url: "http://localhost:8000/forecasts".to_string(),
			measurements_url: "http://localhost:8000/measurements".to_string(),
			topology_url: "http://localhost:8000/ontology".to_string(),
			internal_forecast_url: "http://gsy-orderbook:8080/forecasts".to_string(),
			internal_measurements_url: "http://gsy-orderbook:8080/measurements".to_string(),
			internal_topology_url: "http://gsy-orderbook:8080/market".to_string(),
			internal_community_market_url: "http://gsy-orderbook:8080/community-market".to_string(),
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

	async fn fetch_topology(&self) -> Result<ExternalCommunityTopology, reqwest::Error> {
		let response = self.client.get(&self.topology_url).send().await?;
		response.json::<ExternalCommunityTopology>().await
	}

	// Function to forward the forecast data to internal API
	async fn forward_forecast(&self, forecasts: Vec<ForecastSchema>) -> Result<(), reqwest::Error> {
		self.client
			.post(&self.internal_forecast_url)
			.json(&forecasts)
			.send()
			.await?;
		Ok(())
	}

	// Function to forward the measurement data to internal API
	async fn forward_measurement(&self, measurements: Vec<MeasurementSchema>) -> Result<(), reqwest::Error> {
		self.client
			.post(&self.internal_measurements_url)
			.json(&measurements)
			.send()
			.await?;
		Ok(())
	}

	// Validation logic (basic validation, can be extended)
	fn validate_forecast(&self, forecast: &ForecastSchema, seconds_since_epoch: u64) -> bool {
		forecast.energy_kwh > 0.0 && forecast.time_slot > seconds_since_epoch
	}

	fn validate_measurement(&self, measurement: &MeasurementSchema, seconds_since_epoch: u64) -> bool {
		measurement.energy_kwh > 0.0 && measurement.time_slot <= seconds_since_epoch
	}

	fn convert_forecast_to_internal_schema(&self, forecast: &ExternalForecast) -> ForecastSchema {
		ForecastSchema {
			area_uuid: forecast.asset_uuid.clone(),
			community_uuid: forecast.community_uuid.clone(),
			time_slot: forecast.time_slot,
			creation_time: forecast.creation_time,
			energy_kwh: forecast.energy_kwh,
			confidence: forecast.confidence
		}
	}

	fn convert_measurement_to_internal_schema(&self, measurement: &ExternalMeasurement) -> MeasurementSchema {
		MeasurementSchema {
			area_uuid: measurement.asset_uuid.clone(),
			community_uuid: measurement.community_uuid.clone(),
			time_slot: measurement.time_slot,
			creation_time: measurement.creation_time,
			energy_kwh: measurement.energy_kwh
		}
	}

	async fn update_topology_to_db(&self, topology: ExternalCommunityTopology) -> Option<MarketTopologySchema> {
		let response = self.client.get(&self.internal_community_market_url).send().await;
		match response {
			Ok(response) => {
				Some(response.json::<MarketTopologySchema>().await.unwrap())
			}
			Err(_) => {
				let new_market = MarketTopologySchema {
					community_name: topology.community_name.clone(),
					community_uuid: topology.community_uuid.clone(),
					market_id: H256::random().to_string(),
					time_slot: Local::now().timestamp() as u32, // TODO: Correct timeslot
					creation_time: Local::now().timestamp() as u32,
					area_uuids: topology.areas.clone().into_iter().map(
						|area| AreaTopologySchema {
							area_uuid: area.area_uuid.clone(), name: area.area_name.clone()
						}
					).collect()
				};
				let topology_resp = self.client
					.post(&self.internal_topology_url)
					.json(&new_market)
					.send()
					.await;

				match topology_resp {
					Ok(_) => {
						Some(new_market)
					},
					Err(error) => {
						info!("New topology creation failed with error: {}", error.to_string());
						None
					}
				}
			}
		}
	}
}

async fn poll_and_forward(app_state: AppState) {
	loop {
		let seconds_since_epoch = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
		// Fetch and forward forecasts
		let external_topology_res = app_state.fetch_topology().await;
		match external_topology_res {
			Ok(external_topology) => {
				let ext_topology = external_topology;
				let _internal_topology = app_state.update_topology_to_db(ext_topology.clone()).await.unwrap();
			},
			Err(error) => {
				error!("Failed to fetch external topology: {}", error.to_string());
				continue
			}
		}


		match app_state.fetch_forecasts().await {
			Ok(forecasts) => {
				let valid_forecasts: Vec<ForecastSchema> = forecasts
					.into_iter()
					.map(|forecast| app_state.convert_forecast_to_internal_schema(&forecast))
					.filter(|forecast| app_state.validate_forecast(forecast, seconds_since_epoch))
					.collect();
				if !valid_forecasts.is_empty() {
					if let Err(e) = app_state.forward_forecast(valid_forecasts).await {
						info!("Failed to forward forecasts: {}", e);
					}
					// TODO: Convert forecasts to orders 
					publish_orders(app_state.gsy_node_url.clone(), valid_forecasts).await;
				} else {
					info!("No valid forecasts to forward.");
				}

			}
			Err(e) => error!("Error fetching forecasts: {}", e),
		}

		// Fetch and forward measurements
		match app_state.fetch_measurements().await {
			Ok(measurements) => {
				let valid_measurements: Vec<MeasurementSchema> = measurements
					.into_iter()
					.map(|measurement| app_state.convert_measurement_to_internal_schema(&measurement))
					.filter(|measurement| app_state.validate_measurement(measurement, seconds_since_epoch))
					.collect();
				if !valid_measurements.is_empty() {
					if let Err(e) = app_state.forward_measurement(valid_measurements).await {
						info!("Failed to forward measurements: {}", e);
					}
				} else {
					info!("No valid measurements to forward.");
				}
			}
			Err(e) => error!("Error fetching measurements: {}", e),
		}

		// Sleep for 15 minutes before polling again
		sleep(Duration::from_secs(900)).await;
	}
}


#[tokio::main]
async fn main() {
	let app_state = AppState::new();
	poll_and_forward(app_state).await;
}
