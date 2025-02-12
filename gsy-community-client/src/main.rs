use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::{error, info};
use std::time::Duration;
use tokio::time::sleep;
use gsy_offchain_primitives::db_api_schema::profiles::{MeasurementSchema, ForecastSchema};


// Struct for forecast data
#[derive(Serialize, Deserialize, Debug)]
struct Forecast {
	asset_uuid: String,
	community_uuid: String,
	time_slot: u64,
	creation_time: u64,
	energy_kwh: f64,
	confidence: f64
}

// Struct for measurement data
#[derive(Serialize, Deserialize, Debug)]
struct Measurement {
	asset_uuid: String,
	community_uuid: String,
	time_slot: u64,
	creation_time: u64,
	energy_kwh: f64,
}


#[derive(Clone)]
struct AppState {
	client: Client,
	forecast_url: String,
	measurements_url: String,
	internal_forecast_url: String,
	internal_measurements_url: String,
}

impl AppState {
	fn new() -> Self {
		AppState {
			client: Client::new(),
			forecast_url: "http://localhost:8000/forecasts".to_string(),
			measurements_url: "http://localhost:8000/measurements".to_string(),
			internal_forecast_url: "http://gsy-orderbook:8080/internal/forecasts".to_string(),
			internal_measurements_url: "http://gsy-orderbook:8080/internal/measurements".to_string(),
		}
	}

	// Function to fetch an array of forecast data
	async fn fetch_forecasts(&self) -> Result<Vec<Forecast>, reqwest::Error> {
		let response = self.client.get(&self.forecast_url).send().await?;
		response.json::<Vec<Forecast>>().await
	}

	// Function to fetch an array of measurement data
	async fn fetch_measurements(&self) -> Result<Vec<Measurement>, reqwest::Error> {
		let response = self.client.get(&self.measurements_url).send().await?;
		response.json::<Vec<Measurement>>().await
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

	fn convert_forecast_to_internal_schema(&self, forecast: &Forecast) -> ForecastSchema {
		ForecastSchema {
			area_uuid: forecast.asset_uuid.clone(),
			community_uuid: forecast.community_uuid.clone(),
			time_slot: forecast.time_slot,
			creation_time: forecast.creation_time,
			energy_kwh: forecast.energy_kwh,
			confidence: forecast.confidence
		}
	}

	fn convert_measurement_to_internal_schema(&self, measurement: &Measurement) -> MeasurementSchema {
		MeasurementSchema {
			area_uuid: measurement.asset_uuid.clone(),
			community_uuid: measurement.community_uuid.clone(),
			time_slot: measurement.time_slot,
			creation_time: measurement.creation_time,
			energy_kwh: measurement.energy_kwh
		}
	}
}


async fn poll_and_forward(app_state: AppState) {
	loop {
		let seconds_since_epoch = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
		// Fetch and forward forecasts
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
