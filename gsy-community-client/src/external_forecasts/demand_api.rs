use crate::constants::CommunityClientConstants;
use crate::external_forecasts::ForecastApiError;
use chrono::{DateTime, SecondsFormat, Utc};
use reqwest::Client as ReqwestClient;
use serde::{Deserialize, Serialize};
use std::future::Future;
use std::pin::Pin;

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

/// Boxed future returned by [`DemandForecaster::fetch`].
///
/// A hand-rolled boxed future is used instead of an `async fn` in the trait because the
/// manager holds the forecaster behind a trait object (`Arc<dyn DemandForecaster>`), and
/// native `async fn` in traits is not object-safe. `async-trait` is deliberately avoided so
/// no new dependency is pulled in.
pub type DemandForecastFuture<'a> =
    Pin<Box<dyn Future<Output = Result<DemandForecastResponse, ForecastApiError>> + Send + 'a>>;

/// Common seam over the FEDECOM forecasting back-ends (GD/LIC demand, temporary AIC demand,
/// and PV). Each back-end parses its own  format and normalises it into the shared
/// [`DemandForecastResponse`] so the manager stays back-end agnostic and new back-ends
/// can be added without touching the manager.
pub trait DemandForecaster: Send + Sync {
    // Fetch the forecast time series for one meter, starting at start_time. Passing the
    // community/site name as meter returns the aggregated demand (for the GD/LIC back-end).
    fn fetch<'a>(
        &'a self,
        meter: &'a str,
        site: &'a str,
        start_time: DateTime<Utc>,
    ) -> DemandForecastFuture<'a>;
}

impl DemandForecaster for DemandForecastApiConnection {
    fn fetch<'a>(
        &'a self,
        meter: &'a str,
        site: &'a str,
        start_time: DateTime<Utc>,
    ) -> DemandForecastFuture<'a> {
        Box::pin(DemandForecastApiConnection::fetch(
            self, meter, site, start_time,
        ))
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
    ) -> Result<DemandForecastResponse, ForecastApiError> {
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
            DemandForecastApiResponse::Error { error } => Err(ForecastApiError::Api(error)),
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
