use anyhow::{anyhow, Result};
use reqwest::Client;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::env;
use std::time::Instant;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
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
#[serde(rename_all = "camelCase")]
struct EwdsQueryResponse<T> {
    #[serde(alias = "request_id")]
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


pub async fn query_via_ewds<T: DeserializeOwned>(
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
