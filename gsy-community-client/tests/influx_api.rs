use chrono::{TimeZone, Utc};
use gsy_community_client::external_measurements::influxdb_api::{
    MeasurementInfluxDBConnection, InfluxMeasurementMeterData};

#[cfg(test)]
mod tests {
    use super::*;

    fn meter_data(import_Wh: f64, export_Wh: f64, export_pv_Wh: f64) -> InfluxMeasurementMeterData {
        InfluxMeasurementMeterData {
            sensor_id: "TEST".to_string(),
            time: Utc::now(),
            import_Wh,
            export_Wh,
            consumption_Wh: 0.0,
            export_pv_Wh,
        }
    }

    #[tokio::test]
    async fn test_read_measurements_from_influx_works() {
        let client = MeasurementInfluxDBConnection::new();
        let start_time = Utc.with_ymd_and_hms(2025, 10, 1, 12, 0, 0).unwrap();
        let end_time = Utc.with_ymd_and_hms(2025, 10, 1, 12, 15, 0).unwrap();
        let measurements = client.read(start_time, end_time).await;
        println!("{:?}", measurements);
        assert!(measurements.len() > 0);
        assert!(measurements.contains_key("AIC01"));
        assert_eq!(
            measurements
                .get("AIC01")
                .unwrap()
                .get(&start_time)
                .unwrap()
                .import_Wh,
            0.
        );
        assert_eq!(
            measurements
                .get("AIC01")
                .unwrap()
                .get(&start_time)
                .unwrap()
                .export_Wh,
            4578.
        );
        assert_eq!(
            measurements
                .get("AIC01")
                .unwrap()
                .get(&start_time)
                .unwrap()
                .consumption_Wh,
            93.0
        );
        assert_eq!(
            measurements
                .get("AIC01")
                .unwrap()
                .get(&start_time)
                .unwrap()
                .export_pv_Wh,
            4671.
        );
        assert!(
            (measurements
                .get("AIC01")
                .unwrap()
                .get(&start_time)
                .unwrap()
                .net_energy_kWh()
                - (-4.578))
                .abs()
                < 1e-9
        );
        assert!(measurements.contains_key("AIC16"));
        assert_eq!(
            measurements
                .get("AIC16")
                .unwrap()
                .get(&start_time)
                .unwrap()
                .import_Wh,
            43.
        );
        assert_eq!(
            measurements
                .get("AIC16")
                .unwrap()
                .get(&start_time)
                .unwrap()
                .export_Wh,
            0.
        );
        assert_eq!(
            measurements
                .get("AIC16")
                .unwrap()
                .get(&start_time)
                .unwrap()
                .consumption_Wh,
            43.0
        );
        assert_eq!(
            measurements
                .get("AIC16")
                .unwrap()
                .get(&start_time)
                .unwrap()
                .export_pv_Wh,
            0.
        );
        assert!(
            (measurements
                .get("AIC16")
                .unwrap()
                .get(&start_time)
                .unwrap()
                .net_energy_kWh()
                - 0.043)
                .abs()
                < 1e-9
        );
        assert!(measurements.contains_key("LIC07"));
        assert_eq!(
            measurements
                .get("LIC07")
                .unwrap()
                .get(&start_time)
                .unwrap()
                .import_Wh,
            0.
        );
        assert_eq!(
            measurements
                .get("LIC07")
                .unwrap()
                .get(&start_time)
                .unwrap()
                .export_Wh,
            1020.
        );
        assert_eq!(
            measurements
                .get("LIC07")
                .unwrap()
                .get(&start_time)
                .unwrap()
                .consumption_Wh,
            0.0
        );
        assert_eq!(
            measurements
                .get("LIC07")
                .unwrap()
                .get(&start_time)
                .unwrap()
                .export_pv_Wh,
            0.
        );
        assert!(
            (measurements
                .get("LIC07")
                .unwrap()
                .get(&start_time)
                .unwrap()
                .net_energy_kWh()
                - (-1.02))
                .abs()
                < 1e-9
        );
        assert!(measurements.contains_key("LIC17"));
        assert_eq!(
            measurements
                .get("LIC17")
                .unwrap()
                .get(&start_time)
                .unwrap()
                .import_Wh,
            5.
        );
        assert_eq!(
            measurements
                .get("LIC17")
                .unwrap()
                .get(&start_time)
                .unwrap()
                .export_Wh,
            495.
        );
        assert_eq!(
            measurements
                .get("LIC17")
                .unwrap()
                .get(&start_time)
                .unwrap()
                .consumption_Wh,
            0.
        );
        assert_eq!(
            measurements
                .get("LIC17")
                .unwrap()
                .get(&start_time)
                .unwrap()
                .export_pv_Wh,
            0.
        );
        assert!(
            (measurements
                .get("LIC17")
                .unwrap()
                .get(&start_time)
                .unwrap()
                .net_energy_kWh()
                - (-0.49))
                .abs()
                < 1e-9
        );
    }

    #[test]
    fn test_net_energy_kwh_net_import() {
        // import 1500 Wh, export 250 Wh => (1500 - 250) / 1000 = 1.25 kWh
        let data = meter_data(1500.0, 250.0, 0.0);
        assert!((data.net_energy_kWh() - 1.25).abs() < 1e-9);
    }

    #[test]
    fn test_net_energy_kwh_net_export() {
        // import 0 Wh, export 4578 Wh => -4.578 kWh (production is negative)
        let data = meter_data(0.0, 4578.0, 0.0);
        assert!((data.net_energy_kWh() - (-4.578)).abs() < 1e-9);
    }

    #[test]
    fn test_export_pv_kwh_conversion() {
        // 4671 Wh => 4.671 kWh
        let data = meter_data(0.0, 0.0, 4671.0);
        assert!((data.export_pv_kWh() - 4.671).abs() < 1e-9);
    }
}
