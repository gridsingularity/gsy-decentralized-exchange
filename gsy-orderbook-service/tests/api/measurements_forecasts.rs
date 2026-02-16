use crate::helpers::init_app;
use actix_web::web;
use gsy_offchain_primitives::db_api_schema::profiles::{ForecastSchema, MeasurementSchema};
use subxt::ext::sp_runtime::traits::CheckedConversion;

#[tokio::test]
async fn get_measurements_succeeds() {
    let app = init_app().await;
    let address = app.address;
    let measurement1 = MeasurementSchema {
        area_uuid: "my_uuid".to_string(),
        community_uuid: "my_community".to_string(),
        energy_kwh: 12.21,
        time_slot: 1232123213,
        creation_time: 1232123213,
    };
    let measurement2 = MeasurementSchema {
        area_uuid: "my_uuid1".to_string(),
        community_uuid: "my_community".to_string(),
        energy_kwh: 13.21,
        time_slot: 1232123215,
        creation_time: 1232123215,
    };

    let measurement_vec = vec![measurement1, measurement2];
    let db = web::Data::new(app.db_wrapper);
    let saved = db
        .get_ref()
        .measurements()
        .insert_measurements(measurement_vec.clone())
        .await
        .unwrap();

    assert_eq!(saved.len(), 2);

    // Retrieve measurements from area my_uuid
    let client = reqwest::Client::new();
    let resp = client
        .get(&format!("{}/measurements?area_uuid=my_uuid", &address))
        .header("Content-Type", "application/json")
        .send()
        .await
        .unwrap();

    let status = resp.status();
    assert_eq!(200, status.as_u16());
    let resp_json: Vec<MeasurementSchema> = resp.json().await.unwrap();
    assert_eq!(resp_json.len(), 1);
    assert_eq!(resp_json.iter().nth(0).unwrap().area_uuid, "my_uuid");
    assert_eq!(resp_json.iter().nth(0).unwrap().time_slot, 1232123213);
    assert_eq!(resp_json.iter().nth(0).unwrap().creation_time, 1232123213);
    assert_eq!(resp_json.iter().nth(0).unwrap().energy_kwh, 12.21);
    assert_eq!(
        resp_json.iter().nth(0).unwrap().community_uuid,
        "my_community"
    );

    let resp = client
        .get(&format!("{}/measurements?start_time=1232123214", &address))
        .header("Content-Type", "application/json")
        .send()
        .await
        .unwrap();
    let status = resp.status();
    assert_eq!(200, status.as_u16());
    let resp_json: Vec<MeasurementSchema> = resp.json().await.unwrap();
    assert_eq!(resp_json.len(), 1);
    assert_eq!(resp_json.iter().nth(0).unwrap().area_uuid, "my_uuid1");

    let resp = client
        .get(&format!("{}/measurements?end_time=1232123214", &address))
        .header("Content-Type", "application/json")
        .send()
        .await
        .unwrap();
    let status = resp.status();
    assert_eq!(200, status.as_u16());
    let resp_json: Vec<MeasurementSchema> = resp.json().await.unwrap();
    assert_eq!(resp_json.len(), 1);
    assert_eq!(resp_json.iter().nth(0).unwrap().area_uuid, "my_uuid");
}

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
        .await
        .unwrap();

    let status = resp.status();
    assert_eq!(200, status.as_u16());

    let db = web::Data::new(app.db_wrapper);
    let saved = db
        .get_ref()
        .measurements()
        .filter_measurements("my_uuid".to_string().checked_into(), None, None)
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
async fn get_forecasts_succeeds() {
    let app = init_app().await;
    let address = app.address;
    let forecast1 = ForecastSchema {
        area_uuid: "my_uuid".to_string(),
        community_uuid: "my_community".to_string(),
        energy_kwh: 12.21,
        time_slot: 1232123213,
        creation_time: 1232123213,
        confidence: 1.0,
    };
    let forecast2 = ForecastSchema {
        area_uuid: "my_uuid1".to_string(),
        community_uuid: "my_community".to_string(),
        energy_kwh: 13.21,
        time_slot: 1232123215,
        creation_time: 1232123215,
        confidence: 0.9,
    };

    let forecast_vec = vec![forecast1, forecast2];
    let db = web::Data::new(app.db_wrapper);
    let saved = db
        .get_ref()
        .forecasts()
        .insert_forecasts(forecast_vec.clone())
        .await
        .unwrap();

    assert_eq!(saved.len(), 2);

    // Retrieve measurements from area my_uuid
    let client = reqwest::Client::new();
    let resp = client
        .get(&format!("{}/forecasts?area_uuid=my_uuid", &address))
        .header("Content-Type", "application/json")
        .send()
        .await
        .unwrap();

    let status = resp.status();
    assert_eq!(200, status.as_u16());
    let resp_json: Vec<ForecastSchema> = resp.json().await.unwrap();
    assert_eq!(resp_json.len(), 1);
    assert_eq!(resp_json.iter().nth(0).unwrap().area_uuid, "my_uuid");
    assert_eq!(resp_json.iter().nth(0).unwrap().time_slot, 1232123213);
    assert_eq!(resp_json.iter().nth(0).unwrap().creation_time, 1232123213);
    assert_eq!(resp_json.iter().nth(0).unwrap().energy_kwh, 12.21);
    assert_eq!(
        resp_json.iter().nth(0).unwrap().community_uuid,
        "my_community"
    );
    assert_eq!(resp_json.iter().nth(0).unwrap().confidence, 1.0);

    let resp = client
        .get(&format!("{}/forecasts?start_time=1232123214", &address))
        .header("Content-Type", "application/json")
        .send()
        .await
        .unwrap();
    let status = resp.status();
    assert_eq!(200, status.as_u16());
    let resp_json: Vec<ForecastSchema> = resp.json().await.unwrap();
    assert_eq!(resp_json.len(), 1);
    assert_eq!(resp_json.iter().nth(0).unwrap().area_uuid, "my_uuid1");

    let resp = client
        .get(&format!("{}/forecasts?end_time=1232123214", &address))
        .header("Content-Type", "application/json")
        .send()
        .await
        .unwrap();
    let status = resp.status();
    assert_eq!(200, status.as_u16());
    let resp_json: Vec<ForecastSchema> = resp.json().await.unwrap();
    assert_eq!(resp_json.len(), 1);
    assert_eq!(resp_json.iter().nth(0).unwrap().area_uuid, "my_uuid");
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
        confidence: 0.5,
    };

    let body = vec![forecast.clone()];

    let client = reqwest::Client::new();
    let resp = client
        .post(&format!("{}/forecasts", &address))
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
        .unwrap();

    let status = resp.status();
    assert_eq!(200, status.as_u16());

    let db = web::Data::new(app.db_wrapper);
    let saved = db
        .get_ref()
        .forecasts()
        .filter_forecasts("my_uuid".to_string().checked_into(), None, None)
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
