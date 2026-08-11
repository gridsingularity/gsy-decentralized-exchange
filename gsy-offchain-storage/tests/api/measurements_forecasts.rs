use crate::helpers::init_app;
use actix_web::web;
use gsy_offchain_primitives::db_api_schema::profiles::{ForecastSchema, MeasurementSchema};

#[tokio::test]
async fn get_measurements_succeeds() {
    let app = init_app().await;
    let address = app.address;
    let measurement1 = MeasurementSchema {
        area_uuid: "my_uuid".to_string(),
        area_hash: "my_hash".to_string(),
        community_uuid: "my_community".to_string(),
        energy_kwh: 12.21,
        time_slot: 1232123213,
        creation_time: 1232123213,
    };
    let measurement2 = MeasurementSchema {
        area_uuid: "my_uuid1".to_string(),
        area_hash: "my_hash1".to_string(),
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
        area_hash: "my_hash".to_string(),
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
        .filter_measurements("my_uuid".to_string().try_into().ok(), None, None)
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
        area_hash: "my_hash".to_string(),
        community_uuid: "my_community".to_string(),
        energy_kwh: 12.21,
        time_slot: 1232123213,
        creation_time: 1232123213,
        confidence: 1.0,
    };
    let forecast2 = ForecastSchema {
        area_uuid: "my_uuid1".to_string(),
        area_hash: "my_hash1".to_string(),
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

    assert_eq!(saved, 2);

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
        area_hash: "my_hash".to_string(),
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
        .filter_forecasts("my_uuid".to_string().try_into().ok(), None, None, None)
        .await
        .unwrap();
    assert_eq!(1, saved.len());
    let forecast_db = saved.into_iter().nth(0).unwrap();
    assert_eq!(forecast_db, forecast);
}

#[tokio::test]
async fn post_forecasts_upserts_on_area_and_timeslot() {
    let app = init_app().await;
    let db = web::Data::new(app.db_wrapper);
    let forecast = ForecastSchema {
        area_uuid: "area_1".to_string(),
        area_hash: "hash_1".to_string(),
        community_uuid: "community_1".to_string(),
        energy_kwh: 12.21,
        time_slot: 1_800_000_000,
        creation_time: 1_700_000_000,
        confidence: 0.5,
    };
    let updated = ForecastSchema {
        energy_kwh: 20.0,
        creation_time: 1_700_003_600,
        confidence: 0.9,
        ..forecast.clone()
    };

    let forecasts = db.get_ref().forecasts();
    assert_eq!(forecasts.insert_forecasts(vec![forecast]).await.unwrap(), 1);
    assert_eq!(
        forecasts
            .insert_forecasts(vec![updated.clone()])
            .await
            .unwrap(),
        1
    );

    let stored = forecasts
        .filter_forecasts(Some("area_1".to_string()), None, None, None)
        .await
        .unwrap();
    assert_eq!(
        stored.len(),
        1,
        "same (area_uuid, time_slot) must overwrite, not duplicate"
    );
    assert_eq!(stored[0], updated);
}

#[tokio::test]
async fn post_forecasts_distinct_slots_coexist() {
    let app = init_app().await;
    let db = web::Data::new(app.db_wrapper);
    let base = ForecastSchema {
        area_uuid: "area_1".to_string(),
        area_hash: "hash_1".to_string(),
        community_uuid: "community_1".to_string(),
        energy_kwh: 12.21,
        time_slot: 1_800_000_000,
        creation_time: 1_700_000_000,
        confidence: 0.5,
    };
    let other_slot = ForecastSchema {
        time_slot: 1_800_000_900,
        ..base.clone()
    };

    let forecasts = db.get_ref().forecasts();
    forecasts
        .insert_forecasts(vec![base.clone(), other_slot.clone()])
        .await
        .unwrap();

    let stored = forecasts
        .filter_forecasts(Some("area_1".to_string()), None, None, None)
        .await
        .unwrap();
    assert_eq!(stored.len(), 2);
}

#[tokio::test]
async fn day_ahead_timeslot_roundtrips() {
    // 2027-01-15T00:00:00Z, well within u32 range (year-2106 wraparound is out of scope).
    const DAY_AHEAD_SLOT: u64 = 1_800_144_000;
    let app = init_app().await;
    let forecast = ForecastSchema {
        area_uuid: "area_day_ahead".to_string(),
        area_hash: "hash_day_ahead".to_string(),
        community_uuid: "community_1".to_string(),
        energy_kwh: 5.0,
        time_slot: DAY_AHEAD_SLOT,
        creation_time: DAY_AHEAD_SLOT - 86_400,
        confidence: 0.8,
    };

    let client = reqwest::Client::new();
    client
        .post(&format!("{}/forecasts", &app.address))
        .json(&vec![forecast.clone()])
        .send()
        .await
        .unwrap();

    let resp = client
        .get(&format!(
            "{}/forecasts?start_time={}&end_time={}",
            &app.address,
            DAY_AHEAD_SLOT - 1,
            DAY_AHEAD_SLOT + 1
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(200, resp.status().as_u16());
    let resp_json: Vec<ForecastSchema> = resp.json().await.unwrap();
    assert_eq!(resp_json, vec![forecast]);
}

#[tokio::test]
async fn get_forecasts_by_community_uuid() {
    let app = init_app().await;
    let db = web::Data::new(app.db_wrapper);
    let community_a = ForecastSchema {
        area_uuid: "area_a".to_string(),
        area_hash: "hash_a".to_string(),
        community_uuid: "community_a".to_string(),
        energy_kwh: 1.0,
        time_slot: 1_800_000_000,
        creation_time: 1_700_000_000,
        confidence: 0.5,
    };
    let community_b = ForecastSchema {
        area_uuid: "area_b".to_string(),
        area_hash: "hash_b".to_string(),
        community_uuid: "community_b".to_string(),
        energy_kwh: 2.0,
        time_slot: 1_800_000_000,
        creation_time: 1_700_000_000,
        confidence: 0.5,
    };
    db.get_ref()
        .forecasts()
        .insert_forecasts(vec![community_a.clone(), community_b.clone()])
        .await
        .unwrap();

    let client = reqwest::Client::new();
    let resp = client
        .get(&format!(
            "{}/forecasts?community_uuid=community_a",
            &app.address
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(200, resp.status().as_u16());
    let resp_json: Vec<ForecastSchema> = resp.json().await.unwrap();
    assert_eq!(resp_json, vec![community_a]);
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
