use crate::ewds::{EwdsClient, EwdsOperation};
use crate::db_api_schema::ids::IdMappingSchema;
use anyhow::{anyhow, Result};
use reqwest::Client;
use std::env;
use tracing::info;

pub async fn fetch_onchain_id(
    consumer_client_id_env: &str,
    consumer_client_id_default: &str,
    offchain_id: &str,
) -> Result<String> {
    let onchain_id: String = if env::var("OFFCHAIN_STORAGE_TRANSPORT")
        .map(|value| value.eq_ignore_ascii_case("ewds"))
        .unwrap_or(false)
    {
        info!("Fetching onchain_id via EWDS transport");
        let ewds_client =
            EwdsClient::from_env(consumer_client_id_env, consumer_client_id_default, 8_000);

        let mut resp: Vec<IdMappingSchema> = ewds_client
            .query(EwdsOperation::IdsQuery, serde_json::json!({"offchain_id": offchain_id}))
            .await?;
        let mapping = resp
            .pop()
            .ok_or_else(|| anyhow!("No id mapping returned for offchain_id {}", offchain_id))?;
        mapping.onchain_id
    } else {
        let client = Client::new();
        let offchain_url = env::var("OFFCHAIN_STORAGE_URL")
            .unwrap_or("http://gsy-offchain-storage:8080".to_string());
        let ids_url = format!("{}/ids", offchain_url);

        let ids_resp = client
            .post(&ids_url)
            .query(&[("offchain_id", offchain_id)])
            .send()
            .await?;
        if !ids_resp.status().is_success() {
            return Err(anyhow!(
                "Failed to fetch onchain_id. HTTP {}",
                ids_resp.status()
            ));
        }
        let mapping: IdMappingSchema = ids_resp.json().await?;
        mapping.onchain_id
    };

    Ok(onchain_id)
}