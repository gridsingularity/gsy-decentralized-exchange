use chrono::{TimeZone, Utc};
use gsy_community_client::external_measurements::influxdb_api::{
    MeasurementInfluxDBConnection, InfluxMeasurementMeterData};

#[cfg(test)]
mod tests {
    use super::*;

    fn meter_data(import_wh: Option<f64>, export_wh: Option<f64>) -> InfluxMeasurementMeterData {
        InfluxMeasurementMeterData {
            sensor_id: "TEST".to_string(),
            time: Utc::now(),
            import_Wh: import_wh,
            export_Wh: export_wh,
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
            Some(0.)
        );
        assert_eq!(
            measurements
                .get("AIC01")
                .unwrap()
                .get(&start_time)
                .unwrap()
                .export_Wh,
            Some(4578.)
        );
        assert!(
            (measurements
                .get("AIC01")
                .unwrap()
                .get(&start_time)
                .unwrap()
                .net_energy_kWh()
                .unwrap()
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
            Some(43.)
        );
        assert_eq!(
            measurements
                .get("AIC16")
                .unwrap()
                .get(&start_time)
                .unwrap()
                .export_Wh,
            Some(0.)
        );
        assert!(
            (measurements
                .get("AIC16")
                .unwrap()
                .get(&start_time)
                .unwrap()
                .net_energy_kWh()
                .unwrap()
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
            Some(0.)
        );
        assert_eq!(
            measurements
                .get("LIC07")
                .unwrap()
                .get(&start_time)
                .unwrap()
                .export_Wh,
            Some(1020.)
        );
        assert!(
            (measurements
                .get("LIC07")
                .unwrap()
                .get(&start_time)
                .unwrap()
                .net_energy_kWh()
                .unwrap()
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
            Some(5.)
        );
        assert_eq!(
            measurements
                .get("LIC17")
                .unwrap()
                .get(&start_time)
                .unwrap()
                .export_Wh,
            Some(495.)
        );
        assert!(
            (measurements
                .get("LIC17")
                .unwrap()
                .get(&start_time)
                .unwrap()
                .net_energy_kWh()
                .unwrap()
                - (-0.49))
                .abs()
                < 1e-9
        );
    }

    #[test]
    fn test_net_energy_kwh_net_import() {
        // import 1500 Wh, export 250 Wh => (1500 - 250) / 1000 = 1.25 kWh
        let data = meter_data(Some(1500.0), Some(250.0));
        assert!((data.net_energy_kWh().unwrap() - 1.25).abs() < 1e-9);
    }

    #[test]
    fn test_net_energy_kwh_net_export() {
        // import 0 Wh, export 4578 Wh => -4.578 kWh (production is negative)
        let data = meter_data(Some(0.0), Some(4578.0));
        assert!((data.net_energy_kWh().unwrap() - (-4.578)).abs() < 1e-9);
    }

    #[test]
    fn test_net_energy_kwh_is_none_without_export() {
        // A missing export is a gap, not a zero: no net value rather than a net import.
        let data = meter_data(Some(1500.0), None);
        assert_eq!(data.net_energy_kWh(), None);
    }

    #[test]
    fn test_net_energy_kwh_is_none_without_import() {
        let data = meter_data(None, Some(250.0));
        assert_eq!(data.net_energy_kWh(), None);
    }

    #[test]
    fn test_net_energy_kwh_is_none_without_import_and_export() {
        let data = meter_data(None, None);
        assert_eq!(data.net_energy_kWh(), None);
    }

    #[test]
    fn test_net_energy_kwh_real_zero_is_some() {
        // Both series present with zero energy is a real zero, distinct from a gap.
        let data = meter_data(Some(0.0), Some(0.0));
        assert_eq!(data.net_energy_kWh(), Some(0.0));
    }
}
