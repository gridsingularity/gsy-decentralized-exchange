pub mod manager;

pub mod demand_api;

pub mod pv_api;

pub mod pv_pricing;

pub mod aic_api;

/// Distinguishes a transport / HTTP-level failure from an API-reported error.
#[derive(Debug)]
pub enum ForecastApiError {
    /// A `reqwest` send / decode error.
    Http(reqwest::Error),
    /// The server reported the failure in its body, either with HTTP 200 or with a
    /// non-2xx status whose body carries the reason.
    Api(String),
}

/// Render a non-2xx forecaster response as one log-friendly line.
///
/// The FEDECOM forecasters put the reason in a JSON `detail` (or `error`) field, e.g.
/// `{"detail":"Invalid site ArenaInnovationCommunity. Choose ..."}`. Calling
/// `error_for_status()` discards the body, so the operator only ever saw
/// "400 Bad Request" with no cause. Falls back to the raw body when it is not the
/// expected shape.
pub fn describe_error_response(status: reqwest::StatusCode, body: &str) -> String {
    let detail = serde_json::from_str::<serde_json::Value>(body)
        .ok()
        .and_then(|parsed| {
            parsed
                .get("detail")
                .or_else(|| parsed.get("error"))
                .and_then(|value| value.as_str().map(str::to_string))
        })
        .unwrap_or_else(|| body.trim().chars().take(200).collect());
    format!("HTTP {}: {}", status, detail)
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
