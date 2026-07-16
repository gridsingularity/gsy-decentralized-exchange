use crate::external_forecasts::aic_api::{AicForecastApiConnection, aic_meters};
use crate::external_forecasts::demand_api::{DemandForecastApiConnection, DemandForecaster};
use crate::external_forecasts::ForecastApiError;
use crate::external_forecasts::pv_api::PvForecastApiConnection;
use chrono::{DateTime, Utc};
use gsy_offchain_primitives::db_api_schema::market::{AssetType, MarketTopologySchema};
use gsy_offchain_primitives::db_api_schema::profiles::ForecastSchema;
use std::sync::Arc;
use tracing::{error, info};

pub const AEM_PILOT_COMMUNITIES: [&str; 2] = ["LugaggiaInnovationCommunity", "GaramèDistrict"];

pub const AIC_SITE: &str = "AIC";

// The API reports p5 / p95 quantiles alongside each forecast, i.e. a 90% confidence interval.
const DEMAND_FORECAST_CONFIDENCE: f64 = 0.9;

// Meters that must never be forecast even though the ontology classifies them as a
// forecastable meter type. LIC02SM is a battery mislabelled as a SmartMeter; add further
// mislabelled assets here as they are discovered.
const EXCLUDED_METERS: [&str; 1] = ["LIC02SM"];

#[derive(Clone)]
pub struct DemandForecastsManager {
    demand_forecast_api: Arc<dyn DemandForecaster + Send + Sync>,
    aic_forecast_api: Arc<dyn DemandForecaster + Send + Sync>,
    // PV uses its own inherent `fetch` (distinct response shape and energy-sign /
    // confidence semantics), so it is held as a concrete type rather than behind the
    // shared `DemandForecaster` trait. Wired into the forecast pipeline in a later step.
    #[allow(dead_code)]
    pv_forecast_api: Arc<PvForecastApiConnection>,
}

impl DemandForecastsManager {
    pub fn new() -> Self {
        DemandForecastsManager {
            demand_forecast_api: Arc::new(DemandForecastApiConnection::new()),
            aic_forecast_api: Arc::new(AicForecastApiConnection::new()),
            pv_forecast_api: Arc::new(PvForecastApiConnection::new()),
        }
    }

    // The demand forecaster only serves the metered demand of community members. Batteries
    // and generation assets (PV) are not supported by this API. Assets in EXCLUDED_METERS are
    // also excluded explicitly by name because the ontology mislabels them (e.g. LIC02SM is a
    // battery reported as a SmartMeter) — type-based filtering alone is insufficient for them.
    fn is_forecastable_meter(area_name: &str, area_type: &AssetType) -> bool {
        matches!(area_type, AssetType::SMART_METER | AssetType::GRID_METER)
            && !EXCLUDED_METERS.contains(&area_name)
    }

    pub async fn fetch_community_forecasts(
        &self,
        market: &MarketTopologySchema,
        start_timestamp: u64,
    ) -> Vec<ForecastSchema> {
        let start_time = DateTime::<Utc>::from_timestamp(start_timestamp as i64, 0)
            .expect("valid unix timestamp");

        // The temporary AIC back-end is selected by site name; it does not go through the AEM
        // pilot community gate or the ontology-driven meter list (see T8 / B3).
        if market.community_name == AIC_SITE {
            return self.fetch_aic_forecasts(market, start_time).await;
        }

        if !AEM_PILOT_COMMUNITIES.contains(&market.community_name.as_str()) {
            return vec![];
        }

        let mut forecasts: Vec<ForecastSchema> = vec![];
        for area in market.community_areas.iter() {
            if !Self::is_forecastable_meter(&area.name, &area.area_type) {
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
                            // The demand forecaster already reports energy in kWh.
                            energy_kwh: point.forecast,
                            confidence: DEMAND_FORECAST_CONFIDENCE,
                        });
                    }
                }
                Err(ForecastApiError::Http(e)) => error!(
                    "HTTP error fetching demand forecast for meter {} of community {}: {}",
                    area.name, market.community_name, e
                ),
                Err(ForecastApiError::Api(msg)) => error!(
                    "API-reported error fetching demand forecast for meter {} of community {} \
                     (skipping — server-side issue): {}",
                    area.name, market.community_name, msg
                ),
            }
        }
        forecasts
    }

    // TODO: finalize AIC endpoint URL. The response schema is assumed to
    // match the GD/LIC demand forecaster (see aic_api.rs).
    async fn fetch_aic_forecasts(
        &self,
        market: &MarketTopologySchema,
        start_time: DateTime<Utc>,
    ) -> Vec<ForecastSchema> {
        let mut forecasts: Vec<ForecastSchema> = vec![];
        for meter in aic_meters() {
            match self
                .aic_forecast_api
                .fetch(&meter, AIC_SITE, start_time)
                .await
            {
                Ok(response) => {
                    info!(
                        "Fetched {} AIC demand forecast points for meter {}",
                        response.demand_forecast.len(),
                        meter
                    );
                    // TODO: resolve area_uuid / area_hash for AIC meters. The AIC forecaster
                    // is not backed by the ontology topology, so the meter -> area mapping is
                    // not yet defined.
                    let area = market.community_areas.iter().find(|a| a.name == meter);
                    for point in response.demand_forecast {
                        forecasts.push(ForecastSchema {
                            area_uuid: area.map(|a| a.area_uuid.clone()).unwrap_or_default(),
                            area_hash: area.map(|a| a.area_hash.clone()).unwrap_or_default(),
                            community_uuid: market.community_uuid.clone(),
                            time_slot: point.timestamp.timestamp() as u64,
                            creation_time: Utc::now().timestamp() as u64,
                            energy_kwh: point.forecast,
                            confidence: DEMAND_FORECAST_CONFIDENCE,
                        });
                    }
                }
                Err(ForecastApiError::Http(e)) => error!(
                    "HTTP error fetching AIC demand forecast for meter {}: {}",
                    meter, e
                ),
                Err(ForecastApiError::Api(msg)) => error!(
                    "API-reported error fetching AIC demand forecast for meter {} \
                     (skipping — server-side issue): {}",
                    meter, msg
                ),
            }
        }
        forecasts
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn forecastable_meter_types_are_accepted() {
        assert!(DemandForecastsManager::is_forecastable_meter(
            "LIC08SM",
            &AssetType::SMART_METER
        ));
        assert!(DemandForecastsManager::is_forecastable_meter(
            "LIC00SGIM",
            &AssetType::GRID_METER
        ));
    }

    #[test]
    fn non_meter_types_are_rejected() {
        assert!(!DemandForecastsManager::is_forecastable_meter(
            "LIC03PV",
            &AssetType::PV
        ));
        assert!(!DemandForecastsManager::is_forecastable_meter(
            "LIC02DBATT",
            &AssetType::BATTERY
        ));
    }

    #[test]
    fn excluded_meters_are_rejected_even_when_typed_as_meter() {
        // LIC02SM is a battery the ontology mislabels as SMART_METER; the name guard must
        // exclude it regardless of type.
        assert!(!DemandForecastsManager::is_forecastable_meter(
            "LIC02SM",
            &AssetType::SMART_METER
        ));
    }
}
