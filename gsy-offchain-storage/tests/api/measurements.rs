use crate::helpers::{init_app, stop_app};
use primitives::db_api_schema::profiles::{
    FlowDirection, ForecastSchema, MeasurementPointSchema, MeasurementPointType, MeasurementSchema,
    TimeseriesSchema,
};
use serde_json::{json, Value};

const COMMUNITY: &str = "abcdefab-1234-1234-1234-abcdefabcdef";

async fn query_measurements_via_ewds(app: &crate::helpers::TestApp, payload: Value) -> Value {
    use primitives::ewds::{
        dto::{EwdsRequestEnvelope, EwdsSendMessageDto},
        EwdsOperation,
    };
    let gateway = crate::ewds_handler::mock_gateway().await;
    let config = crate::ewds_handler::test_config(gateway.uri());
    gsy_offchain_storage::ewds_handler::handle_request(
        &app.db_wrapper,
        &reqwest::Client::new(),
        &config,
        EwdsRequestEnvelope {
            request_id: "profile-filter".into(),
            operation: EwdsOperation::MeasurementsQuery,
            payload,
        },
    )
    .await
    .unwrap();
    let requests = gateway.received_requests().await.unwrap();
    assert_eq!(requests.len(), 1);
    let sent: EwdsSendMessageDto = serde_json::from_slice(&requests[0].body).unwrap();
    serde_json::from_str(&sent.payload).unwrap()
}

fn make_measurement(facility_id: &str, time_slot: u64, energy_kwh: f64) -> MeasurementSchema {
    MeasurementSchema {
        facility_id: facility_id.to_string(),
        community_uuid: COMMUNITY.to_string(),
        time_slot,
        creation_time: time_slot - 1,
        energy_kwh,
    }
}

