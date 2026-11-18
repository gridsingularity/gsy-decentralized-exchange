use gsy_community_client::offchain_storage_connector::adapter::AreaMarketInfoAdapter;
use httpmock::prelude::*;
use primitives::db_api_schema::profiles::{ForecastSchema, MeasurementSchema};
use primitives::utils::{epoch_to_datetime, epoch_to_rfc3339};
use serde_json::json;

#[tokio::test]
async fn test_forward_forecast_uses_facility_profile_endpoints() {
    let server = MockServer::start();

    let measurement_points_request = server.mock(|when, then| {
        when.method(POST).path("/measurement-points");
        then.status(200);
    });
    // Timeseries are posted with RFC 3339 timestamps.
    let timeseries_request = server.mock(|when, then| {
        when.method(POST).path("/timeseries").json_body(json!([{
            "measurement_point": "forecast:comm_uuid:facility_uuid",
            "timestamp": epoch_to_rfc3339(123123),
            "value": 11.0,
        }]));
        then.status(200);
    });

    let adapter = AreaMarketInfoAdapter::new(Some(server.base_url()));
    adapter
        .forward_forecast(vec![ForecastSchema {
            facility_id: "facility_uuid".to_string(),
            community_uuid: "comm_uuid".to_string(),
            time_slot: epoch_to_datetime(123123).unwrap(),
            creation_time: epoch_to_datetime(456456).unwrap(),
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
        when.method(POST).path("/timeseries").json_body(json!([{
            "measurement_point": "measurement:comm_uuid:facility_uuid",
            "timestamp": epoch_to_rfc3339(123123),
            "value": 11.0,
        }]));
        then.status(200);
    });

    let adapter = AreaMarketInfoAdapter::new(Some(server.base_url()));
    adapter
        .forward_measurement(vec![MeasurementSchema {
            facility_id: "facility_uuid".to_string(),
            community_uuid: "comm_uuid".to_string(),
            time_slot: epoch_to_datetime(123123).unwrap(),
            creation_time: epoch_to_datetime(456456).unwrap(),
            energy_kwh: 11.,
        }])
        .await
        .unwrap();

    measurement_points_request.assert();
    timeseries_request.assert();
}
