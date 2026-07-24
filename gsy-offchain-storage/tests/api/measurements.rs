use crate::helpers::{init_app, stop_app};
use gsy_offchain_primitives::db_api_schema::profiles::{
    FlowDirection, ForecastSchema, MeasurementPointSchema, MeasurementPointType, MeasurementSchema,
    TimeseriesSchema,
};

fn make_measurement(facility_id: &str, time_slot: u64, energy_kwh: f64) -> MeasurementSchema {
    MeasurementSchema {
        facility_id: facility_id.to_string(),
        community_uuid: "community1".to_string(),
        time_slot,
        creation_time: time_slot - 1,
        energy_kwh,
    }
}

fn make_forecast(facility_id: &str, time_slot: u64, energy_kwh: f64) -> ForecastSchema {
    ForecastSchema {
        facility_id: facility_id.to_string(),
        community_uuid: "community1".to_string(),
        time_slot,
        creation_time: time_slot - 100,
        energy_kwh,
        confidence: 0.9,
    }
}

fn make_measurement_point(measurement_id: &str, asset_name: &str) -> MeasurementPointSchema {
    MeasurementPointSchema {
        point_type: MeasurementPointType::Measurement,
        measurement_id: measurement_id.to_string(),
        property_measured: "active_power".to_string(),
        unit: "kW".to_string(),
        direction: FlowDirection::Export,
        energy_accumulated: false,
        time_resolution: "15m".to_string(),
        phase: 1,
        asset_name: asset_name.to_string(),
        datasource_name: Some("DS-1-MQTT".to_string()),
    }
}

#[tokio::test]
async fn post_and_filter_measurements() {
    let app = init_app().await;
    let address = app.address.clone();
    let client = reqwest::Client::new();

    let measurements = vec![
        make_measurement("area-1", 1_900_000_000, 4.2),
        make_measurement("area-2", 1_900_000_900, -2.0),
    ];

    let resp = client
        .post(&format!("{}/measurements", &address))
        .json(&measurements)
        .send()
        .await
        .unwrap();
    assert_eq!(200, resp.status().as_u16());

    let resp = client
        .get(&format!(
            "{}/measurements?start_time=1900000000&end_time=1900000001",
            &address
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(200, resp.status().as_u16());
    let returned: Vec<MeasurementSchema> = resp.json().await.unwrap();
    assert_eq!(returned.len(), 1);
    assert_eq!(returned[0].facility_id, "area-1");

    let resp = client
        .get(&format!(
            "{}/measurement-points?asset_name=area-1&type=Measurement",
            &address
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(200, resp.status().as_u16());
    let points: Vec<MeasurementPointSchema> = resp.json().await.unwrap();
    assert_eq!(points.len(), 1);
    assert_eq!(points[0].measurement_id, "measurement:community1:area-1");

    let resp = client
        .get(&format!(
            "{}/timeseries?measurement_point=measurement:community1:area-1",
            &address
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(200, resp.status().as_u16());
    let values: Vec<TimeseriesSchema> = resp.json().await.unwrap();
    assert_eq!(values.len(), 1);
    assert_eq!(values[0].value, 4.2);
    stop_app(app).await;
}

#[tokio::test]
async fn post_and_filter_forecasts() {
    let app = init_app().await;
    let address = app.address.clone();
    let client = reqwest::Client::new();

    let forecasts = vec![
        make_forecast("area-1", 1_900_000_000, 4.2),
        make_forecast("area-2", 1_900_000_900, -2.0),
    ];

    let resp = client
        .post(&format!("{}/forecasts", &address))
        .json(&forecasts)
        .send()
        .await
        .unwrap();
    assert_eq!(200, resp.status().as_u16());

    let resp = client
        .get(&format!("{}/forecasts?facility_id=area-2", &address))
        .send()
        .await
        .unwrap();
    assert_eq!(200, resp.status().as_u16());
    let returned: Vec<ForecastSchema> = resp.json().await.unwrap();
    assert_eq!(returned.len(), 1);
    assert_eq!(returned[0].facility_id, "area-2");

    let resp = client
        .get(&format!(
            "{}/measurement-points?asset_name=area-2&type=Forecast",
            &address
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(200, resp.status().as_u16());
    let points: Vec<MeasurementPointSchema> = resp.json().await.unwrap();
    assert_eq!(points.len(), 1);
    assert_eq!(points[0].measurement_id, "forecast:community1:area-2");
    stop_app(app).await;
}

#[tokio::test]
async fn post_and_filter_measurement_points() {
    let app = init_app().await;
    let address = app.address.clone();
    let client = reqwest::Client::new();

    let points = vec![
        make_measurement_point("MSMT-1", "PV-IE-007"),
        MeasurementPointSchema {
            point_type: MeasurementPointType::Forecast,
            measurement_id: "FCST-1".to_string(),
            ..make_measurement_point("FCST-1", "PV-IE-007")
        },
    ];

    let resp = client
        .post(&format!("{}/measurement-points", &address))
        .json(&points)
        .send()
        .await
        .unwrap();
    assert_eq!(200, resp.status().as_u16());

    let resp = client
        .get(&format!(
            "{}/measurement-points?asset_name=PV-IE-007",
            &address
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(200, resp.status().as_u16());
    let returned: Vec<MeasurementPointSchema> = resp.json().await.unwrap();
    assert_eq!(returned.len(), 2);

    let resp = client
        .get(&format!(
            "{}/measurement-points?asset_name=PV-IE-007&type=Forecast",
            &address
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(200, resp.status().as_u16());
    let only_forecasts: Vec<MeasurementPointSchema> = resp.json().await.unwrap();
    assert_eq!(only_forecasts.len(), 1);
    assert_eq!(only_forecasts[0].measurement_id, "FCST-1");
    stop_app(app).await;
}

#[tokio::test]
async fn post_and_filter_timeseries() {
    let app = init_app().await;
    let address = app.address.clone();
    let client = reqwest::Client::new();

    let values = vec![
        TimeseriesSchema {
            measurement_point: "MSMT-1".to_string(),
            timestamp: "2026-03-27T10:00:30Z".to_string(),
            value: 0.23,
        },
        TimeseriesSchema {
            measurement_point: "MSMT-1".to_string(),
            timestamp: "2026-03-27T10:15:30Z".to_string(),
            value: 0.45,
        },
    ];

    let resp = client
        .post(&format!("{}/timeseries", &address))
        .json(&values)
        .send()
        .await
        .unwrap();
    assert_eq!(200, resp.status().as_u16());

    let resp = client
        .get(&format!("{}/timeseries?measurement_point=MSMT-1", &address))
        .send()
        .await
        .unwrap();
    assert_eq!(200, resp.status().as_u16());
    let returned: Vec<TimeseriesSchema> = resp.json().await.unwrap();
    assert_eq!(returned.len(), 2);

    let resp = client
        .get(&format!(
            "{}/timeseries?measurement_point=MSMT-1&start_time=2026-03-27T10:10:00Z",
            &address
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(200, resp.status().as_u16());
    let after_window: Vec<TimeseriesSchema> = resp.json().await.unwrap();
    assert_eq!(after_window.len(), 1);
    assert_eq!(after_window[0].timestamp, "2026-03-27T10:15:30Z");
    stop_app(app).await;
}
