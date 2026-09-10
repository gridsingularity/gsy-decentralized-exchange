use chrono::{TimeZone, Utc};
use gsy_community_client::external_forecasts::demand_api::{
    DemandForecastApiConnection, DemandForecastResponse,
};
use gsy_community_client::external_forecasts::manager::ForecastsManager;
use gsy_offchain_primitives::db_api_schema::market::{
    AreaTopologySchema, AssetType, MarketTopologySchema,
};

#[cfg(test)]
mod tests {
    use super::*;

    fn create_market(community_name: &str, areas: Vec<AreaTopologySchema>) -> MarketTopologySchema {
        MarketTopologySchema {
            community_name: community_name.to_string(),
            community_uuid: "community-uuid".to_string(),
            market_id: "market-id".to_string(),
            time_slot: 1747843200,
            creation_time: 1747843200,
            community_areas: areas,
        }
    }

    #[test]
    fn test_demand_forecast_response_deserialization_works() {
        let response_json = r#"{
            "meter": "LIC08SM",
            "start_time": "2026-05-21T16:15:00+00:00",
            "demand_forecast": [
                {"timestamp": "2026-05-21T16:15:00+00:00", "forecast": 0.199, "p5": 0.169, "p95": 0.199},
                {"timestamp": "2026-05-21T16:30:00+00:00", "forecast": 0.192, "p5": 0.162, "p95": 0.192}
            ]
        }"#;
        let response: DemandForecastResponse = serde_json::from_str(response_json).unwrap();
        assert_eq!(response.meter, "LIC08SM");
        assert_eq!(response.demand_forecast.len(), 2);
        assert_eq!(
            response.demand_forecast[0].timestamp,
            Utc.with_ymd_and_hms(2026, 5, 21, 16, 15, 0).unwrap()
        );
        assert_eq!(response.demand_forecast[0].forecast, 0.199);
        assert_eq!(response.demand_forecast[1].p5, 0.162);
        assert_eq!(response.demand_forecast[1].p95, 0.192);
    }

    #[tokio::test]
    async fn test_fetch_demand_forecast_from_api_works() {
        let api = DemandForecastApiConnection::new();
        let start_time = Utc.with_ymd_and_hms(2026, 5, 21, 16, 15, 0).unwrap();
        let response = api
            .fetch("LIC08SM", "LugaggiaInnovationCommunity", start_time)
            .await
            .unwrap();
        assert_eq!(response.meter, "LIC08SM");
        assert_eq!(response.start_time, start_time);
        assert!(!response.demand_forecast.is_empty());
        assert_eq!(response.demand_forecast[0].timestamp, start_time);
    }

    #[tokio::test]
    async fn test_fetch_community_forecasts_ignores_non_aem_pilot_communities() {
        let manager = ForecastsManager::new();
        let market = create_market(
            "ENBRO_Community",
            vec![AreaTopologySchema {
                area_uuid: "area-uuid".to_string(),
                name: "ENBRO01SM".to_string(),
                area_type: AssetType::SMART_METER,
                area_hash: "area-hash".to_string(),
            }],
        );
        let forecasts = manager
            .fetch_community_forecasts(&market, 1747843200)
            .await;
        assert!(forecasts.is_empty());
    }

    /// LIC02SM is excluded by *name*, even when the ontology mislabels it as SMART_METER.
    /// LIC03PV is excluded by *type* (PV is not a forecastable meter type).
    #[tokio::test]
    async fn test_fetch_community_forecasts_ignores_excluded_assets() {
        let manager = ForecastsManager::new();
        let market = create_market(
            "LugaggiaInnovationCommunity",
            vec![
                // LIC02SM is a battery in practice; the ontology may classify it as
                // SMART_METER.  The name-based guard must exclude it regardless of type.
                AreaTopologySchema {
                    area_uuid: "battery-uuid".to_string(),
                    name: "LIC02SM".to_string(),
                    area_type: AssetType::SMART_METER,
                    area_hash: "battery-hash".to_string(),
                },
                AreaTopologySchema {
                    area_uuid: "pv-uuid".to_string(),
                    name: "LIC03PV".to_string(),
                    area_type: AssetType::PV,
                    area_hash: "pv-hash".to_string(),
                },
            ],
        );
        let forecasts = manager
            .fetch_community_forecasts(&market, 1747843200)
            .await;
        assert!(forecasts.is_empty());
    }

    #[tokio::test]
    async fn test_fetch_community_forecasts_converts_to_internal_schema() {
        let manager = ForecastsManager::new();
        let start_timestamp = Utc
            .with_ymd_and_hms(2026, 5, 21, 16, 15, 0)
            .unwrap()
            .timestamp() as u64;
        let market = create_market(
            "LugaggiaInnovationCommunity",
            vec![AreaTopologySchema {
                area_uuid: "area-uuid".to_string(),
                name: "LIC08SM".to_string(),
                area_type: AssetType::SMART_METER,
                area_hash: "area-hash".to_string(),
            }],
        );
        let forecasts = manager
            .fetch_community_forecasts(&market, start_timestamp)
            .await;
        assert!(!forecasts.is_empty());
        let first = &forecasts[0];
        assert_eq!(first.area_uuid, "area-uuid");
        assert_eq!(first.area_hash, "area-hash");
        assert_eq!(first.community_uuid, "community-uuid");
        assert_eq!(first.time_slot, start_timestamp);
        assert!(first.energy_kwh > 0.);
        assert_eq!(first.confidence, 0.9);
    }
}
