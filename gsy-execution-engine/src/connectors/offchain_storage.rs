use anyhow::{Result, anyhow};
use reqwest::Client;
use gsy_offchain_primitives::db_api_schema::{
    profiles::MeasurementSchema, 
    trades::TradeSchema,
};

fn round_down_timeslot(ts: u64) -> u64 {
    (ts / 900) * 900
}

pub async fn fetch_trades_and_measurements_for_timeslot(
    base_url: &str,
    timeslot: u64,
    market_duration: u64,
) -> Result<(Vec<TradeSchema>, Vec<MeasurementSchema>)> {
    let client = Client::new();

    let start_time = round_down_timeslot(timeslot);
    let end_time = start_time + (market_duration.checked_sub(1).unwrap_or(60));

    let trades_url = format!("{}/trades?start_time={}&end_time={}", base_url, start_time, end_time);
    let measurements_url = format!("{}/measurements?start_time={}&end_time={}", base_url, start_time, end_time);

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