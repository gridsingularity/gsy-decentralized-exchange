use anyhow::{anyhow, Result};
use primitives::constants::GLOBAL_CONSTANTS;
use primitives::db_api_schema::{
    profiles::{MeasurementPointSchema, MeasurementSchema, TimeseriesSchema},
    trades::DbTradeSchema,
};
use primitives::ewds::dto::EwdsTradeDto;
use primitives::ewds::{EwdsClient, EwdsOperation};
use primitives::utils::timestamp_to_string_with_padding;
use reqwest::Client;
use std::collections::HashMap;
use std::env;
use tracing::info;

fn round_down_timeslot(ts: u64) -> u64 {
    (ts / GLOBAL_CONSTANTS.time_slot_sec) * GLOBAL_CONSTANTS.time_slot_sec
}

pub async fn fetch_trades_and_measurements_for_timeslot(
    base_url: &str,
    timeslot: u64,
    market_duration: u64,
) -> Result<(Vec<DbTradeSchema>, Vec<MeasurementSchema>)> {
    let start_time = round_down_timeslot(timeslot);
    let end_time = start_time
        + (market_duration
            .checked_sub(1)
            .unwrap_or(GLOBAL_CONSTANTS.time_slot_sec));

    if env::var("OFFCHAIN_STORAGE_TRANSPORT")
        .map(|value| value.eq_ignore_ascii_case("ewds"))
        .unwrap_or(false)
    {
        info!("Fetching trades/measurements via EWDS transport");
        return fetch_trades_and_measurements_via_ewds(start_time, end_time).await;
    }

    let client = Client::new();

    let trades_url = format!(
        "{}/trades?start_time={}&end_time={}",
        base_url, start_time, end_time
    );
    info!("Fetching trades for {}", trades_url);

    let trades_resp = client.get(&trades_url).send().await?;
    if !trades_resp.status().is_success() {
        return Err(anyhow!(
            "Failed to fetch trades for timeslot {}: HTTP {}",
            timeslot,
            trades_resp.status()
        ));
    }
    let trades: Vec<EwdsTradeDto> = trades_resp.json().await?;
    let trades_db: Vec<DbTradeSchema> = trades
        .into_iter()
        .map(|o| DbTradeSchema::try_from(o).expect("invalid EwdsTradeDto"))
        .collect();

    let measurements =
        fetch_measurements_from_timeseries(&client, base_url, start_time, end_time).await?;

    Ok((trades_db, measurements))
}

async fn fetch_measurements_from_timeseries(
    client: &Client,
    base_url: &str,
    start_time: u64,
    end_time: u64,
) -> Result<Vec<MeasurementSchema>> {
    let measurement_points_url = format!("{}/measurement-points?type=Measurement", base_url);
    let timeseries_url = format!(
        "{}/timeseries?start_time={}&end_time={}",
        base_url,
        timestamp_to_string_with_padding(start_time),
        timestamp_to_string_with_padding(end_time)
    );
    info!("Fetching measurement points for {}", measurement_points_url);
    info!("Fetching timeseries for {}", timeseries_url);

    let points_resp = client.get(&measurement_points_url).send().await?;
    if !points_resp.status().is_success() {
        return Err(anyhow!(
            "Failed to fetch measurement points for timeslot {}: HTTP {}",
            start_time,
            points_resp.status()
        ));
    }
    let points = points_resp.json::<Vec<MeasurementPointSchema>>().await?;
    let points_by_id = points
        .into_iter()
        .map(|point| (point.measurement_id.clone(), point))
        .collect::<HashMap<_, _>>();

    let timeseries_resp = client.get(&timeseries_url).send().await?;
    if !timeseries_resp.status().is_success() {
        return Err(anyhow!(
            "Failed to fetch timeseries for timeslot {}: HTTP {}",
            start_time,
            timeseries_resp.status()
        ));
    }
    let timeseries = timeseries_resp.json::<Vec<TimeseriesSchema>>().await?;

    Ok(timeseries
        .into_iter()
        .filter_map(|value| {
            let point = points_by_id.get(&value.measurement_point)?;
            let time_slot = parse_timeseries_timestamp(value.timestamp.as_str())?;
            Some(MeasurementSchema {
                facility_id: point.asset_name.clone(),
                community_uuid: point.datasource_name.clone().unwrap_or_default(),
                time_slot,
                creation_time: time_slot,
                energy_kwh: value.value,
            })
        })
        .collect())
}
fn parse_timeseries_timestamp(timestamp: &str) -> Option<u64> {
    timestamp.parse::<u64>().ok()
}

async fn fetch_trades_and_measurements_via_ewds(
    start_time: u64,
    end_time: u64,
) -> Result<(Vec<DbTradeSchema>, Vec<MeasurementSchema>)> {
    let query = serde_json::json!({
        "startTime": start_time,
        "endTime": end_time
    });
    let ewds_client = EwdsClient::from_env(
        "EWDS_EXECUTION_ENGINE_CLIENT_ID",
        "gsyexecutionengine",
        8_000,
    );

    let trades: Vec<EwdsTradeDto> = ewds_client
        .query(EwdsOperation::TradesQuery, query.clone())
        .await?;

    let trades_db: Vec<DbTradeSchema> = trades
        .into_iter()
        .map(|o| DbTradeSchema::try_from(o).expect("invalid EwdsTradeDto"))
        .collect();

    let measurements: Vec<MeasurementSchema> = ewds_client
        .query(EwdsOperation::MeasurementsQuery, query)
        .await?;

    Ok((trades_db, measurements))
}
