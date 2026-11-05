use gsy_community_client::offchain_storage_connector::adapter::AreaMarketInfoAdapter;
use httpmock::prelude::*;
use primitives::db_api_schema::profiles::{ForecastSchema, MeasurementSchema};

#[tokio::test]
async fn test_forward_forecast_uses_facility_profile_endpoints() {
    let server = MockServer::start();

    let measurement_points_request = server.mock(|when, then| {
        when.method(POST)
            .path("/measurement-points")
            .body_includes("forecast:abcdefab-1234-1234-1234-abcdefabcdef:facility_uuid")
            .body_includes("\"datasource_name\":\"abcdefab-1234-1234-1234-abcdefabcdef\"");
        then.status(200);
    });
    let timeseries_request = server.mock(|when, then| {
        when.method(POST)
            .path("/timeseries")
            .body_includes("forecast:abcdefab-1234-1234-1234-abcdefabcdef:facility_uuid");
        then.status(200);
    });

    let adapter = AreaMarketInfoAdapter::new(Some(server.base_url()));
    let forecast = ForecastSchema {
        facility_id: "facility_uuid".to_string(),
        community_uuid: "0xABCDEFAB123412341234ABCDEFABCDEF".to_string(),
        time_slot: 123123,
        creation_time: 456456,
        energy_kwh: 11.,
        confidence: 0.4,
    };
    adapter
        .forward_forecast(vec![forecast.clone()])
        .await
        .unwrap();
    let invalid = ForecastSchema {
        community_uuid: "community-1".into(),
        ..forecast.clone()
    };
    assert!(adapter
        .forward_forecast(vec![forecast, invalid])
        .await
        .is_err());

    measurement_points_request.assert();
    timeseries_request.assert();
}

#[tokio::test]
async fn test_forward_measurement_uses_facility_profile_endpoints() {
    let server = MockServer::start();

    let measurement_points_request = server.mock(|when, then| {
        when.method(POST)
            .path("/measurement-points")
            .body_includes("measurement:abcdefab-1234-1234-1234-abcdefabcdef:abcdefab-abcd-abcd-abcd-abcdefabcdef")
            .body_includes("\"asset_name\":\"abcdefab-abcd-abcd-abcd-abcdefabcdef\"")
            .body_includes("\"datasource_name\":\"abcdefab-1234-1234-1234-abcdefabcdef\"");
        then.status(200);
    });
    let timeseries_request = server.mock(|when, then| {
        when.method(POST).path("/timeseries").body_includes(
            "measurement:abcdefab-1234-1234-1234-abcdefabcdef:abcdefab-abcd-abcd-abcd-abcdefabcdef",
        );
        then.status(200);
    });

    let adapter = AreaMarketInfoAdapter::new(Some(server.base_url()));
    let measurement = MeasurementSchema {
        facility_id: "ABCDEFAB-ABCD-ABCD-ABCD-ABCDEFABCDEF".to_string(),
        community_uuid: "ABCDEFAB-1234-1234-1234-ABCDEFABCDEF".to_string(),
        time_slot: 123123,
        creation_time: 456456,
        energy_kwh: 11.,
    };
    adapter
        .forward_measurement(vec![measurement.clone()])
        .await
        .unwrap();
    let invalid = MeasurementSchema {
        community_uuid: "00000000-0000-0000-0000-000000000000".into(),
        ..measurement.clone()
    };
    assert!(adapter
        .forward_measurement(vec![measurement, invalid])
        .await
        .is_err());

    measurement_points_request.assert();
    timeseries_request.assert();
}
