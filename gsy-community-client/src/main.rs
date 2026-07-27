use gsy_community_client::external_api::{
    ExternalCommunityTopology, ExternalForecast, ExternalMeasurement,
};
use gsy_community_client::node_connector::orders::publish_orders;
use gsy_community_client::offchain_storage_connector::adapter::AreaMarketInfoAdapter;
use gsy_community_client::time_utils::{get_current_timestamp_in_secs, get_last_and_next_timeslot};
use primitives::constants::GLOBAL_CONSTANTS;
use primitives::db_api_schema::profiles::{ForecastSchema, MeasurementSchema};
use primitives::MatchingAlgorithm;
use reqwest::Client;
use std::collections::HashSet;
use std::env;
use std::str::FromStr;
use std::time::Duration;
use tokio::time::sleep;
use tracing::{error, info};

#[derive(Clone)]
struct AppState {
    client: Client,
    api_adapter: AreaMarketInfoAdapter,
    evm_node_url: String,
    order_registry_address: String,
    community_signer_private_key: String,
    matching_algorithm: MatchingAlgorithm,
    forecast_url: String,
    measurements_url: String,
    topology_url: String,
}

impl AppState {
    fn new() -> Self {
        AppState {
            client: Client::new(),
            api_adapter: AreaMarketInfoAdapter::new(None),
            evm_node_url: env::var("EVM_NODE_URL")
                .unwrap_or_else(|_| "ws://anvil:8545".to_string()),
            order_registry_address: env::var("ORDER_REGISTRY_ADDRESS")
                .unwrap_or_else(|_| "0x0000000000000000000000000000000000000000".to_string()),
            community_signer_private_key: env::var("COMMUNITY_CLIENT_PRIVATE_KEY").unwrap_or_else(
                |_| {
                    "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80".to_string()
                },
            ),
            matching_algorithm: {
                let configured_value = env::var("MATCHING_ALGORITHM")
                    .unwrap_or_else(|_| MatchingAlgorithm::default().to_string());
                MatchingAlgorithm::from_str(configured_value.as_str())
                    .unwrap_or_else(|error| panic!("Invalid MATCHING_ALGORITHM: {}", error))
            },
            forecast_url: "http://localhost:8000/forecasts".to_string(),
            measurements_url: "http://localhost:8000/measurements".to_string(),
            topology_url: "http://localhost:8000/ontology".to_string(),
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

    async fn poll_and_forward(&self) {
        loop {
            let seconds_since_epoch = get_current_timestamp_in_secs();

            let (_last_timeslot, next_timeslot) = get_last_and_next_timeslot();

            // Fetch topology to validate facility ids, then create the market document for the slot.
            let external_topology_res = self.fetch_topology().await;
            if external_topology_res.is_err() {
                error!(
                    "Failed to fetch external topology: {}",
                    external_topology_res.unwrap_err().to_string()
                );
                continue;
            }
            let external_topology = external_topology_res.unwrap();
            let market = self
                .api_adapter
                .create_market(
                    external_topology.community_uuid.clone(),
                    next_timeslot,
                    self.matching_algorithm.clone(),
                )
                .await
                .unwrap();
            let facility_ids: HashSet<String> = external_topology
                .facilities
                .iter()
                .map(|facility| facility.facility_id.clone())
                .collect();

            match self.fetch_forecasts().await {
                Ok(forecasts) => {
                    let valid_forecasts: Vec<ForecastSchema> = forecasts
                        .into_iter()
                        .filter_map(|forecast| {
                            facility_ids
                                .contains(&forecast.facility_id)
                                .then(|| ForecastSchema {
                                    facility_id: forecast.facility_id,
                                    community_uuid: forecast.community_uuid,
                                    time_slot: forecast.time_slot,
                                    creation_time: forecast.creation_time,
                                    energy_kwh: forecast.energy_kwh,
                                    confidence: forecast.confidence,
                                })
                        })
                        .filter(|forecast| {
                            self.api_adapter
                                .validate_forecast(forecast, seconds_since_epoch)
                        })
                        .collect();
                    if !valid_forecasts.is_empty() {
                        if let Err(e) = self
                            .api_adapter
                            .forward_forecast(valid_forecasts.clone())
                            .await
                        {
                            info!("Failed to forward forecasts: {}", e);
                        }
                        publish_orders(
                            self.evm_node_url.clone(),
                            valid_forecasts.clone(),
                            market.clone(),
                            self.order_registry_address.clone(),
                            self.community_signer_private_key.clone(),
                        )
                        .await
                        .unwrap();
                    } else {
                        info!("No valid forecasts to forward.");
                    }
                }
                Err(e) => error!("Error fetching forecasts: {}", e),
            }

            // Fetch and forward measurements
            match self.fetch_measurements().await {
                Ok(measurements) => {
                    let valid_measurements: Vec<MeasurementSchema> = measurements
                        .into_iter()
                        .filter_map(|measurement| {
                            facility_ids.contains(&measurement.facility_id).then(|| {
                                MeasurementSchema {
                                    facility_id: measurement.facility_id,
                                    community_uuid: measurement.community_uuid,
                                    time_slot: measurement.time_slot,
                                    creation_time: measurement.creation_time,
                                    energy_kwh: measurement.energy_kwh,
                                }
                            })
                        })
                        .filter(|measurement| {
                            self.api_adapter
                                .validate_measurement(measurement, seconds_since_epoch)
                        })
                        .collect();
                    if !valid_measurements.is_empty() {
                        if let Err(e) = self
                            .api_adapter
                            .forward_measurement(valid_measurements)
                            .await
                        {
                            info!("Failed to forward measurements: {}", e);
                        }
                    } else {
                        info!("No valid measurements to forward.");
                    }
                }
                Err(e) => error!("Error fetching measurements: {}", e),
            }

            // Sleep for 15 minutes before polling again
            sleep(Duration::from_secs(GLOBAL_CONSTANTS.time_slot_sec)).await;
        }
    }
}

#[tokio::main]
async fn main() {
    let app_state = AppState::new();
    app_state.poll_and_forward().await;
}
