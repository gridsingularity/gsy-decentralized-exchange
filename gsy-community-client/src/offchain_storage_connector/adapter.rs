use reqwest::Client;
use subxt::utils::H256;
use tracing::info;

use gsy_offchain_primitives::db_api_schema::profiles::{MeasurementSchema, ForecastSchema};
use gsy_offchain_primitives::db_api_schema::market::{AreaTopologySchema, MarketTopologySchema};
use gsy_offchain_primitives::utils::h256_to_string;
use crate::time_utils::get_current_timestamp_in_secs;
use crate::external_api::{
    ExternalForecast, ExternalMeasurement, ExternalCommunityTopology};


#[derive(Clone)]
pub struct AreaMarketInfoAdapter {
    client: Client,
    internal_forecast_url: String,
    internal_measurements_url: String,
    pub internal_topology_url: String,
    pub internal_community_market_url: String,
}

impl AreaMarketInfoAdapter {
    pub fn new(host: Option<String>) -> Self {
        let hostname = host.unwrap_or_else(|| "http://gsy-orderbook:8080".to_string());
        AreaMarketInfoAdapter {
            client: Client::new(),
            internal_forecast_url: hostname.clone() + "/forecasts",
            internal_measurements_url: hostname.clone() + "/measurements",
            internal_topology_url: hostname.clone() + "/market",
            internal_community_market_url: hostname.clone() + "/community-market",
        }
    }

    // Function to forward the forecast data to internal API
    pub async fn forward_forecast(&self, forecasts: Vec<ForecastSchema>) -> Result<(), reqwest::Error> {
        self.client
            .post(&self.internal_forecast_url)
            .json(&forecasts)
            .send()
            .await?;
        Ok(())
    }

    // Function to forward the measurement data to internal API
    pub async fn forward_measurement(&self, measurements: Vec<MeasurementSchema>) -> Result<(), reqwest::Error> {
        self.client
            .post(&self.internal_measurements_url)
            .json(&measurements)
            .send()
            .await?;
        Ok(())
    }

    // Validation logic (basic validation, can be extended)
    pub fn validate_forecast(&self, forecast: &ForecastSchema, seconds_since_epoch: u64) -> bool {
        forecast.energy_kwh > 0.0 && forecast.time_slot > seconds_since_epoch
    }

    pub fn validate_measurement(&self, measurement: &MeasurementSchema, seconds_since_epoch: u64) -> bool {
        measurement.energy_kwh > 0.0 && measurement.time_slot <= seconds_since_epoch
    }

    pub fn convert_forecast_to_internal_schema(&self, forecast: &ExternalForecast) -> ForecastSchema {
        ForecastSchema {
            area_uuid: forecast.area_uuid.clone(),
            community_uuid: forecast.community_uuid.clone(),
            time_slot: forecast.time_slot,
            creation_time: forecast.creation_time,
            energy_kwh: forecast.energy_kwh,
            confidence: forecast.confidence
        }
    }

    pub fn convert_measurement_to_internal_schema(&self, measurement: &ExternalMeasurement) -> MeasurementSchema {
        MeasurementSchema {
            area_uuid: measurement.area_uuid.clone(),
            community_uuid: measurement.community_uuid.clone(),
            time_slot: measurement.time_slot,
            creation_time: measurement.creation_time,
            energy_kwh: measurement.energy_kwh
        }
    }

    pub async fn get_or_create_market_topology(&self, topology: ExternalCommunityTopology, time_slot: u64) -> Option<MarketTopologySchema> {
        let community_market_url = self.internal_community_market_url.clone() +
            "?community_uuid=" + topology.community_uuid.as_str() +
            "&time_slot=" + time_slot.to_string().as_str();
        let response = self.client.get(community_market_url).send().await;
        match response {
            Ok(resp) => {
                Some(resp.json::<MarketTopologySchema>().await.unwrap())
            }
            Err(_) => {
                let new_market = MarketTopologySchema {
                    community_name: topology.community_name.clone(),
                    community_uuid: topology.community_uuid.clone(),
                    market_id: h256_to_string(H256::random()),
                    time_slot: time_slot as u32,
                    creation_time: get_current_timestamp_in_secs() as u32,
                    area_uuids: topology.areas.clone().into_iter().map(
                        |area| AreaTopologySchema {
                            area_uuid: area.area_uuid.clone(), name: area.area_name.clone(),
                            area_hash: h256_to_string(H256::random()),
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

