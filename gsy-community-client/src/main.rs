use gsy_community_client::external_measurements::manager::MeasurementsManager;
use gsy_community_client::node_connector::orders::publish_orders;
use gsy_community_client::offchain_storage_connector::adapter::AreaMarketInfoAdapter;
use gsy_community_client::time_utils::{get_current_timestamp_in_secs, get_last_and_next_timeslot};
use gsy_community_client::topology::TopologyManager;
use gsy_community_client::types::ExternalForecast;
use gsy_offchain_primitives::constants::GlobalConstants;
use gsy_offchain_primitives::db_api_schema::profiles::ForecastSchema;
use reqwest::Client;
use std::collections::HashMap;
use std::time::Duration;
use subxt_signer::sr25519::dev;
use tokio::time::sleep;
use tracing::{error, info};

#[derive(Clone)]
struct AppState {
    client: Client,
    api_adapter: AreaMarketInfoAdapter,
    measurements: MeasurementsManager,
    gsy_node_url: String,
    forecast_url: String,
}

impl AppState {
    fn new() -> Self {
        let api_adapter = AreaMarketInfoAdapter::new(None);
        AppState {
            client: Client::new(),
            api_adapter,
            measurements: MeasurementsManager::new(),
            gsy_node_url: "http://gsy-node:9944/".to_string(),
            forecast_url: "http://localhost:8000/forecasts".to_string(),
        }
    }

    // Function to fetch an array of forecast data
    async fn fetch_forecasts(&self) -> Result<Vec<ExternalForecast>, reqwest::Error> {
        let response = self.client.get(&self.forecast_url).send().await?;
        response.json::<Vec<ExternalForecast>>().await
    }

    async fn poll_and_forward(&self) {
        loop {
            let seconds_since_epoch = get_current_timestamp_in_secs();

            let (_last_timeslot, next_timeslot) = get_last_and_next_timeslot();

            let internal_topology = TopologyManager::new(&self.client, &self.api_adapter)
                .get(next_timeslot)
                .await;

            self.measurements
                .fetch_and_forward(internal_topology.clone(), seconds_since_epoch)
                .await;

            // TODO: Fetch forecast data from MySQL Fedecom DB
            for market in internal_topology.clone() {
                let area_uuid_to_hash: HashMap<String, String> = market
                    .community_areas
                    .iter()
                    .map(|area| (area.area_uuid.clone(), area.area_hash.clone()))
                    .collect();
                match self.fetch_forecasts().await {
                    Ok(forecasts) => {
                        let valid_forecasts: Vec<ForecastSchema> = forecasts
                            .into_iter()
                            .map(|forecast| {
                                self.api_adapter.convert_forecast_to_internal_schema(
                                    &forecast,
                                    area_uuid_to_hash[&forecast.area_uuid].clone(),
                                )
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
                                self.gsy_node_url.clone(),
                                valid_forecasts.clone(),
                                market.clone(),
                                &dev::alice(),
                            )
                            .await
                            .unwrap();
                        } else {
                            info!("No valid forecasts to forward.");
                        }
                    }
                    Err(e) => error!("Error fetching forecasts: {}", e),
                }
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
