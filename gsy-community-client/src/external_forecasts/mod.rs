pub mod manager;

pub mod demand_api;

pub mod pv_api;

pub mod pv_pricing;

pub mod aic_api;

/// Distinguishes a transport / HTTP-level failure from an API-reported error.
#[derive(Debug)]
pub enum ForecastApiError {
    /// A `reqwest` send / status / decode error.
    Http(reqwest::Error),
    /// The server returned HTTP 200 but with an error body.
    Api(String),
}

impl std::fmt::Display for ForecastApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ForecastApiError::Http(e) => write!(f, "HTTP error: {}", e),
            ForecastApiError::Api(msg) => write!(f, "API-reported error: {}", msg),
        }
    }
}

impl std::error::Error for ForecastApiError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            ForecastApiError::Http(e) => Some(e),
            ForecastApiError::Api(_) => None,
        }
    }
}

impl From<reqwest::Error> for ForecastApiError {
    fn from(e: reqwest::Error) -> Self {
        ForecastApiError::Http(e)
    }
}
