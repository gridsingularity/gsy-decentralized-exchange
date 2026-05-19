use crate::helpers::{init_app, stop_app};
use gsy_offchain_primitives::db_api_schema::profiles::{
    FlowDirection, MeasurementPointSchema, MeasurementPointType, TimeseriesSchema,
};

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
        .get(&format!(
            "{}/timeseries?measurement_point=MSMT-1",
            &address
        ))
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
