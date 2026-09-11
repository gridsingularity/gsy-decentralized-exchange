use crate::ewds::dto::EwdsClearingResultDto;
use crate::ewds::{EwdsClient, EwdsOperation};
use anyhow::{anyhow, Result};
use reqwest::Client;
use std::env;
use tracing::info;

pub async fn fetch_clearing_results(
    consumer_client_id_env: &str,
    consumer_client_id_default: &str,
    market_id: &str,
) -> Result<Vec<EwdsClearingResultDto>> {
    let clearing_result: Vec<EwdsClearingResultDto> = if env::var("OFFCHAIN_STORAGE_TRANSPORT")
        .map(|value| value.eq_ignore_ascii_case("ewds"))
        .unwrap_or(false)
    {
        info!("Fetching clearing_result via EWDS transport");
        let ewds_client =
            EwdsClient::from_env(consumer_client_id_env, consumer_client_id_default, 8_000);

        let clearing_result_resp: Vec<EwdsClearingResultDto> = ewds_client
            .query(
                EwdsOperation::ClearingResultsQuery,
                serde_json::json!({"market_id": market_id}),
            )
            .await?;
        clearing_result_resp
    } else {
        let client = Client::new();
        let offchain_url = env::var("OFFCHAIN_STORAGE_URL")
            .unwrap_or("http://gsy-offchain-storage:8080".to_string());
        let ids_url = format!("{}/clearing-results", offchain_url);

        let clearing_result_resp = client
            .get(&ids_url)
            .query(&[("market_id", market_id)])
            .send()
            .await?;
        if !clearing_result_resp.status().is_success() {
            return Err(anyhow!(
                "Failed to fetch clearing_results. HTTP {}",
                clearing_result_resp.status()
            ));
        }
        clearing_result_resp.json().await?
    };

    Ok(clearing_result)
}
