use gsy_community_client::offchain_storage_connector::adapter::AreaMarketInfoAdapter;
use gsy_community_client::time_utils::get_last_and_next_timeslot;
use httpmock::prelude::*;
use primitives::db_api_schema::profiles::{ForecastSchema, MeasurementSchema};
use primitives::utils::{bytes16_to_hex, generate_market_id};
use primitives::{MarketType, MatchingAlgorithm};

const COMMUNITY_ID: &str = "11111111-1111-4111-8111-111111111111";

#[tokio::test]
async fn test_create_market_posts_market_schema() {
    let server = MockServer::start();
    let (_, time_slot) = get_last_and_next_timeslot();

    let mock_request = server.mock(|when, then| {
        when.method(POST).path("/markets");
        then.status(200).header("content-type", "application/json");
    });

    let adapter = AreaMarketInfoAdapter::new(Some(server.base_url()));
    let market = adapter
        .create_market(
            COMMUNITY_ID.to_string(),
            time_slot,
            MatchingAlgorithm::PayAsBid,
        )
        .await
        .unwrap();

    assert_eq!(market.market_type, MarketType::Spot);
    assert_eq!(market.community_id, COMMUNITY_ID);
    assert_eq!(
        market.market_id,
        bytes16_to_hex(generate_market_id(
            COMMUNITY_ID,
            MarketType::Spot,
            time_slot,
        ))
    );
    assert_eq!(market.delivery_start_time, format!("{time_slot:020}"));
    mock_request.assert();
}

#[tokio::test]
async fn test_forward_forecast_uses_facility_profile_endpoints() {
    let server = MockServer::start();

    let measurement_points_request = server.mock(|when, then| {
        when.method(POST).path("/measurement-points");
        then.status(200);
    });
    let timeseries_request = server.mock(|when, then| {
        when.method(POST).path("/timeseries");
        then.status(200);
    });

    let adapter = AreaMarketInfoAdapter::new(Some(server.base_url()));
    adapter
        .forward_forecast(vec![ForecastSchema {
            facility_id: "facility_uuid".to_string(),
            community_uuid: "comm_uuid".to_string(),
            time_slot: 123123,
            creation_time: 456456,
            energy_kwh: 11.,
            confidence: 0.4,
        }])
        .await
        .unwrap();

    measurement_points_request.assert();
    timeseries_request.assert();
}

#[tokio::test]
async fn test_forward_measurement_uses_facility_profile_endpoints() {
    let server = MockServer::start();

    let measurement_points_request = server.mock(|when, then| {
        when.method(POST).path("/measurement-points");
        then.status(200);
    });
    let timeseries_request = server.mock(|when, then| {
        when.method(POST).path("/timeseries");
        then.status(200);
    });

    let adapter = AreaMarketInfoAdapter::new(Some(server.base_url()));
    adapter
        .forward_measurement(vec![MeasurementSchema {
            facility_id: "facility_uuid".to_string(),
            community_uuid: "comm_uuid".to_string(),
            time_slot: 123123,
            creation_time: 456456,
            energy_kwh: 11.,
        }])
        .await
        .unwrap();

    measurement_points_request.assert();
    timeseries_request.assert();
}
