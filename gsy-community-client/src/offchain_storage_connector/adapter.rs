use crate::external_api::{ExternalForecast, ExternalMeasurement};
use crate::topology::ExternalCommunityTopology;
use crate::time_utils::get_current_timestamp_in_secs;
use blake2_rfc::blake2b::blake2b;
use gsy_offchain_primitives::MarketType;
use gsy_offchain_primitives::db_api_schema::market::{AreaTopologySchema, MarketTopologySchema};
use gsy_offchain_primitives::db_api_schema::profiles::{ForecastSchema, MeasurementSchema};
use gsy_offchain_primitives::utils::h256_to_string;
use reqwest::Client;
use subxt::utils::H256;
use tracing::{info, error};
use uuid::Uuid;

fn generate_market_id(market_type: MarketType, delivery_timestamp: u64) -> H256 {
    let mut buffer = Vec::new();
    buffer.extend_from_slice(market_type.as_str().as_bytes());
    buffer.extend_from_slice(&delivery_timestamp.to_be_bytes());
    H256(
        blake2b(32, &[], &buffer)
            .as_bytes()
            .try_into()
            .expect("hash is 32 bytes"),
    )
}

#[derive(Clone, Debug)]
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
    pub async fn forward_forecast(
        &self,
        forecasts: Vec<ForecastSchema>,
    ) -> Result<(), reqwest::Error> {
        self.client
            .post(&self.internal_forecast_url)
            .json(&forecasts)
            .send()
            .await?;
        Ok(())
    }

    // Function to forward the measurement data to internal API
    pub async fn forward_measurement(
        &self,
        measurements: Vec<MeasurementSchema>,
    ) -> Result<(), reqwest::Error> {
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

    pub fn validate_measurement(
        &self,
        measurement: &MeasurementSchema,
        seconds_since_epoch: u64,
    ) -> bool {
        measurement.energy_kwh > 0.0 && measurement.time_slot <= seconds_since_epoch
    }

    pub fn convert_forecast_to_internal_schema(
        &self,
        forecast: &ExternalForecast,
        area_hash: String,
    ) -> ForecastSchema {
        ForecastSchema {
            area_uuid: forecast.area_uuid.clone(),
            area_hash: area_hash.clone(),
            community_uuid: forecast.community_uuid.clone(),
            time_slot: forecast.time_slot,
            creation_time: forecast.creation_time,
            energy_kwh: forecast.energy_kwh,
            confidence: forecast.confidence,
        }
    }

    pub fn convert_measurement_to_internal_schema(
        &self,
        measurement: &ExternalMeasurement,
        area_hash: String,
    ) -> MeasurementSchema {
        MeasurementSchema {
            area_uuid: measurement.area_uuid.clone(),
            area_hash: area_hash.clone(),
            community_uuid: measurement.community_uuid.clone(),
            time_slot: measurement.time_slot,
            creation_time: measurement.creation_time,
            energy_kwh: measurement.energy_kwh,
        }
    }

    pub async fn get_existing_market_topology(
        &self,
        community_market_url: String,
    ) -> Vec<MarketTopologySchema> {
        let response = match self.client.get(community_market_url).send().await {
            Ok(resp) if resp.status().is_success() => resp,
            _ => return vec![],
        };
        response.json::<Vec<MarketTopologySchema>>().await.unwrap_or_else(|err| {
            error!("Failed to deserialize market topology response: {:?}", err);
            vec![]
        })
    }

    pub async fn get_or_create_market_topology(
        &self,
        topology: Vec<ExternalCommunityTopology>,
        time_slot: u64,
    ) -> Vec<MarketTopologySchema> {
        let mut market_topologies: Vec<MarketTopologySchema> = vec![];
        for community_topology in topology {
            let community_market_url = self.internal_community_market_url.clone()
                + "?community_uuid="
                + community_topology.community_name.as_str()
                + "&time_slot="
                + time_slot.to_string().as_str();
            let market_topology_res = self
                .get_existing_market_topology(community_market_url)
                .await;
            if !market_topology_res.is_empty() {
                market_topologies.push(market_topology_res.get(0).unwrap().clone());
            }
            else {
                let new_market = MarketTopologySchema {
                    community_name: community_topology.community_name.clone(),
                    community_uuid: Uuid::new_v4().to_string(),
                    market_id: h256_to_string(generate_market_id(MarketType::Spot, time_slot)),
                    time_slot: time_slot as u32,
                    creation_time: get_current_timestamp_in_secs() as u32,
                    community_areas: community_topology
                        .areas
                        .clone()
                        .into_iter()
                        .map(|area| AreaTopologySchema {
                            area_uuid: Uuid::new_v4().to_string(),
                            area_type: area.area_type.clone(),
                            name: area.area_name.clone(),
                            area_hash: h256_to_string(H256::random()),
                        })
                        .collect(),
                };
                let topology_resp = self
                    .client
                    .post(&self.internal_topology_url)
                    .json(&new_market)
                    .send()
                    .await;

                match topology_resp {
                    Ok(_) => {
                        market_topologies.push(new_market.clone())
                    },
                    Err(error) => {
                        info!(
                            "New topology creation failed with error: {}",
                            error.to_string()
                        );
                    }
                }
            }
        }
        market_topologies
    }
}
