use crate::external_api::{ExternalCommunityTopology, ExternalForecast, ExternalMeasurement};
use crate::time_utils::get_current_timestamp_in_secs;
use blake2_rfc::blake2b::blake2b;
use gsy_offchain_primitives::db_api_schema::market::MarketSchema;
use gsy_offchain_primitives::db_api_schema::profiles::{
    FlowDirection, ForecastSchema, MeasurementPointSchema, MeasurementPointType, MeasurementSchema,
    TimeseriesSchema,
};
use gsy_offchain_primitives::utils::timestamp_to_string_with_padding;
use gsy_offchain_primitives::{MarketType, MatchingAlgorithm};
use reqwest::Client;
use std::env;
use tracing::info;

fn generate_market_id(market_type: MarketType, delivery_timestamp: u64) -> String {
    let mut buffer = Vec::new();
    // Method will be different in DD-398
    // buffer.extend_from_slice(market_type.as_str().as_bytes());
    buffer.extend_from_slice(&delivery_timestamp.to_be_bytes());
    let digest = blake2b(16, &[], &buffer);
    format!("0x{}", ethers::utils::hex::encode(digest.as_bytes()))
}

#[derive(Clone)]
pub struct AreaMarketInfoAdapter {
    client: Client,
    internal_markets_url: String,
    internal_measurement_points_url: String,
    internal_timeseries_url: String,
}

impl AreaMarketInfoAdapter {
    pub fn new(host: Option<String>) -> Self {
        let hostname = env::var("OFFCHAIN_STORAGE_URL")
            .ok()
            .or(host)
            .unwrap_or_else(|| "http://gsy-offchain-storage:8080".to_string());
        let base_url = hostname.trim_end_matches('/').to_string();
        AreaMarketInfoAdapter {
            client: Client::new(),
            internal_markets_url: base_url.clone() + "/markets",
            internal_measurement_points_url: base_url.clone() + "/measurement-points",
            internal_timeseries_url: base_url + "/timeseries",
        }
    }

    pub async fn forward_forecast(
        &self,
        forecasts: Vec<ForecastSchema>,
    ) -> Result<(), reqwest::Error> {
        let measurement_points = forecasts
            .iter()
            .map(forecast_measurement_point)
            .collect::<Vec<_>>();
        let timeseries = forecasts
            .iter()
            .map(|forecast| TimeseriesSchema {
                measurement_point: profile_measurement_id(
                    MeasurementPointType::Forecast,
                    forecast.community_uuid.as_str(),
                    forecast.area_uuid.as_str(),
                ),
                timestamp: timestamp_to_string_with_padding(forecast.time_slot),
                value: forecast.energy_kwh,
            })
            .collect::<Vec<_>>();

        self.client
            .post(&self.internal_measurement_points_url)
            .json(&measurement_points)
            .send()
            .await?
            .error_for_status()?;
        self.client
            .post(&self.internal_timeseries_url)
            .json(&timeseries)
            .send()
            .await?
            .error_for_status()?;
        Ok(())
    }

    pub async fn forward_measurement(
        &self,
        measurements: Vec<MeasurementSchema>,
    ) -> Result<(), reqwest::Error> {
        let measurement_points = measurements
            .iter()
            .map(measurement_point)
            .collect::<Vec<_>>();
        let timeseries = measurements
            .iter()
            .map(|measurement| TimeseriesSchema {
                measurement_point: profile_measurement_id(
                    MeasurementPointType::Measurement,
                    measurement.community_uuid.as_str(),
                    measurement.area_uuid.as_str(),
                ),
                timestamp: timestamp_to_string_with_padding(measurement.time_slot),
                value: measurement.energy_kwh,
            })
            .collect::<Vec<_>>();

        self.client
            .post(&self.internal_measurement_points_url)
            .json(&measurement_points)
            .send()
            .await?
            .error_for_status()?;
        self.client
            .post(&self.internal_timeseries_url)
            .json(&timeseries)
            .send()
            .await?
            .error_for_status()?;
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
        _area_hash: String,
    ) -> ForecastSchema {
        ForecastSchema {
            area_uuid: forecast.area_uuid.clone(),
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
        _area_hash: String,
    ) -> MeasurementSchema {
        MeasurementSchema {
            area_uuid: measurement.area_uuid.clone(),
            community_uuid: measurement.community_uuid.clone(),
            time_slot: measurement.time_slot,
            creation_time: measurement.creation_time,
            energy_kwh: measurement.energy_kwh,
        }
    }

    pub async fn get_or_create_market_topology(
        &self,
        topology: ExternalCommunityTopology,
        time_slot: u64,
    ) -> Option<MarketSchema> {
        let creation_time = get_current_timestamp_in_secs();
        let market_schema = MarketSchema {
            market_id: generate_market_id(MarketType::Spot, time_slot),
            community_id: topology.community_uuid,
            opening_time: timestamp_to_string_with_padding(creation_time),
            closing_time: timestamp_to_string_with_padding(time_slot),
            delivery_start_time: timestamp_to_string_with_padding(time_slot),
            delivery_end_time: timestamp_to_string_with_padding(time_slot + 900),
            market_type: MarketType::Spot,
            matching_algorithm:MatchingAlgorithm::PayAsBid,
            created_at: timestamp_to_string_with_padding(creation_time),
        };

        match self
            .client
            .post(&self.internal_markets_url)
            .json(&market_schema)
            .send()
            .await
        {
            Ok(response) if response.status().is_success() => Some(market_schema),
            Ok(response) => {
                info!("Market upsert failed with status {}", response.status());
                None
            }
            Err(error) => {
                info!("Market upsert failed with error: {}", error);
                None
            }
        }
    }
}

fn profile_measurement_id(
    point_type: MeasurementPointType,
    community_uuid: &str,
    area_uuid: &str,
) -> String {
    let prefix = match point_type {
        MeasurementPointType::Measurement => "measurement",
        MeasurementPointType::Forecast => "forecast",
    };
    format!("{prefix}:{community_uuid}:{area_uuid}")
}

fn forecast_measurement_point(forecast: &ForecastSchema) -> MeasurementPointSchema {
    MeasurementPointSchema {
        point_type: MeasurementPointType::Forecast,
        measurement_id: profile_measurement_id(
            MeasurementPointType::Forecast,
            forecast.community_uuid.as_str(),
            forecast.area_uuid.as_str(),
        ),
        property_measured: "energy_forecast".to_string(),
        unit: "kWh".to_string(),
        direction: flow_direction(forecast.energy_kwh),
        energy_accumulated: false,
        time_resolution: "PT15M".to_string(),
        phase: 0,
        asset_name: forecast.area_uuid.clone(),
        datasource_name: Some(forecast.community_uuid.clone()),
    }
}

fn measurement_point(measurement: &MeasurementSchema) -> MeasurementPointSchema {
    MeasurementPointSchema {
        point_type: MeasurementPointType::Measurement,
        measurement_id: profile_measurement_id(
            MeasurementPointType::Measurement,
            measurement.community_uuid.as_str(),
            measurement.area_uuid.as_str(),
        ),
        property_measured: "energy_measured".to_string(),
        unit: "kWh".to_string(),
        direction: flow_direction(measurement.energy_kwh),
        energy_accumulated: false,
        time_resolution: "PT15M".to_string(),
        phase: 0,
        asset_name: measurement.area_uuid.clone(),
        datasource_name: Some(measurement.community_uuid.clone()),
    }
}

fn flow_direction(value: f64) -> FlowDirection {
    if value >= 0.0 {
        FlowDirection::Import
    } else {
        FlowDirection::Export
    }
}
