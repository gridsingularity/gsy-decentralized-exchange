use gsy_community_client::external_api::{ExternalForecast, ExternalMeasurement, MeasurementInfluxDBConnection};
use gsy_community_client::node_connector::orders::publish_orders;
use gsy_community_client::offchain_storage_connector::adapter::AreaMarketInfoAdapter;
use gsy_community_client::time_utils::{get_current_timestamp_in_secs, get_last_and_next_timeslot};
use gsy_community_client::topology::TopologyManager;
use gsy_offchain_primitives::constants::GlobalConstants;
use gsy_offchain_primitives::db_api_schema::profiles::{ForecastSchema, MeasurementSchema};
use reqwest::Client;
use chrono::Utc;
use std::collections::{HashMap, HashSet};
use subxt_signer::sr25519::dev;
use std::time::Duration;
use tokio::time::sleep;
use tracing::{error, info};
use gsy_offchain_primitives::db_api_schema::market::MarketTopologySchema;

#[derive(Clone)]
struct AppState {
    client: Client,
    api_adapter: AreaMarketInfoAdapter,
    external_measurements_api: MeasurementInfluxDBConnection,
    gsy_node_url: String,
    forecast_url: String,
}

impl AppState {
    fn new() -> Self {
        AppState {
            client: Client::new(),
            api_adapter: AreaMarketInfoAdapter::new(None),
            external_measurements_api: MeasurementInfluxDBConnection::new(),
            gsy_node_url: "http://gsy-node:9944/".to_string(),
            forecast_url: "http://localhost:8000/forecasts".to_string(),
        }
    }

    // Function to fetch an array of forecast data
    async fn fetch_forecasts(&self) -> Result<Vec<ExternalForecast>, reqwest::Error> {
        let response = self.client.get(&self.forecast_url).send().await?;
        response.json::<Vec<ExternalForecast>>().await
    }

    // Function to fetch an array of measurement data
    async fn fetch_measurements(&self, topologies: Vec<MarketTopologySchema>) -> Vec<ExternalMeasurement> {
        let start_time = Utc::now() - Duration::from_secs(2 * GlobalConstants.TIME_SLOT_SEC);
        let end_time = Utc::now();
        let measurements = self.external_measurements_api.read(start_time, end_time).await;

        let mut external_measurements: Vec<ExternalMeasurement> = vec![];
        for topology in topologies.iter() {
            let topology_member_ids: HashSet<String> = HashSet::from_iter(
                topology.community_areas.iter().map(|area| area.name.clone()));
            for (sensor_id, timestamp_hashmap) in measurements.clone().into_iter() {
                // TODO: Create a manual mapping between ontology sensor ids and Influx sensor ids
                if topology_member_ids.contains(&sensor_id) {
                    // This sensor is part of the community. Create external measurements.
                    for (timestamp, record) in timestamp_hashmap.clone().into_iter() {
                        external_measurements.push(ExternalMeasurement {
                            community_uuid: topology.community_uuid.clone(),
                            area_uuid: sensor_id.clone(),
                            time_slot: timestamp.timestamp() as u64,
                            creation_time: Utc::now().timestamp() as u64,
                            energy_kwh: record.net_energy_Wh(),
                        })
                    }
                }
            }

        }
        external_measurements
    }

    async fn poll_and_forward(&self) {

        loop {
            let seconds_since_epoch = get_current_timestamp_in_secs();

            let (_last_timeslot, next_timeslot) = get_last_and_next_timeslot();

            let internal_topology = TopologyManager::new(
                &self.client, &self.api_adapter).get(next_timeslot).await;

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

                // Fetch and forward measurements
                let measurements = self.fetch_measurements(internal_topology.clone()).await;
                let valid_measurements: Vec<MeasurementSchema> = measurements
                    .into_iter()
                    .map(|measurement| {
                        self.api_adapter.convert_measurement_to_internal_schema(
                            &measurement,
                            area_uuid_to_hash[&measurement.area_uuid].clone(),
                        )
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
