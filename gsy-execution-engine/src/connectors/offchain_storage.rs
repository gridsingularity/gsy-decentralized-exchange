use anyhow::{Result, anyhow};
use tracing::info;
use reqwest::Client;
use reqwest::header::{HeaderMap, HeaderValue};
use gsy_offchain_primitives::db_api_schema::{
    profiles::MeasurementSchema,
    trades::TradeSchema,
};
use gsy_offchain_primitives::constants::GlobalConstants;
use gsy_offchain_primitives::utils::read_env_or;

fn round_down_timeslot(ts: u64) -> u64 {
    (ts / GlobalConstants.TIME_SLOT_SEC) * GlobalConstants.TIME_SLOT_SEC
}

/// Build a reqwest client that sends the `x-api-key` header the off-chain storage now
/// requires. The key comes from the `API_KEY` env var (default `fedecom_user`) and must
/// match the storage service's configured key.
fn authorized_storage_client() -> Client {
    let api_key = read_env_or("API_KEY", "fedecom_user".to_string());
    let mut headers = HeaderMap::new();
    if let Ok(value) = HeaderValue::from_str(&api_key) {
        headers.insert("x-api-key", value);
    }
    Client::builder()
        .default_headers(headers)
        .build()
        .expect("Failed to build off-chain storage HTTP client")
}

pub async fn fetch_trades_and_measurements_for_timeslot(
    base_url: &str,
    timeslot: u64,
    market_duration: u64,
) -> Result<(Vec<TradeSchema>, Vec<MeasurementSchema>)> {
    let client = authorized_storage_client();

    let start_time = round_down_timeslot(timeslot);
    let end_time = start_time + (market_duration.checked_sub(1).unwrap_or(GlobalConstants.TIME_SLOT_SEC));

    let trades_url = format!("{}/trades?start_time={}&end_time={}", base_url, start_time, end_time);
    let measurements_url = format!("{}/measurements?start_time={}&end_time={}", base_url, start_time, end_time);
    info!("Fetching trades for {}", trades_url);
    info!("Fetching measurements for {}", measurements_url);

    // 1) Fetch trades
    let trades_resp = client.get(&trades_url).send().await?;
    if !trades_resp.status().is_success() {
        return Err(anyhow!(
            "Failed to fetch trades for timeslot {}: HTTP {}",
            timeslot,
            trades_resp.status()
        ));
    }
    let trades: Vec<TradeSchema> = trades_resp.json().await?;

    // 2) Fetch measurements
    let measurements_resp = client.get(&measurements_url).send().await?;
    if !measurements_resp.status().is_success() {
        return Err(anyhow!(
            "Failed to fetch measurements for timeslot {}: HTTP {}",
            timeslot,
            measurements_resp.status()
        ));
    }
    let measurements: Vec<MeasurementSchema> = measurements_resp.json().await?;

    Ok((trades, measurements))
}