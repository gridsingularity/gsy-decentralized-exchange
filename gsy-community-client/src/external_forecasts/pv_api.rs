use crate::constants::CommunityClientConstants;
use crate::external_forecasts::ForecastApiError;
use chrono::{DateTime, NaiveDateTime, SecondsFormat, Utc};
use gsy_offchain_primitives::constants::GlobalConstants;
use reqwest::Client as ReqwestClient;
use serde::{Deserialize, Serialize};

/// The PV forecaster reports average power in watts over a single market slot. This
/// function converts the average power to the energy delivered over one slot. The
/// slot duration in hours is derived from TIME_SLOT_SEC global constant.
pub fn pv_avg_watts_to_kwh(avg_watts: f64) -> f64 {
    let slot_hours = GlobalConstants.TIME_SLOT_SEC as f64 / 3600.0;
    avg_watts / 1000.0 * slot_hours
}

/// PV forecast request parameters.
#[derive(Serialize, Debug)]
pub struct PvForecastRequestParams {
    pub meter: String,
    pub site: String,
    pub start_time: String,
    pub api_key: String,
}

/// A single 15-minute PV forecast point as returned by the PV forecaster.
/// pv_forecast is the average power in watts over the slot. p5 / p95 are
/// 2-element 5th and 95th percentile arrays.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PvForecastPoint {
    /// Slot timestamp. The PV forecaster returns a naive datetime string.
    pub timestamp: NaiveDateTime,
    /// Average power in watts.
    pub pv_forecast: f64,
    /// 5th percentile band.
    pub p5: Vec<f64>,
    /// 95th percentile band.
    pub p95: Vec<f64>,
}

impl PvForecastPoint {
    /// Convert the timezone-naive timestamp as UTC.
    pub fn timestamp_utc(&self) -> DateTime<Utc> {
        self.timestamp.and_utc()
    }

    /// Extracts scalar (q5, q95) quantile bounds from the 2-element percentile arrays.
    /// The widest band is selected, with lower bound being the min of p5, upper bound
    /// the max of p95.
    pub fn quantile_bounds(&self) -> (f64, f64) {
        if self.p5.is_empty() || self.p95.is_empty() {
            return (0.0, 0.0);
        }
        let q5 = self.p5.iter().copied().fold(f64::INFINITY, f64::min);
        let q95 = self.p95.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        (q5, q95)
    }
}

/// The list of PV forecast points.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PvForecastData {
    pub pv_forecasts: Vec<PvForecastPoint>,
}

/// PV forecast response body
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PvForecastResponse {
    pub data: PvForecastData,
}

/// Internal enum used to parse a PV forecast HTTP response body.
#[derive(Deserialize)]
#[serde(untagged)]
enum PvForecastApiResponse {
    Success(PvForecastResponse),
    Error { error: String },
}

/// Parses a raw PV forecast response body, mapping a 200 with error body
/// to [`ForecastApiError::Api`]. Kept separate from [`PvForecastApiConnection::fetch`]
/// so the parsing logic is unit-testable without a live HTTP round-trip.
pub fn parse_response(body: &str) -> Result<PvForecastResponse, ForecastApiError> {
    let raw: PvForecastApiResponse = serde_json::from_str(body)
        .map_err(|e| ForecastApiError::Api(format!("failed to parse PV response: {}", e)))?;
    match raw {
        PvForecastApiResponse::Success(r) => Ok(r),
        PvForecastApiResponse::Error { error } => Err(ForecastApiError::Api(error)),
    }
}

#[derive(Clone)]
pub struct PvForecastApiConnection {
    client: ReqwestClient,
    address: String,
    api_key: String,
}

impl PvForecastApiConnection {
    pub fn new() -> Self {
        PvForecastApiConnection {
            client: ReqwestClient::builder()
                .timeout(std::time::Duration::from_secs(
                    CommunityClientConstants.HTTP_REQUEST_TIMEOUT_SEC,
                ))
                .connect_timeout(std::time::Duration::from_secs(
                    CommunityClientConstants.HTTP_CONNECT_TIMEOUT_SEC,
                ))
                .build()
                .expect("Failed to build PV forecast HTTP client"),
            address: CommunityClientConstants.FEDECOM_PV_FORECAST_URL.clone(),
            api_key: CommunityClientConstants.FEDECOM_PV_FORECAST_API_KEY.clone(),
        }
    }

    // Fetch the PV forecast time series for one meter, starting at start_time. Unlike the
    // demand API, the api_key is carried in the request body rather than a header.
    pub async fn fetch(
        &self,
        meter: &str,
        site: &str,
        start_time: DateTime<Utc>,
    ) -> Result<PvForecastResponse, ForecastApiError> {
        let request_params = PvForecastRequestParams {
            meter: meter.to_string(),
            site: site.to_string(),
            start_time: start_time.to_rfc3339_opts(SecondsFormat::Secs, false),
            api_key: self.api_key.clone(),
        };
        let body = self
            .client
            .post(&self.address)
            // NOTE: no `X-API-Key` header here — the key travels inside the JSON body above.
            .json(&request_params)
            .send()
            .await?
            .error_for_status()?
            .text()
            .await?;
        parse_response(&body)
    }
}
