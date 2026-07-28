use crate::external_forecasts::ForecastApiError;
use crate::external_forecasts::demand_api::{DemandForecastApiConnection, DemandForecaster};
use crate::external_forecasts::pv_api::{PvForecastApiConnection, PvForecastPoint};
use crate::external_forecasts::pv_pricing::{PvCommitmentConfig, commitment_from_point};
use chrono::{DateTime, Utc};
use gsy_offchain_primitives::db_api_schema::market::{
    AreaTopologySchema, AssetType, MarketTopologySchema,
};
use gsy_offchain_primitives::db_api_schema::profiles::ForecastSchema;
use std::sync::Arc;
use tracing::{error, info};

/// The FEDECOM forecaster `site` for a meter, selected by the meter-id prefix.
///
/// The ontology now groups meters under generic `Pilot#` community names, but the
/// forecaster APIs are keyed by these fixed site names, so the site is derived from
/// the meter id rather than the ontology community name. Meters whose id has no known
/// prefix are not served by any forecaster and are skipped.
fn forecaster_site(meter_name: &str) -> Option<&'static str> {
    if meter_name.starts_with("LIC") {
        Some("LugaggiaInnovationCommunity")
    } else if meter_name.starts_with("GD") {
        Some("GaramèDistrict")
    } else if meter_name.starts_with("AIC") {
        Some("ArenaInnovationCommunity")
    } else {
        None
    }
}

// The API reports p5 / p95 quantiles alongside each forecast, i.e. a 90% confidence interval.
const DEMAND_FORECAST_CONFIDENCE: f64 = 0.9;

// Meters that must never be forecast even though the ontology classifies them as a
// forecastable meter type. LIC02SM is a battery mislabelled as a SmartMeter; add further
// mislabelled assets here as they are discovered.
const EXCLUDED_METERS: [&str; 1] = ["LIC02SM"];

#[derive(Clone)]
pub struct ForecastsManager {
    demand_forecast_api: Arc<dyn DemandForecaster + Send + Sync>,
    // PV uses its own inherent `fetch` (distinct response shape and energy-sign /
    // confidence semantics), so it is held as a concrete type rather than behind the
    // shared `DemandForecaster` trait.
    pv_forecast_api: Arc<PvForecastApiConnection>,
}

