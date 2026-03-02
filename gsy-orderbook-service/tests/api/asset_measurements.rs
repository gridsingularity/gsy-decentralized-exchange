use crate::helpers::{init_app, stop_app, TestApp};
use actix_web::web;
use gsy_offchain_primitives::db_api_schema::market::{AreaTopologySchema, MarketTopologySchema};
use gsy_offchain_primitives::db_api_schema::profiles::{
    BatteryMeasurementSchema, MeasurementMetadataSchema, PVMeasurementSchema,
    SmartMeterMeasurementSchema, TransformerMeasurementSchema,
};
use gsy_offchain_primitives::MarketType;
use test_context::{test_context, AsyncTestContext};

struct AssetMeasurementsTestContext {
    app: TestApp,
    client: reqwest::Client,
}

impl AsyncTestContext for AssetMeasurementsTestContext {
    async fn setup() -> AssetMeasurementsTestContext {
        let app = init_app().await;
        let client = reqwest::Client::new();
        let market_topology = MarketTopologySchema {
            creation_time: 12344,
            time_slot: 12345,
            market_id: "new_market".to_string(),
            market_type: MarketType::Spot,
            community_areas: vec![
                AreaTopologySchema {
                    area_type: "PV".to_string(),
                    area_uuid: "pv1".to_string(),
                    name: "pv1name".to_string(),
                },
                AreaTopologySchema {
                    area_type: "SmartMeter".to_string(),
                    area_uuid: "smartmeter1".to_string(),
                    name: "smartmeter1name".to_string(),
                },
                AreaTopologySchema {
                    area_type: "Battery".to_string(),
                    area_uuid: "battery1".to_string(),
                    name: "battery1name".to_string(),
                },
                AreaTopologySchema {
                    area_type: "Transformer".to_string(),
                    area_uuid: "transformer1".to_string(),
                    name: "transformer1name".to_string(),
                },
            ],
            community_name: "community1".to_string(),
            community_uuid: "community1".to_string(),
        };
        let db = web::Data::new(app.db_wrapper.clone());
        let _saved = db
            .get_ref()
            .markets()
            .insert(market_topology.clone())
            .await
            .unwrap();
        AssetMeasurementsTestContext { app, client }
    }

    async fn teardown(self) {
        stop_app(self.app).await;
    }
}

#[test_context(AssetMeasurementsTestContext)]
#[tokio::test]
async fn test_post_and_fetch_pv_asset_measurements(ctx: &mut AssetMeasurementsTestContext) {
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

    // Sending the measurements for the PV of the community
    let pv_measurements = vec![pv_measurement];
    let resp = ctx
        .client
        .post(&format!("{}/asset-measurements", &ctx.app.address))
        .json(&pv_measurements)
        .send()
        .await
        .unwrap();
    let status = resp.status().as_u16();
    assert_eq!(status, 200);

    // Get PV Measurements
    let resp = ctx
        .client
        .get(&format!(
            "{}/asset-measurements?community_uuid={}&area_uuid={}&start_time=12340&end_time=12350",
            &ctx.app.address,
            "community1".to_string(),
            "pv1".to_string()
        ))
        .send()
        .await
        .unwrap();

    let status = resp.status().as_u16();
    let measurements: Vec<PVMeasurementSchema> = resp.json().await.unwrap();
    assert_eq!(status, 200);
    assert_eq!(measurements.len(), 1);
    assert_eq!(measurements[0].metadata.area_uuid, "pv1");
    assert_eq!(measurements[0].metadata.time_slot, 12345);
    assert_eq!(measurements[0].metadata.creation_time, 12344);
    assert_eq!(measurements[0].power_kW, 1000.0);
    assert_eq!(measurements[0].energy_kWh, 5000.0);
}

