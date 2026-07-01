use crate::constants::CommunityClientConstants;
use chrono::{DateTime, SecondsFormat, Utc};
use reqwest::Client as ReqwestClient;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Debug)]
struct DemandForecastRequestParams {
    meter: String,
    site: String,
    start_time: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct DemandForecastPoint {
    pub timestamp: DateTime<Utc>,
    pub forecast: f64,
    pub p5: f64,
    pub p95: f64,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct DemandForecastResponse {
    pub meter: String,
    pub start_time: DateTime<Utc>,
    pub demand_forecast: Vec<DemandForecastPoint>,
}

/// Internal untagged union used to parse a demand-forecast HTTP response body.
/// The API occasionally returns HTTP 200 with an error body such as
/// `{"error":"index 0 is out of bounds for axis 0 with size 0"}`.
/// Because that body has no `meter` / `start_time` / `demand_forecast` fields it
/// falls through to the `Error` variant; a valid success body still hits `Success`.
#[derive(Deserialize)]
#[serde(untagged)]
enum DemandForecastApiResponse {
    Success(DemandForecastResponse),
    Error { error: String },
}

/// Distinguishes a transport / HTTP-level failure from an API-reported error
/// (HTTP 200 body containing `{"error": "..."}`).
#[derive(Debug)]
pub enum DemandForecastError {
    /// A `reqwest` send / status / decode error.
    Http(reqwest::Error),
    /// The server returned HTTP 200 but with an error body (e.g. empty-series pandas bug).
    Api(String),
}

impl std::fmt::Display for DemandForecastError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DemandForecastError::Http(e) => write!(f, "HTTP error: {}", e),
            DemandForecastError::Api(msg) => write!(f, "API-reported error: {}", msg),
        }
    }
}

impl std::error::Error for DemandForecastError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            DemandForecastError::Http(e) => Some(e),
            DemandForecastError::Api(_) => None,
        }
    }
}

impl From<reqwest::Error> for DemandForecastError {
    fn from(e: reqwest::Error) -> Self {
        DemandForecastError::Http(e)
    }
}

#[derive(Clone)]
pub struct DemandForecastApiConnection {
    client: ReqwestClient,
    address: String,
    api_key: String,
}

impl DemandForecastApiConnection {
    pub fn new() -> Self {
        DemandForecastApiConnection {
            client: ReqwestClient::builder()
                .timeout(std::time::Duration::from_secs(
                    CommunityClientConstants.HTTP_REQUEST_TIMEOUT_SEC,
                ))
                .connect_timeout(std::time::Duration::from_secs(
                    CommunityClientConstants.HTTP_CONNECT_TIMEOUT_SEC,
                ))
                .build()
                .expect("Failed to build demand forecast HTTP client"),
            address: CommunityClientConstants.FEDECOM_DEMAND_FORECAST_URL.clone(),
            api_key: CommunityClientConstants
                .FEDECOM_DEMAND_FORECAST_API_KEY
                .clone(),
        }
    }

    // Fetch the demand forecast time series for one meter, starting at start_time.
    // Passing the community name as meter returns the aggregated community demand.
    pub async fn fetch(
        &self,
        meter: &str,
        site: &str,
        start_time: DateTime<Utc>,
    ) -> Result<DemandForecastResponse, DemandForecastError> {
        let request_params = DemandForecastRequestParams {
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
            .json::<DemandForecastApiResponse>()
            .await?;
        match raw {
            DemandForecastApiResponse::Success(r) => Ok(r),
            DemandForecastApiResponse::Error { error } => Err(DemandForecastError::Api(error)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_body_deserializes_to_error_variant() {
        let json = r#"{"error":"index 0 is out of bounds for axis 0 with size 0"}"#;
        let parsed: DemandForecastApiResponse = serde_json::from_str(json).unwrap();
        match parsed {
            DemandForecastApiResponse::Error { error } => {
                assert_eq!(error, "index 0 is out of bounds for axis 0 with size 0");
            }
            DemandForecastApiResponse::Success(_) => {
                panic!("error body should not parse as Success");
            }
        }
    }

    #[test]
    fn success_body_deserializes_to_success_variant() {
        let json = r#"{
            "meter": "LIC08SM",
            "start_time": "2026-05-21T16:15:00+00:00",
            "demand_forecast": [
                {"timestamp": "2026-05-21T16:15:00+00:00", "forecast": 0.199, "p5": 0.169, "p95": 0.199}
            ]
        }"#;
        let parsed: DemandForecastApiResponse = serde_json::from_str(json).unwrap();
        match parsed {
            DemandForecastApiResponse::Success(r) => {
                assert_eq!(r.meter, "LIC08SM");
                assert_eq!(r.demand_forecast.len(), 1);
            }
            DemandForecastApiResponse::Error { .. } => {
                panic!("valid success body should not parse as Error");
            }
        }
    }
}
