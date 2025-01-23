use crate::helpers::init_app;
use actix_web::web;
use gsy_offchain_primitives::db_api_schema::profiles::{MeasurementSchema, ForecastSchema};

#[tokio::test]
async fn post_measurements_succeeds() {
    let app = init_app().await;
    let address = app.address;
    let measurement = MeasurementSchema {
        area_uuid: "my_uuid".to_string(),
        community_uuid: "my_community".to_string(),
        energy_kwh: 12.21,
        time_slot: 1232123213,
        creation_time: 1232123213,
    };

    let body = vec![measurement.clone()];

    let client = reqwest::Client::new();
    let resp = client
        .post(&format!("{}/measurements", &address))
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await.unwrap();

    let status = resp.status();
    assert_eq!(200, status.as_u16());

    let db = web::Data::new(app.db_wrapper);
    let saved = db
        .get_ref()
        .measurements()
        .get_all_measurements_for_area("my_uuid".to_string())
        .await
        .unwrap();
    assert_eq!(1, saved.len());
    let measurement_db = saved.into_iter().nth(0).unwrap();
    assert_eq!(measurement_db, measurement);
}

#[tokio::test]
async fn post_measurements_fails_with_incorrect_json() {
    let app = init_app().await;
    let address = app.address;

    let client = reqwest::Client::new();
    let test_cases = vec![("area_uuid", "err"), ("energy_kwh", "err")];

    for (invalid_body, error_message) in test_cases {
        let resp = client
            .post(&format!("{}/measurements", &address))
            .header("Content-Type", "application/json")
            .body(invalid_body)
            .send()
            .await
            .expect("Failed to execute request.");
        assert_eq!(
            400,
            resp.status().as_u16(),
            "The API did not fail with 400 Bad Request when the payload was {}.",
            error_message
        );
    }
}

#[tokio::test]
async fn post_forecasts_succeeds() {
    let app = init_app().await;
    let address = app.address;
    let forecast = ForecastSchema {
        area_uuid: "my_uuid".to_string(),
        community_uuid: "my_uuid".to_string(),
        energy_kwh: 12.21,
        time_slot: 1232123213,
        creation_time: 1232123213,
        confidence: 0.5
    };

    let body = vec![forecast.clone()];

    let client = reqwest::Client::new();
    let resp = client
        .post(&format!("{}/forecasts", &address))
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await.unwrap();

    let status = resp.status();
    assert_eq!(200, status.as_u16());

    let db = web::Data::new(app.db_wrapper);
    let saved = db
        .get_ref()
        .forecasts()
        .get_all_forecasts_for_area("my_uuid".to_string())
        .await
        .unwrap();
    assert_eq!(1, saved.len());
    let forecast_db = saved.into_iter().nth(0).unwrap();
    assert_eq!(forecast_db, forecast);
}

#[tokio::test]
async fn post_forecasts_fails_with_incorrect_json() {
    let app = init_app().await;
    let address = app.address;

    let client = reqwest::Client::new();
    let test_cases = vec![("area_uuid", "err"), ("energy_kwh", "err")];

    for (invalid_body, error_message) in test_cases {
        let resp = client
            .post(&format!("{}/forecasts", &address))
            .header("Content-Type", "application/json")
            .body(invalid_body)
            .send()
            .await
            .expect("Failed to execute request.");
        assert_eq!(
            400,
            resp.status().as_u16(),
            "The API did not fail with 400 Bad Request when the payload was {}.",
            error_message
        );
    }
}
