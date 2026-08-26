use crate::db_api_schema::grid_topology::FacilitySchema;
use crate::ewds::{EwdsClient, EwdsOperation};
use anyhow::{anyhow, Result};
use reqwest::Client;
use std::collections::HashMap;
use std::env;
use tracing::info;

pub async fn fetch_facility_owner_mapping(
    consumer_client_id_env: &str,
    consumer_client_id_default: &str,
) -> Result<HashMap<String, String>> {
    let facilities: Vec<FacilitySchema> = if env::var("OFFCHAIN_STORAGE_TRANSPORT")
        .map(|value| value.eq_ignore_ascii_case("ewds"))
        .unwrap_or(false)
    {
        info!("Fetching facilities via EWDS transport");
        let ewds_client =
            EwdsClient::from_env(consumer_client_id_env, consumer_client_id_default, 8_000);

        ewds_client
            .query(EwdsOperation::FacilitiesQuery, serde_json::json!({}))
            .await?
    } else {
        let client = Client::new();
        let offchain_url = env::var("OFFCHAIN_STORAGE_URL")
            .unwrap_or("http://gsy-offchain-storage:8080".to_string());
        let facilities_url = format!("{}/facilities", offchain_url);
        info!("Fetching facilities for {}", facilities_url);

        let facilities_resp = client.get(&facilities_url).send().await?;
        if !facilities_resp.status().is_success() {
            return Err(anyhow!(
                "Failed to fetch facilities. HTTP {}",
                facilities_resp.status()
            ));
        }
        facilities_resp.json().await?
    };

    let mapping: HashMap<String, String> = facilities
        .into_iter()
        .map(|f| (f.facility_id, f.owner_id))
        .collect();
    info!("returning mapping {:?}", mapping.clone());
    Ok(mapping)
}
