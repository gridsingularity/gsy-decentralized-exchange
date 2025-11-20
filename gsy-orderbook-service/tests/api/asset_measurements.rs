use crate::helpers::{init_app, stop_app};
use gsy_offchain_primitives::db_api_schema::profiles::{
    PVMeasurementSchema, SmartMeterMeasurementSchema, BatteryMeasurementSchema,
    TransformerMeasurementSchema, MeasurementMetadataSchema
};


# [tokio::test]
async fn test_post_and_fetch_pv_asset_measurements() {
    let app = init_app().await;
    let client = reqwest::Client::new();

    // Test PV Measurement
    let pv_measurement = PVMeasurementSchema {
        metadata: MeasurementMetadataSchema {
            area_uuid: "pv1".to_string(),
            community_uuid: "community1".to_string(),
            asset_type: "PV".to_string(),
            time_slot: 12345,
            creation_time: 12344,
        },
        current_A: 10.5,
        power_kW: 1000.0,
        voltage_V: 230.0,
        energy_kWh: 5000.0,
    };

    let pv_measurements = vec![pv_measurement];
    let resp = client
        .post(&format!("{}/asset_measurements", &app.address))
        .json(&pv_measurements)
        .send()
        .await
        .unwrap();
    let status = resp.status().as_u16();
    assert_eq!(status, 200);

    // Get PV Measurements
    let resp = client
        .get(&format!("{}/asset_measurements?area_uuid={}&start_time=12340&end_time=12350",
                      &app.address, "pv1".to_string()))
        .send()
        .await
        .unwrap();

    let status = resp.status().as_u16();
    println!("Response: {:?}", resp.text().await.unwrap());
    assert_eq!(status, 209);
    stop_app(app).await;
}

# [tokio::test]
async fn test_post_battery_asset_measurements() {
    let app = init_app().await;
    let client = reqwest::Client::new();

    // Test Battery Measurement
    let battery_measurement = BatteryMeasurementSchema {
        metadata: MeasurementMetadataSchema {
            area_uuid: "battery1".to_string(),
            community_uuid: "community1".to_string(),
            asset_type: "Battery".to_string(),
            time_slot: 12345,
            creation_time: 12344,
        },
        current_A: 20.0,
        power_kW: 2000.0,
        power_charge_kW: 1500.0,
        power_discharge_kW: 500.0,
        soc: 75.5,
        temperature_C: 25.0,
        voltage_V: 48.0,
        energy_charge_kWh: 12.0,
        energy_discharge_kWh: 11.0
    };

    let battery_measurements = vec![battery_measurement];
    let resp = client
        .post(&format!("{}/asset_measurements", &app.address))
        .json(&battery_measurements)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status().as_u16(), 200);
    stop_app(app).await;
}

# [tokio::test]
async fn test_post_transformer_asset_measurements() {
    let app = init_app().await;
    let client = reqwest::Client::new();

    // Test Transformer Measurement
    let transformer_measurement = TransformerMeasurementSchema {
        metadata: MeasurementMetadataSchema {
            area_uuid: "transformer_1".to_string(),
            community_uuid: "community1".to_string(),
            asset_type: "Transformer".to_string(),
            time_slot: 12345,
            creation_time: 12344,
        },
        energy_kWh: 100.0,
        grid_frequency: 50.0,
        current_A_p1: 10.0,
        current_A_p2: 11.0,
        current_A_p3: 12.0,
        power_kW_p1: 2.3,
        power_kW_p2: 2.4,
        power_kW_p3: 2.5,
        reactive_power_kvar_p1: 0.5,
        reactive_power_kvar_p2: 0.6,
        reactive_power_kvar_p3: 0.7,
        voltage_V_p1: 230.0,
        voltage_V_p2: 231.0,
        voltage_V_p3: 232.0,
    };

    let transformer_measurements = vec![transformer_measurement];
    let resp = client
        .post(&format!("{}/asset_measurements", &app.address))
        .json(&transformer_measurements)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status().as_u16(), 200);
    stop_app(app).await;
}

# [tokio::test]
async fn test_post_smart_meter_asset_measurements() {
    let app = init_app().await;
    let client = reqwest::Client::new();

        // Test Smart Meter Measurement
    let smart_meter_measurement = SmartMeterMeasurementSchema {
        metadata: MeasurementMetadataSchema {
            area_uuid: "smart_meter_1".to_string(),
            community_uuid: "community1".to_string(),
            asset_type: "SmartMeter".to_string(),
            time_slot: 12345,
            creation_time: 12344,
        },
        energy_grid_injection_kWh: 1000.0,
        energy_grid_injection_day_kWh: 10000.0,
        grid_frequency: 50.0,
        current_A_p1: 5.0,
        current_A_p2: 5.1,
        current_A_p3: 5.2,
        power_kW_p1: 1150.0,
        power_kW_p2: 1175.0,
        power_kW_p3: 1200.0,
        power_kW_pv: 3000.0,
        reactive_power_kvar_p1: 100.0,
        reactive_power_kvar_p2: 110.0,
        reactive_power_kvar_p3: 120.0,
        voltage_V_p1: 230.0,
        voltage_V_p2: 231.0,
        voltage_V_p3: 232.0,
    };

    let smart_meter_measurements = vec![smart_meter_measurement];
    let resp = client
        .post(&format!("{}/asset_measurements", &app.address))
        .json(&smart_meter_measurements)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status().as_u16(), 200);
    stop_app(app).await;
}
