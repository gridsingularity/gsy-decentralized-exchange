use gsy_community_client::offchain_storage_connector::adapter::AreaMarketInfoAdapter;
use httpmock::prelude::*;
use primitives::db_api_schema::profiles::{ForecastSchema, MeasurementSchema};
use primitives::utils::{epoch_to_datetime, epoch_to_rfc3339};
use serde_json::json;

#[tokio::test]
async fn test_forward_forecast_uses_facility_profile_endpoints() {
    let server = MockServer::start();

    let measurement_points_request = server.mock(|when, then| {
        when.method(POST)
            .path("/measurement-points")
            .body_includes("forecast:0xABCDEFAB123412341234ABCDEFABCDEF:facility_uuid")
            .body_includes("\"datasource_name\":\"0xABCDEFAB123412341234ABCDEFABCDEF\"");
        then.status(200);
    });
    // Timeseries are posted with RFC 3339 timestamps.
    let timeseries_request = server.mock(|when, then| {
        when.method(POST).path("/timeseries").json_body(json!([{
            "measurement_point": "forecast:0xABCDEFAB123412341234ABCDEFABCDEF:facility_uuid",
            "timestamp": epoch_to_rfc3339(123123),
            "value": 11.0,
        }]));
        then.status(200);
    });

    let adapter = AreaMarketInfoAdapter::new(Some(server.base_url()));
    let forecast = ForecastSchema {
        facility_id: "facility_uuid".to_string(),
        community_uuid: "0xABCDEFAB123412341234ABCDEFABCDEF".to_string(),
        time_slot: epoch_to_datetime(123123).unwrap(),
        creation_time: epoch_to_datetime(456456).unwrap(),
        energy_kwh: 11.,
        confidence: 0.4,
    };
    adapter
        .forward_forecast(vec![forecast.clone()])
        .await
        .unwrap();
    let invalid = ForecastSchema {
        community_uuid: " community-1".into(),
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
            .body_includes("measurement:ABCDEFAB-1234-1234-1234-ABCDEFABCDEF:ABCDEFAB-ABCD-ABCD-ABCD-ABCDEFABCDEF")
            .body_includes("\"asset_name\":\"ABCDEFAB-ABCD-ABCD-ABCD-ABCDEFABCDEF\"")
            .body_includes("\"datasource_name\":\"ABCDEFAB-1234-1234-1234-ABCDEFABCDEF\"");
        then.status(200);
    });
    let timeseries_request = server.mock(|when, then| {
        when.method(POST).path("/timeseries").json_body(json!([{
            "measurement_point": "measurement:ABCDEFAB-1234-1234-1234-ABCDEFABCDEF:ABCDEFAB-ABCD-ABCD-ABCD-ABCDEFABCDEF",
            "timestamp": epoch_to_rfc3339(123123),
            "value": 11.0,
        }]));
        then.status(200);
    });

    let adapter = AreaMarketInfoAdapter::new(Some(server.base_url()));
    let measurement = MeasurementSchema {
        facility_id: "ABCDEFAB-ABCD-ABCD-ABCD-ABCDEFABCDEF".to_string(),
        community_uuid: "ABCDEFAB-1234-1234-1234-ABCDEFABCDEF".to_string(),
        time_slot: epoch_to_datetime(123123).unwrap(),
        creation_time: epoch_to_datetime(456456).unwrap(),
        energy_kwh: 11.,
    };
    adapter
        .forward_measurement(vec![measurement.clone()])
        .await
        .unwrap();
    let invalid = MeasurementSchema {
        community_uuid: "community-1 ".into(),
        ..measurement.clone()
    };
    assert!(adapter
        .forward_measurement(vec![measurement, invalid])
        .await
        .is_err());

    measurement_points_request.assert();
    timeseries_request.assert();
}
