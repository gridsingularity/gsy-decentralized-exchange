#![allow(non_snake_case, non_upper_case_globals)]

use gsy_offchain_primitives::utils::read_env_or;
use once_cell::sync::Lazy;

/// Reserved community name identifying the single inter-community market per timeslot.
pub const INTER_COMMUNITY_MARKET_NAME: &str = "INTER_COMMUNITY";

pub struct Constants {
    pub FEDECOM_ONTOLOGY_URL: String,
    pub FEDECOM_ONTOLOGY_ASSETS_URL: String,
    pub FEDECOM_INFLUX_DB_URL: String,
    pub FEDECOM_INFLUX_DB_ORG: String,
    pub FEDECOM_INFLUX_DB_TOKEN: String,
    pub FEDECOM_DEMAND_FORECAST_URL: String,
    pub FEDECOM_DEMAND_FORECAST_API_KEY: String,
    pub FEDECOM_PV_FORECAST_URL: String,
    pub FEDECOM_PV_FORECAST_API_KEY: String,
    /// Endpoint of the temporary AIC demand forecaster.
    pub FEDECOM_AIC_FORECAST_URL: String,
    pub FEDECOM_AIC_FORECAST_API_KEY: String,
    /// How often, in seconds, bids and offers are resubmitted within a market slot.
    pub ORDER_RESUBMISSION_INTERVAL_SEC: u64,
    /// Lower bound of the order price range, in currency units per kWh.
    pub MIN_ORDER_RATE: f64,
    /// Upper bound of the order price range, in currency units per kWh.
    pub MAX_ORDER_RATE: f64,
    /// Risk aversion for percentile-based PV offer-energy commitment. 0.0 commits
    /// the point forecast; 1.0 (default) commits the conservative p5 quantile.
    pub PV_RISK_AVERSION: f64,
    /// Normalizer for the relative p5..p95 spread when deriving the confidence scalar.
    pub PV_SPREAD_NORM: f64,
    /// Lower clamp for the per-slot confidence scalar.
    pub PV_MIN_CONFIDENCE: f64,
    /// Floor (kWh) for the denominator of the relative spread; avoids divide-by-zero
    /// at night / near-zero output.
    pub PV_MIN_FORECAST_KWH: f64,
    /// Weight for confidence-based offer rate modulation. 0.0 disables confidence-based
    /// rate modulation entirely (offers ramp down to MIN_ORDER_RATE as before); 1.0 lets
    /// a zero-confidence offer ramp no lower than MAX_ORDER_RATE.
    pub PV_PRICE_CONFIDENCE_WEIGHT: f64,
    /// Overall request timeout (in seconds) applied to every external HTTP call.
    /// Keeps a slow/hung endpoint from blocking indefinitely. Set above the demand
    /// forecaster's observed ~30s response latency so valid slow responses are not cut off.
    pub HTTP_REQUEST_TIMEOUT_SEC: u64,
    /// Overall request timeout (in seconds) applied to the PV forecaster HTTP call only.
    /// The PV forecaster can take up to ~2 minutes to respond, far longer than the demand
    /// forecaster, so it gets a dedicated, larger timeout.
    pub PV_HTTP_REQUEST_TIMEOUT_SEC: u64,
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
            FEDECOM_PV_FORECAST_URL: read_env_or(
                "FEDECOM_PV_FORECAST_URL",
                "https://fedecom.imp.bg.ac.rs/pv_forecaster_aic/forecast/pv_aic".to_string(),
            ),
            FEDECOM_PV_FORECAST_API_KEY: read_env_or(
                "FEDECOM_PV_FORECAST_API_KEY",
                "fedecom_user".to_string(),
            ),
            // TODO(B3): finalize AIC endpoint URL pending Eleni. Defaults to empty until then.
            FEDECOM_AIC_FORECAST_URL: read_env_or("FEDECOM_AIC_FORECAST_URL", "".to_string()),
            FEDECOM_AIC_FORECAST_API_KEY: read_env_or(
                "FEDECOM_AIC_FORECAST_API_KEY",
                "fedecom_user".to_string(),
            ),
            ORDER_RESUBMISSION_INTERVAL_SEC: read_env_or("ORDER_RESUBMISSION_INTERVAL_SEC", 300),
            MIN_ORDER_RATE: read_env_or("MIN_ORDER_RATE", 0.07),
            MAX_ORDER_RATE: read_env_or("MAX_ORDER_RATE", 0.30),
            PV_RISK_AVERSION: read_env_or("PV_RISK_AVERSION", 1.0),
            PV_SPREAD_NORM: read_env_or("PV_SPREAD_NORM", 1.0),
            PV_MIN_CONFIDENCE: read_env_or("PV_MIN_CONFIDENCE", 0.1),
            PV_MIN_FORECAST_KWH: read_env_or("PV_MIN_FORECAST_KWH", 0.05),
            PV_PRICE_CONFIDENCE_WEIGHT: read_env_or("PV_PRICE_CONFIDENCE_WEIGHT", 0.5),
            HTTP_REQUEST_TIMEOUT_SEC: read_env_or("HTTP_REQUEST_TIMEOUT_SEC", 60u64),
            PV_HTTP_REQUEST_TIMEOUT_SEC: read_env_or("PV_HTTP_REQUEST_TIMEOUT_SEC", 150u64),
            HTTP_CONNECT_TIMEOUT_SEC: read_env_or("HTTP_CONNECT_TIMEOUT_SEC", 10u64),
        }
    }
}

pub static CommunityClientConstants: Lazy<Constants> = Lazy::new(Constants::new);
