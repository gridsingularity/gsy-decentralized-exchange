use crate::constants::CommunityClientConstants;
use crate::external_forecasts::demand_api::{
    DemandForecastError, DemandForecastFuture, DemandForecastResponse, DemandForecaster,
};
use chrono::{DateTime, SecondsFormat, Utc};
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

#[derive(Deserialize)]
#[serde(untagged)]
enum PvForecastApiResponse {
    Success(DemandForecastResponse),
    Error { error: String },
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
    ) -> Result<DemandForecastResponse, DemandForecastError> {
        let request_params = PvForecastRequestParams {
            meter: meter.to_string(),
            site: site.to_string(),
            start_time: start_time.to_rfc3339_opts(SecondsFormat::Secs, false),
            api_key: self.api_key.clone(),
        };
        let raw = self
            .client
            .post(&self.address)
            // NOTE: no `X-API-Key` header here — the key travels inside the JSON body above.
            .json(&request_params)
            .send()
            .await?
            .error_for_status()?
            .json::<PvForecastApiResponse>()
            .await?;
        match raw {
            PvForecastApiResponse::Success(r) => Ok(r),
            PvForecastApiResponse::Error { error } => Err(DemandForecastError::Api(error)),
        }
    }
}

impl DemandForecaster for PvForecastApiConnection {
    fn fetch<'a>(
        &'a self,
        meter: &'a str,
        site: &'a str,
        start_time: DateTime<Utc>,
    ) -> DemandForecastFuture<'a> {
        Box::pin(PvForecastApiConnection::fetch(
            self, meter, site, start_time,
        ))
    }
}
