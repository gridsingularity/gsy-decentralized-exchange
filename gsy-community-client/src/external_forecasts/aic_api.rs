use crate::constants::CommunityClientConstants;
use crate::external_forecasts::demand_api::{
    DemandForecastError, DemandForecastFuture, DemandForecastResponse, DemandForecaster,
};
use chrono::{DateTime, SecondsFormat, Utc};
use reqwest::Client as ReqwestClient;
use serde::{Deserialize, Serialize};

// The temporary AIC demand back-end has no ontology, so the meters it serves are hardcoded.
// Numeric meters run aic01..=aic48 with aic02 and aic44 excluded.
const AIC_NUMERIC_METER_RANGE: std::ops::RangeInclusive<u8> = 1..=48;
const AIC_EXCLUDED_NUMERIC_METERS: [u8; 2] = [2, 44];
// Fixed set of aggregate / building meters served alongside the numeric ones.
const AIC_NAMED_METERS: [&str; 10] = [
    "aic_aggregate",
    "aic_b1",
    "aic_b2",
    "aic_b3",
    "aic_b5",
    "aic_b6",
    "aic_b9",
    "aic_b11",
    "aic_b12",
    "aic_b14",
];

/// Builds the hardcoded list of AIC meters (aic01, aic03..aic48 excluding aic02/aic44, plus
/// the fixed aggregate / building meters). Numeric meters are zero-padded to two digits.
pub fn aic_meters() -> Vec<String> {
    let mut meters: Vec<String> = AIC_NUMERIC_METER_RANGE
        .filter(|n| !AIC_EXCLUDED_NUMERIC_METERS.contains(n))
        .map(|n| format!("aic{:02}", n))
        .collect();
    meters.extend(AIC_NAMED_METERS.iter().map(|m| m.to_string()));
    meters
}

// AIC forecast request parameters. Like the GD/LIC demand API, authentication is via the
// `X-API-Key` header rather than a body field, and the AIC back-end expects `site == "AIC"`.
#[derive(Serialize, Debug)]
struct AicForecastRequestParams {
    meter: String,
    site: String,
    start_time: String,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum AicForecastApiResponse {
    Success(DemandForecastResponse),
    Error { error: String },
}

#[derive(Clone)]
pub struct AicForecastApiConnection {
    client: ReqwestClient,
    address: String,
    api_key: String,
}

impl AicForecastApiConnection {
    pub fn new() -> Self {
        AicForecastApiConnection {
            client: ReqwestClient::builder()
                .timeout(std::time::Duration::from_secs(
                    CommunityClientConstants.HTTP_REQUEST_TIMEOUT_SEC,
                ))
                .connect_timeout(std::time::Duration::from_secs(
                    CommunityClientConstants.HTTP_CONNECT_TIMEOUT_SEC,
                ))
                .build()
                .expect("Failed to build AIC forecast HTTP client"),
            // TODO: finalize AIC endpoint URL.
            address: CommunityClientConstants.FEDECOM_AIC_FORECAST_URL.clone(),
            api_key: CommunityClientConstants
                .FEDECOM_AIC_FORECAST_API_KEY
                .clone(),
        }
    }

    // Fetch the AIC demand forecast time series for one meter, starting at start_time. Like the
    // GD/LIC demand API, the api_key is sent via the `X-API-Key` header.
    pub async fn fetch(
        &self,
        meter: &str,
        site: &str,
        start_time: DateTime<Utc>,
    ) -> Result<DemandForecastResponse, DemandForecastError> {
        let request_params = AicForecastRequestParams {
            meter: meter.to_string(),
            site: site.to_string(),
            start_time: start_time.to_rfc3339_opts(SecondsFormat::Secs, false),
        };
        let raw = self
            .client
            .post(&self.address)
            .header("X-API-Key", self.api_key.as_str())
            .json(&request_params)
            .send()
            .await?
            .error_for_status()?
            .json::<AicForecastApiResponse>()
            .await?;
        match raw {
            AicForecastApiResponse::Success(r) => Ok(r),
            AicForecastApiResponse::Error { error } => Err(DemandForecastError::Api(error)),
        }
    }
}

impl DemandForecaster for AicForecastApiConnection {
    fn fetch<'a>(
        &'a self,
        meter: &'a str,
        site: &'a str,
        start_time: DateTime<Utc>,
    ) -> DemandForecastFuture<'a> {
        Box::pin(AicForecastApiConnection::fetch(
            self, meter, site, start_time,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aic_meter_list_is_built_correctly() {
        let meters = aic_meters();
        // 48 numeric meters minus the two excluded, plus 10 named meters.
        assert_eq!(meters.len(), 46 + 10);
        assert!(meters.contains(&"aic01".to_string()));
        assert!(meters.contains(&"aic03".to_string()));
        assert!(meters.contains(&"aic48".to_string()));
        // Excluded numeric meters must be absent.
        assert!(!meters.contains(&"aic02".to_string()));
        assert!(!meters.contains(&"aic44".to_string()));
        // Named meters must be present.
        assert!(meters.contains(&"aic_aggregate".to_string()));
        assert!(meters.contains(&"aic_b1".to_string()));
        assert!(meters.contains(&"aic_b14".to_string()));
    }

    #[test]
    fn request_body_omits_api_key() {
        // Auth is header-based (`X-API-Key`), mirroring the GD/LIC demand API, so the JSON
        // body carries no `api_key` field. Assert the serialized body has none.
        let params = AicForecastRequestParams {
            meter: "aic01".to_string(),
            site: "AIC".to_string(),
            start_time: "2026-05-21T16:15:00+00:00".to_string(),
        };
        let json = serde_json::to_value(&params).unwrap();
        assert!(json.get("api_key").is_none());
        assert_eq!(json["site"], "AIC");
    }
}
