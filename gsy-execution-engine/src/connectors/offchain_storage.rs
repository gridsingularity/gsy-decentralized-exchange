use anyhow::{anyhow, Result};
use gsy_offchain_primitives::constants::GLOBAL_CONSTANTS;
use gsy_offchain_primitives::db_api_schema::{profiles::MeasurementSchema, trades::TradeSchema};
use reqwest::Client;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::env;
use std::time::Instant;
use tracing::info;

fn round_down_timeslot(ts: u64) -> u64 {
    (ts / GLOBAL_CONSTANTS.time_slot_sec) * GLOBAL_CONSTANTS.time_slot_sec
}

pub async fn fetch_trades_and_measurements_for_timeslot(
    base_url: &str,
    timeslot: u64,
    market_duration: u64,
) -> Result<(Vec<TradeSchema>, Vec<MeasurementSchema>)> {
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
    let measurements_url = format!(
        "{}/measurements?start_time={}&end_time={}",
        base_url, start_time, end_time
    );
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

#[derive(Serialize)]
struct EwdsRequestEnvelope {
    request_id: String,
    operation: String,
    payload: serde_json::Value,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct EwdsSendMessageDto {
    fqcn: String,
    topic_name: String,
    topic_version: String,
    topic_owner: String,
    transaction_id: String,
    payload: String,
    anonymous_recipient: Vec<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct EwdsMessageDto {
    payload: String,
}

#[derive(Deserialize)]
struct EwdsQueryResponse<T> {
    request_id: String,
    success: bool,
    data: Option<Vec<T>>,
    error: Option<EwdsErrorPayload>,
}

#[derive(Deserialize)]
struct EwdsErrorPayload {
    code: String,
    message: String,
}

async fn fetch_trades_and_measurements_via_ewds(
    start_time: u64,
    end_time: u64,
) -> Result<(Vec<TradeSchema>, Vec<MeasurementSchema>)> {
    let query = serde_json::json!({
        "start_time": start_time,
        "end_time": end_time
    });

    let trades: Vec<TradeSchema> = query_via_ewds(
        "trades.query",
        query.clone(),
        "EWDS_TRADES_REQUEST_TOPIC",
        "tradesQuery",
        "EWDS_TRADES_RESPONSE_TOPIC",
        "tradesQueryResponse",
    )
    .await?;

    let measurements: Vec<MeasurementSchema> = query_via_ewds(
        "measurements.query",
        query,
        "EWDS_MEASUREMENTS_REQUEST_TOPIC",
        "measurementsQuery",
        "EWDS_MEASUREMENTS_RESPONSE_TOPIC",
        "measurementsQueryResponse",
    )
    .await?;

    Ok((trades, measurements))
}

async fn query_via_ewds<T: DeserializeOwned>(
    operation: &str,
    query_payload: serde_json::Value,
    request_topic_env: &str,
    request_topic_default: &str,
    response_topic_env: &str,
    response_topic_default: &str,
) -> Result<Vec<T>> {
    let gateway_base =
        env::var("EWDS_GATEWAY_URL").unwrap_or_else(|_| "http://ewds-gateway-api:3333".to_string());
    let request_fqcn = env_var("EWDS_REQUEST_PUBLISH_FQCN")
        .or_else(|| env_var("EWDS_REQUEST_FQCN"))
        .unwrap_or_else(|| "gsy.intelligent.requests.pub".to_string());
    let response_fqcn = env_var("EWDS_RESPONSE_SUBSCRIBE_FQCN")
        .or_else(|| env_var("EWDS_RESPONSE_FQCN"))
        .unwrap_or_else(|| "gsy.intelligent.responses.sub".to_string());
    let topic_owner = env::var("EWDS_TOPIC_OWNER")
        .unwrap_or_else(|_| "integration.apps.intelligent.auth.ewc".to_string());
    let topic_version = env::var("EWDS_TOPIC_VERSION").unwrap_or_else(|_| "1.0.0".to_string());
    let response_client_id = env_var("EWDS_EXECUTION_ENGINE_CLIENT_ID")
        .or_else(|| env_var("EWDS_RESPONSE_CLIENT_ID"))
        .unwrap_or_else(|| "gsyexecutionengine".to_string());
    let request_topic =
        env::var(request_topic_env).unwrap_or_else(|_| request_topic_default.to_string());
    let response_topic =
        env::var(response_topic_env).unwrap_or_else(|_| response_topic_default.to_string());

    let timeout_ms = env::var("EWDS_RESPONSE_TIMEOUT_MS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(8_000);
    let poll_interval_ms = env::var("EWDS_RESPONSE_POLL_INTERVAL_MS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(400);

    let request_id = format!(
        "{}-{}-{}",
        operation.replace('.', "-"),
        chrono::Utc::now().timestamp_millis(),
        std::process::id()
    );

    let envelope = EwdsRequestEnvelope {
        request_id: request_id.clone(),
        operation: operation.to_string(),
        payload: query_payload,
    };

    let send_message_body = EwdsSendMessageDto {
        fqcn: request_fqcn,
        topic_name: request_topic,
        topic_version,
        topic_owner: topic_owner.clone(),
        transaction_id: request_id.clone(),
        payload: serde_json::to_string(&envelope)?,
        anonymous_recipient: Vec::new(),
    };

    let client = Client::new();
    let post_url = format!("{}/api/v2/messages", gateway_base.trim_end_matches('/'));
    let send_response = client
        .post(post_url)
        .json(&send_message_body)
        .send()
        .await?;
    let send_status = send_response.status();
    if !send_status.is_success() {
        let body = send_response.text().await.unwrap_or_default();
        return Err(anyhow!(
            "EWDS message send failed for {}: HTTP {}{}",
            operation,
            send_status,
            format_response_body(&body)
        ));
    }

    let started = Instant::now();
    let get_url = format!("{}/api/v2/messages", gateway_base.trim_end_matches('/'));
    let poll_client_id = client_id_for_suffix(response_client_id.as_str(), response_topic.as_str());
    loop {
        if started.elapsed().as_millis() as u64 > timeout_ms {
            return Err(anyhow!(
                "EWDS timeout waiting for {} response (request_id={})",
                operation,
                request_id
            ));
        }

        let response = client
            .get(get_url.as_str())
            .query(&[
                ("fqcn", response_fqcn.as_str()),
                ("amount", "100"),
                ("topicName", response_topic.as_str()),
                ("topicOwner", topic_owner.as_str()),
                ("clientId", poll_client_id.as_str()),
            ])
            .send()
            .await?;

        let status = response.status();
        if status.is_success() {
            let messages = response
                .json::<Vec<EwdsMessageDto>>()
                .await
                .unwrap_or_default();
            for message in messages {
                let parsed = serde_json::from_str::<EwdsQueryResponse<T>>(&message.payload);
                if let Ok(parsed_payload) = parsed {
                    if parsed_payload.request_id == request_id {
                        if !parsed_payload.success {
                            let error_message = parsed_payload
                                .error
                                .map(|error| format!("{}: {}", error.code, error.message))
                                .unwrap_or_else(|| "Unknown EWDS error".to_string());
                            return Err(anyhow!(
                                "EWDS {} returned error (request_id={}): {}",
                                operation,
                                request_id,
                                error_message
                            ));
                        }
                        return Ok(parsed_payload.data.unwrap_or_default());
                    }
                }
            }
        } else {
            let body = response.text().await.unwrap_or_default();
            return Err(anyhow!(
                "EWDS response poll failed for {} (request_id={}): HTTP {}{}",
                operation,
                request_id,
                status,
                format_response_body(&body)
            ));
        }

        tokio::time::sleep(std::time::Duration::from_millis(poll_interval_ms)).await;
    }
}

fn format_response_body(body: &str) -> String {
    let compact = body.split_whitespace().collect::<Vec<_>>().join(" ");
    if compact.is_empty() {
        return String::new();
    }

    let max_chars = 1_024;
    let truncated = compact.chars().take(max_chars).collect::<String>();
    if compact.chars().count() > max_chars {
        format!(": {}...", truncated)
    } else {
        format!(": {}", truncated)
    }
}

fn env_var(key: &str) -> Option<String> {
    env::var(key)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn client_id_for_suffix(base: &str, suffix: &str) -> String {
    let mut value = String::with_capacity(base.len() + suffix.len());
    value.extend(base.chars().filter(|ch| ch.is_ascii_alphanumeric()));
    value.extend(suffix.chars().filter(|ch| ch.is_ascii_alphanumeric()));

    if value.is_empty() {
        "gsyexecutionengine".to_string()
    } else {
        value
    }
}