fn make_forecast(facility_id: &str, time_slot: u64, energy_kwh: f64) -> ForecastSchema {
    ForecastSchema {
        facility_id: facility_id.to_string(),
        community_uuid: COMMUNITY.to_string(),
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
async fn profile_community_ids_are_exact_and_filters_combine_over_both_transports() {
    let app = init_app().await;
    let client = reqwest::Client::new();
    let facility = "abcdefab-abcd-abcd-abcd-abcdefabcdef";
    let communities = [
        COMMUNITY.to_string(),
        COMMUNITY.to_uppercase(),
        format!("0x{}", COMMUNITY.replace('-', "")),
        COMMUNITY.replace('-', ""),
        "Community-MiXeD/LIC01".into(),
        "community-mixed/lic01".into(),
        "00000000-0000-0000-0000-000000000000".into(),
    ];
    for route in ["measurements", "forecasts"] {
        for community in &communities {
            // Only an exact replay deduplicates a profile identity.
            for _ in 0..2 {
                for id in [facility.to_uppercase(), "facility-b".into()] {
                    let mut row =
                        serde_json::to_value(make_forecast(&id, 1_900_000_000, 1.0)).unwrap();
                    row["community_uuid"] = json!(community);
                    if route == "measurements" {
                        row.as_object_mut().unwrap().remove("confidence");
                    }
                    assert_eq!(
                        client
                            .post(format!("{}/{route}", app.address))
                            .json(&vec![row])
                            .send()
                            .await
                            .unwrap()
                            .status()
                            .as_u16(),
                        200
                    );
                }
            }
        }
        for (query, expected) in [
            (json!({}), communities.len() * 2),
            (
                json!({"facilityId": facility.to_uppercase()}),
                communities.len(),
            ),
            (
                json!({"areaUuid": facility.to_uppercase()}),
                communities.len(),
            ),
            (json!({"facility_id": facility}), 0),
            (json!({"communityUuid": COMMUNITY.to_uppercase()}), 2),
            (
                json!({"community_uuid": format!("0x{}", COMMUNITY.replace('-', ""))}),
                2,
            ),
            (
                json!({"communityUuid": COMMUNITY, "facilityId": facility.to_uppercase()}),
                1,
            ),
            (
                json!({"community_uuid": "Community-MiXeD/LIC01", "facility_id": "absent"}),
                0,
            ),
            (json!({"community_uuid": "Community-MiXeD/LIC01"}), 2),
            (
                json!({"communityUuid": "community-mixed/lic01", "facilityId": "facility-b"}),
                1,
            ),
            (json!({"startTime": "2030-03-17T17:46:41Z"}), 0),
        ] {
            let response = client
                .get(format!("{}/{route}", app.address))
                .query(&query)
                .send()
                .await
                .unwrap();
            assert_eq!(response.status().as_u16(), 200, "{query}");
            let rows: Value = response.json().await.unwrap();
            assert_eq!(rows.as_array().unwrap().len(), expected, "{route}: {query}");
            let mut http_pairs = rows
                .as_array()
                .unwrap()
                .iter()
                .map(|row| {
                    let community = row["community_uuid"].as_str().unwrap();
                    assert!(communities.iter().any(|id| id == community));
                    if let Some(filter) = query
                        .get("communityUuid")
                        .or_else(|| query.get("community_uuid"))
                    {
                        assert_eq!(community, filter.as_str().unwrap());
                    }
                    (
                        row["facility_id"].as_str().unwrap().to_string(),
                        community.to_string(),
                    )
                })
                .collect::<Vec<_>>();
            http_pairs.sort();
            if route == "measurements" {
                let reply = query_measurements_via_ewds(&app, query).await;
                assert_eq!(reply["success"], true);
                let mut ewds_pairs = reply["data"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|row| {
                        (
                            row["facilityId"].as_str().unwrap().to_string(),
                            row["communityUuid"].as_str().unwrap().to_string(),
                        )
                    })
                    .collect::<Vec<_>>();
                ewds_pairs.sort();
                assert_eq!(http_pairs, ewds_pairs);
            }
        }
        for invalid in ["", " padded", "padded ", "\tcommunity", "community\n"] {
            let mut valid =
                serde_json::to_value(make_forecast("not-written", 1_900_000_000, 1.0)).unwrap();
            if route == "measurements" {
                valid.as_object_mut().unwrap().remove("confidence");
            }
            let mut bad = valid.clone();
            bad["community_uuid"] = json!(invalid);
            assert_eq!(
                client
                    .post(format!("{}/{route}", app.address))
                    .json(&vec![valid, bad])
                    .send()
                    .await
                    .unwrap()
                    .status()
                    .as_u16(),
                400
            );
            assert_eq!(
                client
                    .get(format!("{}/{route}", app.address))
                    .query(&[("communityUuid", invalid)])
                    .send()
                    .await
                    .unwrap()
                    .status()
                    .as_u16(),
                400
            );
            if route == "measurements" {
                let reply =
                    query_measurements_via_ewds(&app, json!({"communityUuid": invalid})).await;
                assert_eq!(reply["success"], false);
                assert_eq!(reply["error"]["code"], "invalid_request");
            }
        }
    }
    // Both profile types preserve each exact community/facility pair, not replay duplicates.
    assert_eq!(
        app.db_wrapper
            .measurement_points()
            .count_documents(mongodb::bson::doc! {})
            .await
            .unwrap(),
        (communities.len() * 4) as u64
    );
    assert_eq!(
        app.db_wrapper
            .timeseries()
            .count_documents(mongodb::bson::doc! {})
            .await
            .unwrap(),
        (communities.len() * 4) as u64
    );
    stop_app(app).await;
}

#[tokio::test]
async fn typed_profiles_escape_identity_components_without_changing_metadata() {
    let app = init_app().await;
    let client = reqwest::Client::new();
    let time_slot = 1_900_000_000;
    let identities = [
        ("A:B", "C", "A%3AB:C", 1.0),
        ("A", "B:C", "A:B%3AC", 2.0),
        ("A%3AB", "C", "A%253AB:C", 3.0),
    ];
    for (route, prefix, point_type) in [
        (
            "measurements",
            "measurement",
            MeasurementPointType::Measurement,
        ),
        ("forecasts", "forecast", MeasurementPointType::Forecast),
    ] {
        let payload = identities
            .iter()
            .map(|&(community, facility, _, energy)| {
                if route == "measurements" {
                    let mut row = make_measurement(facility, time_slot, energy);
                    row.community_uuid = community.into();
                    serde_json::to_value(row).unwrap()
                } else {
                    let mut row = make_forecast(facility, time_slot, energy);
                    row.community_uuid = community.into();
                    serde_json::to_value(row).unwrap()
                }
            })
            .collect::<Vec<_>>();
        // Replaying the batch must retain three distinct identities at the same timestamp.
        for _ in 0..2 {
            let response = client
                .post(format!("{}/{route}", app.address))
                .json(&payload)
                .send()
                .await
                .unwrap();
            assert_eq!(response.status().as_u16(), 200, "{route}");
        }

        let points = app
            .db_wrapper
            .measurement_points()
            .filter_points(None, Some(point_type))
            .await
            .unwrap();
        assert_eq!(points.len(), identities.len(), "{route}");
        for (community, facility, escaped, energy) in identities {
            let key = format!("{prefix}:{escaped}");
            let point = points
                .iter()
                .find(|point| point.measurement_id == key)
                .unwrap();
            assert_eq!(point.datasource_name.as_deref(), Some(community));
            assert_eq!(point.asset_name, facility);
            let values = app
                .db_wrapper
                .timeseries()
                .filter_values(Some(key.clone()), None, None)
                .await
                .unwrap();
            assert_eq!(values.len(), 1, "{key}");
            assert_eq!(values[0].measurement_point, key);
            assert_eq!(values[0].timestamp, format!("{time_slot:020}"));
            assert_eq!(values[0].value, energy);

            let rows: Vec<Value> = client
                .get(format!("{}/{route}", app.address))
                .query(&[("community_uuid", community), ("facility_id", facility)])
                .send()
                .await
                .unwrap()
                .error_for_status()
                .unwrap()
                .json()
                .await
                .unwrap();
            assert_eq!(rows.len(), 1, "{key}");
            assert_eq!(rows[0]["community_uuid"], community);
            assert_eq!(rows[0]["facility_id"], facility);
            assert_eq!(rows[0]["energy_kwh"], energy);
        }
    }
    assert_eq!(
        app.db_wrapper
            .measurement_points()
            .count_documents(mongodb::bson::doc! {})
            .await
            .unwrap(),
        6
    );
    assert_eq!(
        app.db_wrapper
            .timeseries()
            .count_documents(mongodb::bson::doc! {})
            .await
            .unwrap(),
        6
    );
    stop_app(app).await;
}

#[tokio::test]
async fn datasource_metadata_is_preserved_without_identity_validation() {
    let app = init_app().await;
    let client = reqwest::Client::new();
    let datasources = [
        None,
        Some("DS-1-MQTT".to_string()),
        Some(COMMUNITY.to_uppercase()),
        Some("0xABCDEFAB123412341234ABCDEFABCDEF".to_string()),
        Some("".to_string()),
        Some(" padded datasource ".to_string()),
    ];
    for (index, datasource) in datasources.iter().cloned().enumerate() {
        let id = format!("Custom-Point-{index}");
        let mut point = make_measurement_point(&id, &format!("facility-{index}"));
        point.datasource_name = datasource.clone();
        assert_eq!(
            client
                .post(format!("{}/measurement-points", app.address))
                .json(&vec![point])
                .send()
                .await
                .unwrap()
                .status()
                .as_u16(),
            200
        );
        app.db_wrapper
            .timeseries()
            .insert_values(vec![TimeseriesSchema {
                measurement_point: id.clone(),
                timestamp: "1900000000".into(),
                value: 1.0,
            }])
            .await
            .unwrap();
        let stored = app
            .db_wrapper
            .measurement_points()
            .find_one(mongodb::bson::doc! {"measurement_id": &id})
            .await
            .unwrap()
            .unwrap();
        assert_eq!(stored.measurement_id, id);
        assert_eq!(stored.datasource_name, datasource);
    }
    let rows: Vec<MeasurementSchema> = client
        .get(format!("{}/measurements", app.address))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(rows.len(), datasources.len());
    for row in &rows {
        let index: usize = row
            .facility_id
            .strip_prefix("facility-")
            .unwrap()
            .parse()
            .unwrap();
        assert_eq!(
            row.community_uuid,
            datasources[index].as_deref().unwrap_or_default()
        );
    }
    let response = query_measurements_via_ewds(&app, json!({})).await;
    assert_eq!(response["success"], true);
    let mut communities = response["data"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| row["communityUuid"].as_str().unwrap().to_string())
        .collect::<Vec<_>>();
    communities.sort();
    let mut expected = datasources
        .into_iter()
        .map(Option::unwrap_or_default)
        .collect::<Vec<_>>();
    expected.sort();
    assert_eq!(communities, expected);
    let filtered =
        query_measurements_via_ewds(&app, json!({"communityUuid": COMMUNITY.to_uppercase()})).await;
    assert_eq!(filtered["data"].as_array().unwrap().len(), 1);
    let filtered = query_measurements_via_ewds(&app, json!({"communityUuid": COMMUNITY})).await;
    assert!(filtered["data"].as_array().unwrap().is_empty());
    stop_app(app).await;
}

#[tokio::test]
async fn profile_ingress_and_filters_preserve_facility_spelling() {
    let app = init_app().await;
    let client = reqwest::Client::new();
    let canonical = "abcdefab-abcd-abcd-abcd-abcdefabcdef";
    for route in ["measurements", "forecasts"] {
        for id in [canonical.to_uppercase(), canonical.to_string()] {
            let payload = if route == "measurements" {
                serde_json::to_value(vec![make_measurement(&id, 1_900_000_000, 4.2)]).unwrap()
            } else {
                serde_json::to_value(vec![make_forecast(&id, 1_900_000_000, 4.2)]).unwrap()
            };
            assert_eq!(
                client
                    .post(format!("{}/{route}", app.address))
                    .json(&payload)
                    .send()
                    .await
                    .unwrap()
                    .status()
                    .as_u16(),
                200
            );
            let rows: serde_json::Value = client
                .get(format!("{}/{route}", app.address))
                .query(&[("facility_id", &id)])
                .send()
                .await
                .unwrap()
                .json()
                .await
                .unwrap();
            assert_eq!(rows.as_array().unwrap().len(), 1);
            assert_eq!(rows[0]["facility_id"], id);
        }
        let mut valid =
            serde_json::to_value(make_forecast("new-facility", 1_900_000_000, 4.2)).unwrap();
        if route == "measurements" {
            valid.as_object_mut().unwrap().remove("confidence");
        }
        let mut invalid = valid.clone();
        invalid["facility_id"] = serde_json::json!(" padded ");
        assert_eq!(
            client
                .post(format!("{}/{route}", app.address))
                .json(&vec![valid, invalid])
                .send()
                .await
                .unwrap()
                .status()
                .as_u16(),
            400
        );
        let rows: serde_json::Value = client
            .get(format!("{}/{route}", app.address))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(
            rows.as_array().unwrap().len(),
            2,
            "invalid batch must not be partly written"
        );
    }
    let points = vec![make_measurement_point(
        "Custom-Point-ID",
        &canonical.to_uppercase(),
    )];
    assert_eq!(
        client
            .post(format!("{}/measurement-points", app.address))
            .json(&points)
            .send()
            .await
            .unwrap()
            .status()
            .as_u16(),
        200
    );
    let stored = app
        .db_wrapper
        .measurement_points()
        .find_one(mongodb::bson::doc! {"measurement_id": "Custom-Point-ID"})
        .await
        .unwrap()
        .unwrap();
    assert_eq!(stored.asset_name, canonical.to_uppercase());
    assert_eq!(stored.datasource_name.as_deref(), Some("DS-1-MQTT"));
    assert_eq!(stored.measurement_id, "Custom-Point-ID");
    stop_app(app).await;
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
            "{}/measurements?start_time=2030-03-17T17:46:40Z&end_time=2030-03-17T17:46:41Z",
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
    assert_eq!(
        points[0].measurement_id,
        "measurement:abcdefab-1234-1234-1234-abcdefabcdef:area-1"
    );

    let resp = client
        .get(&format!(
            "{}/timeseries?measurement_point=measurement:abcdefab-1234-1234-1234-abcdefabcdef:area-1",
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
    assert_eq!(
        points[0].measurement_id,
        "forecast:abcdefab-1234-1234-1234-abcdefabcdef:area-2"
    );
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
