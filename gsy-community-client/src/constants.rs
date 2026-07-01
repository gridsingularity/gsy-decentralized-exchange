#![allow(non_snake_case, non_upper_case_globals)]

use gsy_offchain_primitives::utils::read_env_or;
use once_cell::sync::Lazy;

pub struct Constants {
    pub FEDECOM_ONTOLOGY_URL: String,
    pub FEDECOM_ONTOLOGY_ASSETS_URL: String,
    pub FEDECOM_INFLUX_DB_URL: String,
    pub FEDECOM_INFLUX_DB_ORG: String,
    pub FEDECOM_INFLUX_DB_TOKEN: String,
    pub FEDECOM_DEMAND_FORECAST_URL: String,
    pub FEDECOM_DEMAND_FORECAST_API_KEY: String,
    /// How often, in seconds, bids and offers are resubmitted within a market slot.
    pub ORDER_RESUBMISSION_INTERVAL_SEC: u64,
    /// Lower bound of the order price range, in currency units per kWh.
    pub MIN_ORDER_RATE: f64,
    /// Upper bound of the order price range, in currency units per kWh.
    pub MAX_ORDER_RATE: f64,
    /// Overall request timeout (in seconds) applied to every external HTTP call.
    /// Keeps a slow/hung endpoint from blocking indefinitely. Set above the demand
    /// forecaster's observed ~30s response latency so valid slow responses are not cut off.
    pub HTTP_REQUEST_TIMEOUT_SEC: u64,
    /// TCP connect timeout (in seconds) applied to every external HTTP call.
    pub HTTP_CONNECT_TIMEOUT_SEC: u64,
}

impl Constants {
    fn new() -> Self {
        Self {
            FEDECOM_ONTOLOGY_URL: read_env_or(
                "FEDECOM_ONTOLOGY_URL",
                "https://fedecom.tekniker.es/services/queries/get_lecs_buildings".to_string(),
            ),
            FEDECOM_ONTOLOGY_ASSETS_URL: read_env_or(
                "FEDECOM_ONTOLOGY_ASSETS_URL",
                "https://fedecom.tekniker.es/services/queries/get_assets".to_string(),
            ),
            FEDECOM_INFLUX_DB_URL: read_env_or(
                "FEDECOM_INFLUX_DB_URL",
                "https://fedecom.imp.bg.ac.rs/influxdb/api/v2/query".to_string(),
            ),
            FEDECOM_INFLUX_DB_ORG: read_env_or("FEDECOM_INFLUX_DB_ORG", "fedecom".to_string()),
            // Token is mandatory
            FEDECOM_INFLUX_DB_TOKEN: read_env_or("FEDECOM_INFLUX_DB_TOKEN", "".to_string()),
            FEDECOM_DEMAND_FORECAST_URL: read_env_or(
                "FEDECOM_DEMAND_FORECAST_URL",
                "https://fedecom.imp.bg.ac.rs/demand_forecaster/forecast/gd_lic".to_string(),
            ),
            FEDECOM_DEMAND_FORECAST_API_KEY: read_env_or(
                "FEDECOM_DEMAND_FORECAST_API_KEY",
                "fedecom_user".to_string(),
            ),
            ORDER_RESUBMISSION_INTERVAL_SEC: read_env_or("ORDER_RESUBMISSION_INTERVAL_SEC", 300),
            MIN_ORDER_RATE: read_env_or("MIN_ORDER_RATE", 0.07),
            MAX_ORDER_RATE: read_env_or("MAX_ORDER_RATE", 0.30),
            HTTP_REQUEST_TIMEOUT_SEC: read_env_or("HTTP_REQUEST_TIMEOUT_SEC", 60u64),
            HTTP_CONNECT_TIMEOUT_SEC: read_env_or("HTTP_CONNECT_TIMEOUT_SEC", 10u64),
        }
    }
}

pub static CommunityClientConstants: Lazy<Constants> = Lazy::new(Constants::new);
