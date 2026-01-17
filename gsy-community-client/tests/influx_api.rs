use gsy_community_client::external_api::MeasurementInfluxDBConnection;
use chrono::{Utc, TimeZone};

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_read_measurements_from_influx_works() {
        let client = MeasurementInfluxDBConnection::new();
        let start_time = Utc.with_ymd_and_hms(2025, 10, 1, 12, 0, 0).unwrap();
        let end_time = Utc.with_ymd_and_hms(2025, 10, 1, 12, 15, 0).unwrap();
        let measurements = client.read(start_time, end_time).await;
        println!("{:?}", measurements);
        assert!(measurements.len() > 0);
        assert!(measurements.contains_key("AIC01"));
        assert_eq!(measurements.get("AIC01").unwrap().get(&start_time).unwrap().import_Wh, 0.);
        assert_eq!(measurements.get("AIC01").unwrap().get(&start_time).unwrap().export_Wh, 4578.);
        assert_eq!(measurements.get("AIC01").unwrap().get(&start_time).unwrap().consumption_Wh, 93.0);
        assert_eq!(measurements.get("AIC01").unwrap().get(&start_time).unwrap().export_pv_Wh, 4671.);
        assert_eq!(measurements.get("AIC01").unwrap().get(&start_time).unwrap().net_energy_Wh(), -4578.);
        assert!(measurements.contains_key("AIC16"));
        assert_eq!(measurements.get("AIC16").unwrap().get(&start_time).unwrap().import_Wh, 43.);
        assert_eq!(measurements.get("AIC16").unwrap().get(&start_time).unwrap().export_Wh, 0.);
        assert_eq!(measurements.get("AIC16").unwrap().get(&start_time).unwrap().consumption_Wh, 43.0);
        assert_eq!(measurements.get("AIC16").unwrap().get(&start_time).unwrap().export_pv_Wh, 0.);
        assert_eq!(measurements.get("AIC16").unwrap().get(&start_time).unwrap().net_energy_Wh(), 43.);
        assert!(measurements.contains_key("LIC07"));
        assert_eq!(measurements.get("LIC07").unwrap().get(&start_time).unwrap().import_Wh, 0.);
        assert_eq!(measurements.get("LIC07").unwrap().get(&start_time).unwrap().export_Wh, 1020.);
        assert_eq!(measurements.get("LIC07").unwrap().get(&start_time).unwrap().consumption_Wh, 0.0);
        assert_eq!(measurements.get("LIC07").unwrap().get(&start_time).unwrap().export_pv_Wh, 0.);
        assert_eq!(measurements.get("LIC07").unwrap().get(&start_time).unwrap().net_energy_Wh(), -1020.);
        assert!(measurements.contains_key("LIC17"));
        assert_eq!(measurements.get("LIC17").unwrap().get(&start_time).unwrap().import_Wh, 5.);
        assert_eq!(measurements.get("LIC17").unwrap().get(&start_time).unwrap().export_Wh, 495.);
        assert_eq!(measurements.get("LIC17").unwrap().get(&start_time).unwrap().consumption_Wh, 0.);
        assert_eq!(measurements.get("LIC17").unwrap().get(&start_time).unwrap().export_pv_Wh, 0.);
        assert_eq!(measurements.get("LIC17").unwrap().get(&start_time).unwrap().net_energy_Wh(), -490.);
    }
}
