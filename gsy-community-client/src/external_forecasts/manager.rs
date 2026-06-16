use crate::external_forecasts::demand_api::DemandForecastApiConnection;
use chrono::{DateTime, Utc};
use gsy_offchain_primitives::db_api_schema::market::{AssetType, MarketTopologySchema};
use gsy_offchain_primitives::db_api_schema::profiles::ForecastSchema;
use tracing::{error, info};

// Communities of the AEM (Swiss) pilot site that are served by the GD/LIC demand
// forecasting API.
pub const AEM_PILOT_COMMUNITIES: [&str; 2] = ["LugaggiaInnovationCommunity", "GaramèDistrict"];

// The API reports p5 / p95 quantiles alongside each forecast, i.e. a 90% confidence interval.
const DEMAND_FORECAST_CONFIDENCE: f64 = 0.9;

#[derive(Clone)]
pub struct DemandForecastsManager {
    demand_forecast_api: DemandForecastApiConnection,
}

impl DemandForecastsManager {
    pub fn new() -> Self {
        DemandForecastsManager {
            demand_forecast_api: DemandForecastApiConnection::new(),
        }
    }

    // The demand forecaster only serves the metered demand of community members. Batteries
    // (e.g. LIC02SM) and generation assets (PV) are not supported by this API.
    fn is_forecastable_meter(area_type: &AssetType) -> bool {
        matches!(area_type, AssetType::SMART_METER | AssetType::GRID_METER)
    }

    pub async fn fetch_community_forecasts(
        &self,
        market: &MarketTopologySchema,
        start_timestamp: u64,
    ) -> Vec<ForecastSchema> {
        if !AEM_PILOT_COMMUNITIES.contains(&market.community_name.as_str()) {
            return vec![];
        }
        let start_time = DateTime::<Utc>::from_timestamp(start_timestamp as i64, 0)
            .expect("valid unix timestamp");

        let mut forecasts: Vec<ForecastSchema> = vec![];
        for area in market.community_areas.iter() {
            if !Self::is_forecastable_meter(&area.area_type) {
                continue;
            }
            match self
                .demand_forecast_api
                .fetch(&area.name, &market.community_name, start_time)
                .await
            {
                Ok(response) => {
                    info!(
                        "Fetched {} demand forecast points for meter {} of community {}",
                        response.demand_forecast.len(),
                        area.name,
                        market.community_name
                    );
                    for point in response.demand_forecast {
                        forecasts.push(ForecastSchema {
                            area_uuid: area.area_uuid.clone(),
                            area_hash: area.area_hash.clone(),
                            community_uuid: market.community_uuid.clone(),
                            time_slot: point.timestamp.timestamp() as u64,
                            creation_time: Utc::now().timestamp() as u64,
                            energy_kwh: point.forecast,
                            confidence: DEMAND_FORECAST_CONFIDENCE,
                        });
                    }
                }
                Err(e) => error!(
                    "Failed to fetch demand forecast for meter {} of community {}: {}",
                    area.name, market.community_name, e
                ),
            }
        }
        forecasts
    }
}