impl ForecastsManager {
    pub fn new() -> Self {
        ForecastsManager {
            demand_forecast_api: Arc::new(DemandForecastApiConnection::new()),
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

    pub fn is_pv_asset(area_name: &str, area_type: &AssetType) -> bool {
        matches!(area_type, AssetType::PV) && !EXCLUDED_METERS.contains(&area_name)
    }

    /// Thin wrapper around [`Self::fetch_area_set_forecasts`] for the market-shaped call
    /// site (the publish path and existing tests): unpacks the market's own identity and
    /// area list.
    pub async fn fetch_community_forecasts(
        &self,
        market: &MarketTopologySchema,
        start_timestamp: u64,
    ) -> Vec<ForecastSchema> {
        self.fetch_area_set_forecasts(
            &market.community_uuid,
            &market.community_name,
            &market.community_areas,
            start_timestamp,
        )
        .await
    }

    /// Fetch PV + demand forecasts for an explicit `(community_uuid, areas)` pair, without
    /// requiring a per-timeslot `MarketTopologySchema`. This is what the day-ahead ingestion
    /// loop calls directly with deterministic ids derived straight from the ontology, since
    /// ingestion has no per-timeslot market to unpack one from.
    pub async fn fetch_area_set_forecasts(
        &self,
        community_uuid: &str,
        community_name: &str,
        areas: &[AreaTopologySchema],
        start_timestamp: u64,
    ) -> Vec<ForecastSchema> {
        let start_time = DateTime::<Utc>::from_timestamp(start_timestamp as i64, 0)
            .expect("valid unix timestamp");

        // PV production forecasts (each PV asset routed to its forecaster site by meter id).
        let mut forecasts = self
            .fetch_pv_forecasts(community_uuid, areas, start_time)
            .await;

        // Demand forecasts for each metered community member. The forecaster `site` is
        // derived from the meter id (LIC*/GD*/AIC*) rather than the ontology community
        // name, which is now a generic `Pilot#` label. Meters with no known site are skipped.
        for area in areas.iter() {
            if !Self::is_forecastable_meter(&area.name, &area.area_type) {
                continue;
            }
            let Some(site) = forecaster_site(&area.name) else {
                continue;
            };
            match self
                .demand_forecast_api
                .fetch(&area.name, site, start_time)
                .await
            {
                Ok(response) => {
                    info!(
                        "Fetched {} demand forecast points for meter {} (site {}, community {})",
                        response.demand_forecast.len(),
                        area.name,
                        site,
                        community_name
                    );
                    for point in response.demand_forecast {
                        forecasts.push(ForecastSchema {
                            area_uuid: area.area_uuid.clone(),
                            area_hash: area.area_hash.clone(),
                            community_uuid: community_uuid.to_string(),
                            time_slot: point.timestamp.timestamp() as u64,
                            creation_time: Utc::now().timestamp() as u64,
                            // The demand forecaster already reports energy in kWh.
                            energy_kwh: point.forecast,
                            confidence: DEMAND_FORECAST_CONFIDENCE,
                        });
                    }
                }
                Err(ForecastApiError::Http(e)) => error!(
                    "HTTP error fetching demand forecast for meter {} (site {}): {}",
                    area.name, site, e
                ),
                Err(ForecastApiError::Api(msg)) => error!(
                    "API-reported error fetching demand forecast for meter {} (site {}) \
                     (skipping — server-side issue): {}",
                    area.name, site, msg
                ),
            }
        }
        forecasts
    }

    // Build a PV `ForecastSchema` for one forecast point. Returns `None` for night slots
    // (zero committed energy → post no order). Factored out as a pure function so the
    // point → schema mapping (production sign, per-slot confidence, unix time_slot) is
    // unit-testable offline without a live PV endpoint.
    pub fn pv_forecast_schema_from_point(
        point: &PvForecastPoint,
        area: &AreaTopologySchema,
        community_uuid: &str,
        cfg: &PvCommitmentConfig,
    ) -> Option<ForecastSchema> {
        let commitment = commitment_from_point(point, cfg);
        // Night slots commit zero energy: post no order.
        if commitment.energy_kwh == 0.0 {
            return None;
        }
        Some(ForecastSchema {
            area_uuid: area.area_uuid.clone(),
            area_hash: area.area_hash.clone(),
            community_uuid: community_uuid.to_string(),
            time_slot: point.timestamp_utc().timestamp() as u64,
            creation_time: Utc::now().timestamp() as u64,
            // Negative energy marks a production offer (see node_connector/orders.rs); the
            // magnitude is the confidence-adjusted committed quantity from pv_pricing.
            energy_kwh: -commitment.energy_kwh,
            // The real per-slot confidence, not the fixed demand constant.
            confidence: commitment.confidence,
        })
    }

    // Fetch PV production forecasts for every PV asset in the market topology and map each
    // forecast point to a production (negative-energy) `ForecastSchema`. Mirrors the demand
    // path: the meter id is the area name and the site is the community name, both taken from
    // the ontology-driven topology. One failing PV meter is logged and skipped so it does not
    // sink the whole fetch.
    async fn fetch_pv_forecasts(
        &self,
        community_uuid: &str,
        areas: &[AreaTopologySchema],
        start_time: DateTime<Utc>,
    ) -> Vec<ForecastSchema> {
        let cfg = PvCommitmentConfig::from_constants();
        let mut forecasts: Vec<ForecastSchema> = vec![];
        for area in areas.iter() {
            if !Self::is_pv_asset(&area.name, &area.area_type) {
                continue;
            }
            let Some(site) = forecaster_site(&area.name) else {
                continue;
            };
            match self
                .pv_forecast_api
                .fetch(&area.name, site, start_time)
                .await
            {
                Ok(response) => {
                    info!(
                        "Fetched {} PV forecast points for meter {} (site {})",
                        response.data.pv_forecasts.len(),
                        area.name,
                        site
                    );
                    for point in &response.data.pv_forecasts {
                        if let Some(schema) =
                            Self::pv_forecast_schema_from_point(point, area, community_uuid, &cfg)
                        {
                            forecasts.push(schema);
                        }
                    }
                }
                Err(ForecastApiError::Http(e)) => error!(
                    "HTTP error fetching PV forecast for meter {} (site {}): {}",
                    area.name, site, e
                ),
                Err(ForecastApiError::Api(msg)) => error!(
                    "API-reported error fetching PV forecast for meter {} (site {}) \
                     (skipping — server-side issue): {}",
                    area.name, site, msg
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
        assert!(ForecastsManager::is_forecastable_meter(
            "LIC08SM",
            &AssetType::SMART_METER
        ));
        assert!(ForecastsManager::is_forecastable_meter(
            "LIC00SGIM",
            &AssetType::GRID_METER
        ));
    }

    #[test]
    fn non_meter_types_are_rejected() {
        assert!(!ForecastsManager::is_forecastable_meter(
            "LIC03PV",
            &AssetType::PV
        ));
        assert!(!ForecastsManager::is_forecastable_meter(
            "LIC02DBATT",
            &AssetType::BATTERY
        ));
    }

    #[test]
    fn excluded_meters_are_rejected_even_when_typed_as_meter() {
        // LIC02SM is a battery the ontology mislabels as SMART_METER; the name guard must
        // exclude it regardless of type.
        assert!(!ForecastsManager::is_forecastable_meter(
            "LIC02SM",
            &AssetType::SMART_METER
        ));
    }
}
