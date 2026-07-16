use gsy_community_client::external_forecasts::pv_api::{
    parse_response, pv_avg_watts_to_kwh, PvForecastPoint, PvForecastRequestParams, PvForecastResponse,
};

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{TimeZone, Utc};

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

    // A real-shaped success body plus a night slot (all-zero) point.
    const SUCCESS_BODY: &str = r#"{
        "data": {
            "pv_forecasts": [
                {
                    "timestamp": "2026-07-15T04:30:00",
                    "pv_forecast": 123.68235294117646,
                    "p5": [61.84117647058823, 74.20941176470588],
                    "p95": [160.7870588235294, 166.9711764705882]
                },
                {
                    "timestamp": "2026-07-15T23:45:00",
                    "pv_forecast": 0,
                    "p5": [0, 0],
                    "p95": [0, 0]
                }
            ]
        }
    }"#;

    #[test]
    fn success_body_parses_into_pv_response() {
        let response: PvForecastResponse = parse_response(SUCCESS_BODY).unwrap();
        let points = &response.data.pv_forecasts;
        assert_eq!(points.len(), 2);

        let first = &points[0];
        assert!((first.pv_forecast - 123.68235294117646).abs() < 1e-12);
        assert_eq!(first.p5, vec![61.84117647058823, 74.20941176470588]);
        assert_eq!(first.p95, vec![160.7870588235294, 166.9711764705882]);
        // Naive timestamp is interpreted as UTC by convention.
        assert_eq!(
            first.timestamp_utc(),
            Utc.with_ymd_and_hms(2026, 7, 15, 4, 30, 0).unwrap()
        );

        let night = &points[1];
        assert_eq!(night.pv_forecast, 0.0);
        assert_eq!(
            night.timestamp_utc(),
            Utc.with_ymd_and_hms(2026, 7, 15, 23, 45, 0).unwrap()
        );
    }

    #[test]
    fn naive_timestamp_deserializes_with_default_format() {
        // Guards the assumption that chrono's default NaiveDateTime Deserialize
        // handles the forecaster's `%Y-%m-%dT%H:%M:%S` format with plain derive.
        let point: PvForecastPoint = serde_json::from_str(
            r#"{"timestamp": "2026-07-15T04:30:00", "pv_forecast": 1.0, "p5": [0.0], "p95": [2.0]}"#,
        )
        .unwrap();
        assert_eq!(
            point.timestamp_utc(),
            Utc.with_ymd_and_hms(2026, 7, 15, 4, 30, 0).unwrap()
        );
    }

    #[test]
    fn error_body_maps_to_api_error() {
        let err = parse_response(r#"{"error": "site not found"}"#).unwrap_err();
        match err {
            gsy_community_client::external_forecasts::ForecastApiError::Api(msg) => {
                assert_eq!(msg, "site not found");
            }
            other => panic!("expected Api error, got {:?}", other),
        }
    }

    #[test]
    fn demand_shaped_body_does_not_parse_as_pv_success() {
        // A valid demand response body must not be silently accepted as a PV success.
        let demand_body = r#"{
            "meter": "LIC08SM",
            "start_time": "2026-05-21T16:15:00+00:00",
            "demand_forecast": [
                {"timestamp": "2026-05-21T16:15:00+00:00", "forecast": 0.199, "p5": 0.169, "p95": 0.199}
            ]
        }"#;
        // It has no `data` field and no `error` field, so the untagged enum matches
        // neither variant and parsing fails (mapped to an Api error).
        assert!(parse_response(demand_body).is_err());
    }

    #[test]
    fn quantile_bounds_picks_widest_band() {
        let response: PvForecastResponse = parse_response(SUCCESS_BODY).unwrap();
        let points = &response.data.pv_forecasts;

        // Widest band: min of p5, max of p95.
        let (q5, q95) = points[0].quantile_bounds();
        assert_eq!(q5, 61.84117647058823);
        assert_eq!(q95, 166.9711764705882);

        // Night slot: all zeros.
        assert_eq!(points[1].quantile_bounds(), (0.0, 0.0));
    }

    #[test]
    fn quantile_bounds_guards_empty_arrays() {
        let point = PvForecastPoint {
            timestamp: chrono::NaiveDateTime::parse_from_str(
                "2026-07-15T04:30:00",
                "%Y-%m-%dT%H:%M:%S",
            )
            .unwrap(),
            pv_forecast: 0.0,
            p5: vec![],
            p95: vec![],
        };
        let (q5, q95) = point.quantile_bounds();
        assert_eq!((q5, q95), (0.0, 0.0));
        assert!(q5.is_finite() && q95.is_finite());
    }
}
