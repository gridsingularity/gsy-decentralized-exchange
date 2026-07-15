use crate::helpers::init_app;
use gsy_offchain_primitives::db_api_schema::profiles::MeasurementSchema;
use serde_json::Value;

const COMMUNITY_UUID: &str = "community_uuid";

fn asset_measurement(area_uuid: &str, time_slot: u64, energy_kwh: f64) -> MeasurementSchema {
    MeasurementSchema {
        area_uuid: area_uuid.to_string(),
        area_hash: format!("{}_hash", area_uuid),
        community_uuid: COMMUNITY_UUID.to_string(),
        time_slot,
        creation_time: time_slot,
        energy_kwh,
    }
}

/// Community-level measurements (posted for the community itself rather than
/// an individual asset) are stored with `area_uuid == community_uuid`.
fn community_measurement(time_slot: u64, energy_kwh: f64) -> MeasurementSchema {
    asset_measurement(COMMUNITY_UUID, time_slot, energy_kwh)
}

async fn post_measurements(address: &str, measurements: &[MeasurementSchema]) {
    let client = reqwest::Client::new();
    let resp = client
        .post(&format!("{}/measurements", address))
        .header("Content-Type", "application/json")
        .json(&measurements)
        .send()
        .await
        .unwrap();
    assert_eq!(200, resp.status().as_u16());
}

async fn get_guarantees_of_origin(address: &str, query: &str) -> Vec<Value> {
    let client = reqwest::Client::new();
    let resp = client
        .get(&format!("{}/guarantees-of-origin-measurements{}", address, query))
        .header("Content-Type", "application/json")
        .send()
        .await
        .unwrap();
    assert_eq!(200, resp.status().as_u16());
    resp.json().await.unwrap()
}

#[tokio::test]
async fn get_guarantees_of_origin_returns_renamed_asset_entries_without_community_ones() {
    let app = init_app().await;
    let address = app.address;

    post_measurements(
        &address,
        &[
            asset_measurement("pv_uuid", 100, 5.5),
            asset_measurement("battery_uuid", 100, -2.5),
            community_measurement(100, 3.0),
        ],
    )
    .await;

    let resp_json = get_guarantees_of_origin(&address, "").await;

    // The community-level entry is excluded.
    assert_eq!(resp_json.len(), 2);
    for item in &resp_json {
        let object = item.as_object().unwrap();
        // Renamed fields are present, the original names and area_hash are not.
        assert!(object.contains_key("asset_id"));
        assert!(object.contains_key("community_id"));
        assert!(!object.contains_key("area_uuid"));
        assert!(!object.contains_key("community_uuid"));
        assert!(!object.contains_key("area_hash"));
        assert_eq!(object.len(), 5);
        assert_eq!(item["community_id"], COMMUNITY_UUID);
        assert_ne!(item["asset_id"], COMMUNITY_UUID);
        assert_eq!(item["time_slot"], 100);
        assert_eq!(item["creation_time"], 100);
    }

    let pv = resp_json
        .iter()
        .find(|item| item["asset_id"] == "pv_uuid")
        .unwrap();
    assert_eq!(pv["energy_kwh"], 5.5);
    let battery = resp_json
        .iter()
        .find(|item| item["asset_id"] == "battery_uuid")
        .unwrap();
    assert_eq!(battery["energy_kwh"], -2.5);
}

#[tokio::test]
async fn get_guarantees_of_origin_filters_by_time_window() {
    let app = init_app().await;
    let address = app.address;

    post_measurements(
        &address,
        &[
            asset_measurement("pv_uuid", 100, 1.0),
            asset_measurement("pv_uuid", 200, 2.0),
            asset_measurement("battery_uuid", 300, 3.0),
            community_measurement(200, 6.0),
        ],
    )
    .await;

    // Lower bound only (inclusive).
    let resp_json = get_guarantees_of_origin(&address, "?start_time=200").await;
    assert_eq!(resp_json.len(), 2);
    assert!(resp_json.iter().all(|item| item["time_slot"] != 100));

    // Upper bound only (inclusive).
    let resp_json = get_guarantees_of_origin(&address, "?end_time=200").await;
    assert_eq!(resp_json.len(), 2);
    assert!(resp_json.iter().all(|item| item["time_slot"] != 300));

    // Both bounds: only the asset entry at time_slot 200 remains; the
    // community-level entry in the same window is still excluded.
    let resp_json = get_guarantees_of_origin(&address, "?start_time=150&end_time=250").await;
    assert_eq!(resp_json.len(), 1);
    assert_eq!(resp_json[0]["asset_id"], "pv_uuid");
    assert_eq!(resp_json[0]["time_slot"], 200);
    assert_eq!(resp_json[0]["energy_kwh"], 2.0);
}
