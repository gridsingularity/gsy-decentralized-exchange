use anyhow::{anyhow, bail, Context, Result};
use chrono::DateTime;
use primitives::MarketTimeSeriesGranularity;
use serde::Deserialize;
use std::collections::HashMap;
use std::fmt;

/// Raw environment variables, deserialized by `envy`. Validated into [`Config`].
#[derive(Deserialize, Debug)]
struct RawConfig {
    #[serde(default = "default_database_url_scheme")]
    database_url_scheme: String,
    #[serde(default = "default_database_host")]
    database_host: String,
    #[serde(default = "default_database_username")]
    database_username: String,
    #[serde(default = "default_database_password")]
    database_password: String,
    #[serde(default = "default_database_name")]
    database_name: String,
    analytics_results_database_name: Option<String>,
    #[serde(default = "default_results_collection")]
    analytics_results_collection: String,
    #[serde(default = "default_interval_seconds")]
    analytics_interval_seconds: u64,
    #[serde(default = "default_lookback_hours")]
    analytics_lookback_hours: u64,
    #[serde(default = "default_settlement_delay_minutes")]
    analytics_settlement_delay_minutes: u64,
    analytics_backfill_from: Option<String>,
    #[serde(default = "default_enabled_kpis")]
    analytics_enabled_kpis: String,
    #[serde(default = "default_granularities")]
    analytics_granularities: String,
    // Kept as strings so that empty values (e.g. `${VAR:-}` in compose) mean "unset".
    analytics_grid_tariff_eur_per_kwh: Option<String>,
    analytics_grid_tariff_overrides: Option<String>,
    #[serde(default = "default_api_host")]
    analytics_api_host: String,
    #[serde(default = "default_api_port")]
    analytics_api_port: u16,
}

fn default_database_url_scheme() -> String {
    "mongodb".to_string()
}
fn default_database_host() -> String {
    "mongodb".to_string()
}
fn default_database_username() -> String {
    "gsy".to_string()
}
fn default_database_password() -> String {
    "gsy".to_string()
}
fn default_database_name() -> String {
    "offchain_storage".to_string()
}
fn default_results_collection() -> String {
    "kpi_results".to_string()
}
fn default_interval_seconds() -> u64 {
    900
} // 15 minutes
fn default_lookback_hours() -> u64 {
    48
}
fn default_settlement_delay_minutes() -> u64 {
    15
}
fn default_enabled_kpis() -> String {
    "procurement_cost_per_kwh".to_string()
}
fn default_api_host() -> String {
    "0.0.0.0".to_string()
}
fn default_api_port() -> u16 {
    8081
}
fn default_granularities() -> String {
    "15min".to_string()
}

/// Database password, hidden from `Debug` output.
#[derive(Clone, PartialEq, Eq)]
pub struct Password(pub String);

