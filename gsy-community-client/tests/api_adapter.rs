use gsy_community_client::offchain_storage_connector::adapter::AreaMarketInfoAdapter;
use gsy_community_client::external_api::{
    ExternalForecast, ExternalMeasurement, ExternalCommunityTopology, ExternalAreaTopology};
use gsy_offchain_primitives::db_api_schema::market::{AreaTopologySchema, MarketTopologySchema};
use gsy_community_client::time_utils::get_last_and_next_timeslot;

use subxt::utils::H256;
use serde_json;
use httpmock::prelude::*;
use tracing::Level;
use tracing_subscriber;

fn setup_tracing() {
    tracing_subscriber::fmt().with_max_level(Level::INFO).init();
}


#[tokio::test]
async fn test_get_or_create_market_topology() {
    setup_tracing();
    let server = MockServer::start();

    let (_, time_slot) = get_last_and_next_timeslot();

    let external_topology = ExternalCommunityTopology {
        community_name: "comm_name".to_string(),
        community_uuid: "comm_uuid".to_string(),
        areas: vec![
            ExternalAreaTopology {
                area_uuid: "area_uuid".to_string(),
                area_name: "area_name".to_string(),
            }
        ]
    };

    let expected_market = MarketTopologySchema {
        creation_time: 123,
        time_slot: 456,
        market_id: H256::random(),
        community_uuid: "comm_uuid".to_string(),
        community_name: "comm_name".to_string(),
        area_uuids: vec![
            AreaTopologySchema {
                area_uuid: "area_uuid".to_string(),
                name: "area_name".to_string(),
                area_hash: H256::random(),
            }
        ]
    };

    let market_json_str = serde_json::to_string(&expected_market).unwrap();

    let mock_request = server.mock(|when, then| {
        when.method(GET)
            .path("/community-market")
            .query_param("community_uuid", "comm_uuid")
            .query_param("time_slot", time_slot.to_string());
        then.status(200)
            .header("content-type", "text/html; charset=UTF-8")
            .body(market_json_str);
    });

    let adapter = AreaMarketInfoAdapter::new(
        Some(server.base_url()));
    let market = adapter.get_or_create_market_topology(
        external_topology, time_slot).await.unwrap();
    assert_eq!(market, expected_market);
    mock_request.assert();
}


#[cfg(test)]
mod tests {
    use super::*;
    use tracing::Level;
    use tracing_subscriber;

    fn setup_tracing() {
        tracing_subscriber::fmt().with_max_level(Level::INFO).init();
    }

    #[test]
    fn test_convert_forecast_to_internal_schema() {
        let adapter = AreaMarketInfoAdapter::new(None);
        let forecast = ExternalForecast {
            time_slot: 123123,
            creation_time: 456456,
            community_uuid: "comm_uuid".to_string(),
            energy_kwh: 11.,
            area_uuid: "area_uuid".to_string(),
            confidence: 0.4
        };
        let converted_forecast = adapter.convert_forecast_to_internal_schema(&forecast);
        assert_eq!(converted_forecast.area_uuid, "area_uuid");
        assert_eq!(converted_forecast.community_uuid, "comm_uuid");
        assert_eq!(converted_forecast.energy_kwh, 11.);
        assert_eq!(converted_forecast.confidence, 0.4);
        assert_eq!(converted_forecast.time_slot, 123123);
        assert_eq!(converted_forecast.creation_time, 456456);
    }

    #[test]
    fn test_convert_measurement_to_internal_schema() {
        let adapter = AreaMarketInfoAdapter::new(None);
        let measurement = ExternalMeasurement {
            time_slot: 123123,
            creation_time: 456456,
            community_uuid: "comm_uuid".to_string(),
            energy_kwh: 11.,
            area_uuid: "area_uuid".to_string(),
        };
        let converted_measurement = adapter.convert_measurement_to_internal_schema(&measurement);
        assert_eq!(converted_measurement.area_uuid, "area_uuid");
        assert_eq!(converted_measurement.community_uuid, "comm_uuid");
        assert_eq!(converted_measurement.energy_kwh, 11.);
        assert_eq!(converted_measurement.time_slot, 123123);
        assert_eq!(converted_measurement.creation_time, 456456);
    }

}