#[test_context(AssetMeasurementsTestContext)]
#[tokio::test]
async fn test_post_battery_asset_measurements(ctx: &mut AssetMeasurementsTestContext) {
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
        energy_discharge_kWh: 11.0,
    };

    let battery_measurements = vec![battery_measurement];
    let resp = ctx
        .client
        .post(&format!("{}/asset-measurements", &ctx.app.address))
        .json(&battery_measurements)
        .send()
        .await
        .unwrap();

    let status = resp.status().as_u16();
    assert_eq!(status, 200);

    // Get Battery Measurements
    let resp = ctx
        .client
        .get(&format!(
            "{}/asset-measurements?community_uuid={}&area_uuid={}&start_time=12340&end_time=12350",
            &ctx.app.address,
            "community1".to_string(),
            "battery1".to_string()
        ))
        .send()
        .await
        .unwrap();

    let status = resp.status().as_u16();
    let measurements: Vec<BatteryMeasurementSchema> = resp.json().await.unwrap();
    assert_eq!(status, 200);
    assert_eq!(measurements.len(), 1);
    assert_eq!(measurements[0].metadata.area_uuid, "battery1");
    assert_eq!(measurements[0].metadata.time_slot, 12345);
    assert_eq!(measurements[0].metadata.creation_time, 12344);
    assert_eq!(measurements[0].power_kW, 2000.0);
    assert_eq!(measurements[0].power_charge_kW, 1500.0);
    assert_eq!(measurements[0].power_discharge_kW, 500.0);
    assert_eq!(measurements[0].soc, 75.5);
}

#[test_context(AssetMeasurementsTestContext)]
#[tokio::test]
async fn test_post_transformer_asset_measurements(ctx: &mut AssetMeasurementsTestContext) {
    // Test Transformer Measurement
    let transformer_measurement = TransformerMeasurementSchema {
        metadata: MeasurementMetadataSchema {
            area_uuid: "transformer1".to_string(),
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
    let resp = ctx
        .client
        .post(&format!("{}/asset-measurements", &ctx.app.address))
        .json(&transformer_measurements)
        .send()
        .await
        .unwrap();
    let status = resp.status().as_u16();
    assert_eq!(status, 200);

    // Get Transformer Measurements
    let resp = ctx
        .client
        .get(&format!(
            "{}/asset-measurements?community_uuid={}&area_uuid={}&start_time=12340&end_time=12350",
            &ctx.app.address,
            "community1".to_string(),
            "transformer1".to_string()
        ))
        .send()
        .await
        .unwrap();

    let status = resp.status().as_u16();
    let measurements: Vec<TransformerMeasurementSchema> = resp.json().await.unwrap();
    assert_eq!(status, 200);
    assert_eq!(measurements.len(), 1);
    assert_eq!(measurements[0].metadata.area_uuid, "transformer1");
    assert_eq!(measurements[0].metadata.time_slot, 12345);
    assert_eq!(measurements[0].metadata.creation_time, 12344);
    assert_eq!(measurements[0].current_A_p1, 10.0);
    assert_eq!(measurements[0].power_kW_p2, 2.4);
    assert_eq!(measurements[0].reactive_power_kvar_p3, 0.7);
    assert_eq!(measurements[0].voltage_V_p1, 230.0);
}

#[test_context(AssetMeasurementsTestContext)]
#[tokio::test]
async fn test_post_smart_meter_asset_measurements(ctx: &mut AssetMeasurementsTestContext) {
    // Test Smart Meter Measurement
    let smart_meter_measurement = SmartMeterMeasurementSchema {
        metadata: MeasurementMetadataSchema {
            area_uuid: "smartmeter1".to_string(),
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
    let resp = ctx
        .client
        .post(&format!("{}/asset-measurements", &ctx.app.address))
        .json(&smart_meter_measurements)
        .send()
        .await
        .unwrap();
    let status = resp.status().as_u16();
    assert_eq!(status, 200);

    // Get Smart Meter Measurements
    let resp = ctx
        .client
        .get(&format!(
            "{}/asset-measurements?community_uuid={}&area_uuid={}&start_time=12340&end_time=12350",
            &ctx.app.address,
            "community1".to_string(),
            "smartmeter1".to_string()
        ))
        .send()
        .await
        .unwrap();

    let status = resp.status().as_u16();
    let measurements: Vec<SmartMeterMeasurementSchema> = resp.json().await.unwrap();
    assert_eq!(status, 200);
    assert_eq!(measurements.len(), 1);
    assert_eq!(measurements[0].metadata.area_uuid, "smartmeter1");
    assert_eq!(measurements[0].metadata.time_slot, 12345);
    assert_eq!(measurements[0].metadata.creation_time, 12344);
    assert_eq!(measurements[0].power_kW_p1, 1150.0);
    assert_eq!(measurements[0].current_A_p2, 5.1);
    assert_eq!(measurements[0].reactive_power_kvar_p3, 120.0);
    assert_eq!(measurements[0].voltage_V_p3, 232.0);
}
