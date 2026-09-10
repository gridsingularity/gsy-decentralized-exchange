use chrono::NaiveDate;
use gsy_community_client::external_forecasts::manager::ForecastsManager;
use gsy_community_client::external_forecasts::pv_api::PvForecastPoint;
use gsy_community_client::external_forecasts::pv_pricing::PvCommitmentConfig;
use gsy_offchain_primitives::db_api_schema::market::{AreaTopologySchema, AssetType};

#[cfg(test)]
mod tests {
    use super::*;

    /// Config identical to the v1 defaults, held explicitly so the mapping assertions do
    /// not depend on env state.
    fn default_cfg() -> PvCommitmentConfig {
        PvCommitmentConfig {
            risk_aversion: 1.0,
            spread_norm: 1.0,
            min_confidence: 0.1,
            min_forecast_kwh: 0.05,
        }
    }

    fn pv_area() -> AreaTopologySchema {
        AreaTopologySchema {
            area_uuid: "pv-uuid".to_string(),
            name: "LIC03PV".to_string(),
            area_type: AssetType::PV,
            area_hash: "pv-hash".to_string(),
        }
    }

    fn point(watts: f64, p5: Vec<f64>, p95: Vec<f64>) -> PvForecastPoint {
        PvForecastPoint {
            timestamp: NaiveDate::from_ymd_opt(2026, 7, 15)
                .unwrap()
                .and_hms_opt(4, 30, 0)
                .unwrap(),
            pv_forecast: watts,
            p5,
            p95,
        }
    }

    // ---- PV gate --------------------------------------------------------------

    #[test]
    fn pv_asset_type_is_accepted() {
        assert!(ForecastsManager::is_pv_asset("LIC03PV", &AssetType::PV));
    }

    #[test]
    fn non_pv_asset_types_are_rejected() {
        assert!(!ForecastsManager::is_pv_asset(
            "LIC08SM",
            &AssetType::SMART_METER
        ));
        assert!(!ForecastsManager::is_pv_asset(
            "LIC00SGIM",
            &AssetType::GRID_METER
        ));
        assert!(!ForecastsManager::is_pv_asset(
            "LIC02DBATT",
            &AssetType::BATTERY
        ));
    }

    #[test]
    fn excluded_meters_are_rejected_even_when_typed_as_pv() {
        // Mirror the demand path's EXCLUDED_METERS name guard for PV areas too.
        assert!(!ForecastsManager::is_pv_asset(
            "LIC02SM",
            &AssetType::PV
        ));
    }

    // ---- point -> ForecastSchema mapping --------------------------------------

    #[test]
    fn mapping_produces_negative_energy_offer_with_real_confidence() {
        // 12000 W => 3.0 kWh point forecast; p5 8000 W => 2.0 kWh, p95 16000 W => 4.0 kWh.
        let p = point(12000.0, vec![8000.0], vec![16000.0]);
        let schema =
            ForecastsManager::pv_forecast_schema_from_point(&p, &pv_area(), "community-uuid", &default_cfg())
                .expect("daytime slot should produce an offer");

        // risk_aversion = 1.0 commits q5 = 2.0 kWh, emitted as a negative production offer.
        assert!((schema.energy_kwh - (-2.0)).abs() < 1e-9);
        // relative_spread = (4.0 - 2.0) / 3.0 = 0.6667 => confidence = 1 - 0.6667 = 0.3333.
        assert!((schema.confidence - (1.0 / 3.0)).abs() < 1e-9);
        // Confidence is a real per-slot value, not the fixed demand constant of 0.9.
        assert_ne!(schema.confidence, 0.9);
        assert!(schema.confidence >= default_cfg().min_confidence && schema.confidence <= 1.0);
        // Area/community resolution mirrors the demand path.
        assert_eq!(schema.area_uuid, "pv-uuid");
        assert_eq!(schema.area_hash, "pv-hash");
        assert_eq!(schema.community_uuid, "community-uuid");
    }

    #[test]
    fn mapping_time_slot_is_unix_seconds_of_utc_timestamp() {
        let p = point(12000.0, vec![8000.0], vec![16000.0]);
        let schema =
            ForecastsManager::pv_forecast_schema_from_point(&p, &pv_area(), "community-uuid", &default_cfg())
                .unwrap();
        let expected = NaiveDate::from_ymd_opt(2026, 7, 15)
            .unwrap()
            .and_hms_opt(4, 30, 0)
            .unwrap()
            .and_utc()
            .timestamp() as u64;
        assert_eq!(schema.time_slot, expected);
    }

    #[test]
    fn night_slot_is_skipped() {
        // Zero production => zero committed energy => no order.
        let p = point(0.0, vec![0.0], vec![0.0]);
        let schema = ForecastsManager::pv_forecast_schema_from_point(
            &p,
            &pv_area(),
            "community-uuid",
            &default_cfg(),
        );
        assert!(schema.is_none());
    }
}
