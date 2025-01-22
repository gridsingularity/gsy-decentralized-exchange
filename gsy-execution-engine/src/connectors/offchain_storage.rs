use anyhow::{Result, anyhow};
use reqwest::Client;
use crate::primitives::{
    trades::Trade,
    measurements::Measurement
};

pub async fn fetch_trades_and_measurements_for_timeslot(
    base_url: &str,
    timeslot: &str,
) -> Result<(Vec<Trade>, Vec<Measurement>)> {
    let client = Client::new();

    // TODO: we might have an endpoint like /trades?timeslot=YYYY-MM-DD-HH
    let trades_url = format!("{}/trades?timeslot={}", base_url, timeslot);
    let measurements_url = format!("{}/measurements?timeslot={}", base_url, timeslot);

    // 1) Fetch trades
    let trades_resp = client.get(&trades_url).send().await?;
    if !trades_resp.status().is_success() {
        return Err(anyhow!(
            "Failed to fetch trades for timeslot {}: HTTP {}",
            timeslot,
            trades_resp.status()
        ));
    }
    let trades: Vec<Trade> = trades_resp.json().await?;

    // 2) Fetch measurements
    let measurements_resp = client.get(&measurements_url).send().await?;
    if !measurements_resp.status().is_success() {
        return Err(anyhow!(
            "Failed to fetch measurements for timeslot {}: HTTP {}",
            timeslot,
            measurements_resp.status()
        ));
    }
    let measurements: Vec<Measurement> = measurements_resp.json().await?;

    Ok((trades, measurements))
}