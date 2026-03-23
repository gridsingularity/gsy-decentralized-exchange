use gsy_community_client::external_api::{
    ExternalCommunityTopology, ExternalForecast, ExternalMeasurement,
};
use gsy_community_client::node_connector::orders::publish_orders;
use gsy_community_client::offchain_storage_connector::adapter::AreaMarketInfoAdapter;
use gsy_community_client::time_utils::{get_current_timestamp_in_secs, get_last_and_next_timeslot};
use gsy_offchain_primitives::constants::GLOBAL_CONSTANTS;
use gsy_offchain_primitives::db_api_schema::profiles::{ForecastSchema, MeasurementSchema};
use reqwest::Client;
use std::collections::HashMap;
use std::env;
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

            // Fetch and forward topology
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
                    external_topology_res.unwrap().clone(),
                    next_timeslot,
                )
                .await
                .unwrap();
            let area_uuid_to_hash: HashMap<String, String> = internal_topology
                .community_areas
                .iter()
                .map(|area| (area.area_uuid.clone(), area.area_uuid.clone()))
                .collect();
            match self.fetch_forecasts().await {
                Ok(forecasts) => {
                    let valid_forecasts: Vec<ForecastSchema> = forecasts
                        .into_iter()
                        .filter_map(|forecast| {
                            area_uuid_to_hash.get(&forecast.area_uuid).map(|area_hash| {
                                self.api_adapter.convert_forecast_to_internal_schema(
                                    &forecast,
                                    area_hash.clone(),
                                )
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
                            internal_topology.clone(),
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
                            area_uuid_to_hash
                                .get(&measurement.area_uuid)
                                .map(|area_hash| {
                                    self.api_adapter.convert_measurement_to_internal_schema(
                                        &measurement,
                                        area_hash.clone(),
                                    )
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
