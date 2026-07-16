use gsy_community_client::external_forecasts::pv_api::{PvForecastRequestParams, pv_avg_watts_to_kwh};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_body_embeds_api_key() {
        let params = PvForecastRequestParams {
            meter: "aic01".to_string(),
            site: "AIC".to_string(),
            start_time: "2026-05-21T16:15:00+00:00".to_string(),
            api_key: "fedecom_user".to_string(),
        };
        let json = serde_json::to_value(&params).unwrap();
        assert_eq!(json["api_key"], "fedecom_user");
        assert_eq!(json["meter"], "aic01");
        assert_eq!(json["site"], "AIC");
    }

    #[test]
    fn pv_avg_watts_to_kwh_converts_over_slot() {
        // 12000 W average over a 15-min slot => 12 kW * 0.25 h = 3.0 kWh
        assert!((pv_avg_watts_to_kwh(12000.0) - 3.0).abs() < 1e-9);
        // No production => 0 kWh.
        assert!((pv_avg_watts_to_kwh(0.0) - 0.0).abs() < 1e-9);
        // Example value: 123.68235294117646 W => ~0.030920588 kWh.
        assert!((pv_avg_watts_to_kwh(123.68235294117646) - 0.030920588235294).abs() < 1e-9);
    }
}
