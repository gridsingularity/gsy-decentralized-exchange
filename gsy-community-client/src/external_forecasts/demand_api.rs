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
    ) -> Result<DemandForecastResponse, reqwest::Error> {
        let request_params = DemandForecastRequestParams {
            meter: meter.to_string(),
            site: site.to_string(),
            start_time: start_time.to_rfc3339_opts(SecondsFormat::Secs, false),
        };
        self.client
            .post(&self.address)
            .header("X-API-Key", self.api_key.as_str())
            .json(&request_params)
            .send()
            .await?
            .error_for_status()?
            .json::<DemandForecastResponse>()
            .await
    }
}