impl fmt::Debug for Password {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("\"***\"")
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct DatabaseConfig {
    pub url_scheme: String,
    pub host: String,
    pub username: String,
    pub password: Password,
    pub name: String,
}

impl DatabaseConfig {
    /// Same format as `gsy-offchain-storage` (`Settings::get_connection_string`).
    pub fn connection_string(&self) -> String {
        format!(
            "{}://{}:{}@{}/?retryWrites=true&w=majority",
            self.url_scheme, self.username, self.password.0, self.host
        )
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct TariffConfig {
    /// Global flat utility tariff in EUR/kWh.
    pub default_eur_per_kwh: Option<f64>,
    /// Per-community tariffs in EUR/kWh, keyed by community id.
    pub overrides: HashMap<String, f64>,
}

impl TariffConfig {
    pub fn is_empty(&self) -> bool {
        self.default_eur_per_kwh.is_none() && self.overrides.is_empty()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Config {
    pub database: DatabaseConfig,
    pub results_database_name: String,
    pub results_collection: String,
    pub interval_seconds: u64,
    pub lookback_hours: u64,
    pub settlement_delay_minutes: u64,
    /// Unix seconds.
    pub backfill_from: Option<i64>,
    pub enabled_kpis: Vec<String>,
    pub granularities: Vec<MarketTimeSeriesGranularity>,
    pub tariffs: TariffConfig,
    pub api_host: String,
    pub api_port: u16,
}

impl Config {
    /// `host:port` the HTTP API binds to.
    pub fn api_address(&self) -> String {
        format!("{}:{}", self.api_host, self.api_port)
    }

    pub fn from_env() -> Result<Self> {
        Self::from_vars(std::env::vars())
    }

    pub fn from_vars<I>(vars: I) -> Result<Self>
    where
        I: IntoIterator<Item = (String, String)>,
    {
        let raw: RawConfig = envy::from_iter(vars).context("Invalid analytics engine config")?;

        if raw.analytics_interval_seconds == 0 {
            bail!("ANALYTICS_INTERVAL_SECONDS must be greater than 0");
        }

        let results_database_name =
            non_empty(raw.analytics_results_database_name).unwrap_or(raw.database_name.clone());

        Ok(Config {
            database: DatabaseConfig {
                url_scheme: raw.database_url_scheme,
                host: raw.database_host,
                username: raw.database_username,
                password: Password(raw.database_password),
                name: raw.database_name,
            },
            results_database_name,
            results_collection: raw.analytics_results_collection,
            interval_seconds: raw.analytics_interval_seconds,
            lookback_hours: raw.analytics_lookback_hours,
            settlement_delay_minutes: raw.analytics_settlement_delay_minutes,
            backfill_from: non_empty(raw.analytics_backfill_from)
                .map(|value| parse_timestamp(&value))
                .transpose()?,
            enabled_kpis: parse_list(&raw.analytics_enabled_kpis),
            granularities: parse_granularities(&raw.analytics_granularities)?,
            tariffs: TariffConfig {
                default_eur_per_kwh: non_empty(raw.analytics_grid_tariff_eur_per_kwh)
                    .map(|value| parse_tariff(&value))
                    .transpose()
                    .context("Invalid ANALYTICS_GRID_TARIFF_EUR_PER_KWH")?,
                overrides: non_empty(raw.analytics_grid_tariff_overrides)
                    .map(|value| parse_tariff_overrides(&value))
                    .transpose()?
                    .unwrap_or_default(),
            },
            api_host: raw.analytics_api_host,
            api_port: raw.analytics_api_port,
        })
    }
}

fn non_empty(value: Option<String>) -> Option<String> {
    value
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
}

fn parse_list(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(str::to_string)
        .collect()
}

fn parse_granularities(value: &str) -> Result<Vec<MarketTimeSeriesGranularity>> {
    let granularities = parse_list(value)
        .iter()
        .map(|item| match item.as_str() {
            "15min" => Ok(MarketTimeSeriesGranularity::FifteenMinutes),
            "1h" | "1d" => Err(anyhow!(
                "Granularity '{}' is not supported yet. Only 15min is supported",
                item
            )),
            other => Err(anyhow!("Unknown granularity '{}'. Expected 15min", other)),
        })
        .collect::<Result<Vec<_>>>()?;
    if granularities.is_empty() {
        bail!("ANALYTICS_GRANULARITIES must contain at least one granularity");
    }
    Ok(granularities)
}

fn parse_tariff(value: &str) -> Result<f64> {
    let tariff: f64 = value
        .trim()
        .parse()
        .with_context(|| format!("'{}' is not a number", value))?;
    if !tariff.is_finite() || tariff < 0.0 {
        bail!("Tariff must be a non-negative number, got '{}'", value);
    }
    Ok(tariff)
}

/// Parses `communityId=0.28,otherId=0.25`.
fn parse_tariff_overrides(value: &str) -> Result<HashMap<String, f64>> {
    parse_list(value)
        .iter()
        .map(|entry| {
            let (community_id, tariff) = entry.split_once('=').ok_or_else(|| {
                anyhow!(
                    "Invalid ANALYTICS_GRID_TARIFF_OVERRIDES entry '{}'. Expected communityId=tariff",
                    entry
                )
            })?;
            let community_id = community_id.trim();
            if community_id.is_empty() {
                bail!("Missing community id in ANALYTICS_GRID_TARIFF_OVERRIDES entry '{}'", entry);
            }
            let tariff = parse_tariff(tariff)
                .with_context(|| format!("Invalid tariff for community '{}'", community_id))?;
            Ok((community_id.to_string(), tariff))
        })
        .collect()
}

/// Accepts unix seconds or an RFC 3339 timestamp.
fn parse_timestamp(value: &str) -> Result<i64> {
    if let Ok(seconds) = value.parse::<i64>() {
        return Ok(seconds);
    }
    DateTime::parse_from_rfc3339(value)
        .map(|timestamp| timestamp.timestamp())
        .with_context(|| {
            format!(
                "Invalid ANALYTICS_BACKFILL_FROM '{}'. Expected unix seconds or RFC 3339",
                value
            )
        })
}
