use primitives::db_api_schema::grid_topology::FacilitySchema;
use primitives::db_api_schema::profiles::{
    FlowDirection, ForecastSchema, MeasurementPointSchema, MeasurementPointType, MeasurementSchema,
    TimeseriesSchema,
};
use primitives::utils::timestamp_to_string_with_padding;
use reqwest::Client;
use std::env;

#[derive(Clone)]
pub struct AreaMarketInfoAdapter {
    client: Client,
    internal_measurement_points_url: String,
    internal_timeseries_url: String,
    internal_facilities_url: String,
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
            internal_measurement_points_url: base_url.clone() + "/measurement-points",
            internal_timeseries_url: base_url.clone() + "/timeseries",
            internal_facilities_url: base_url.clone() + "/facilities",
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
                    forecast.facility_id.as_str(),
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
                    measurement.facility_id.as_str(),
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

    pub async fn forward_facilities(&self, facilities: FacilitySchema) -> anyhow::Result<()> {
        let resp = self
            .client
            .post(&self.internal_facilities_url)
            .json(&facilities)
            .send()
            .await?;

        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("forward_facilities failed ({status}): {body}");
        }
        Ok(())
    }
}

fn profile_measurement_id(
    point_type: MeasurementPointType,
    community_uuid: &str,
    facility_id: &str,
) -> String {
    let prefix = match point_type {
        MeasurementPointType::Measurement => "measurement",
        MeasurementPointType::Forecast => "forecast",
    };
    format!("{prefix}:{community_uuid}:{facility_id}")
}

fn forecast_measurement_point(forecast: &ForecastSchema) -> MeasurementPointSchema {
    MeasurementPointSchema {
        point_type: MeasurementPointType::Forecast,
        measurement_id: profile_measurement_id(
            MeasurementPointType::Forecast,
            forecast.community_uuid.as_str(),
            forecast.facility_id.as_str(),
        ),
        property_measured: "energy_forecast".to_string(),
        unit: "kWh".to_string(),
        direction: flow_direction(forecast.energy_kwh),
        energy_accumulated: false,
        time_resolution: "PT15M".to_string(),
        phase: 0,
        asset_name: forecast.facility_id.clone(),
        datasource_name: Some(forecast.community_uuid.clone()),
    }
}

fn measurement_point(measurement: &MeasurementSchema) -> MeasurementPointSchema {
    MeasurementPointSchema {
        point_type: MeasurementPointType::Measurement,
        measurement_id: profile_measurement_id(
            MeasurementPointType::Measurement,
            measurement.community_uuid.as_str(),
            measurement.facility_id.as_str(),
        ),
        property_measured: "energy_measured".to_string(),
        unit: "kWh".to_string(),
        direction: flow_direction(measurement.energy_kwh),
        energy_accumulated: false,
        time_resolution: "PT15M".to_string(),
        phase: 0,
        asset_name: measurement.facility_id.clone(),
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
