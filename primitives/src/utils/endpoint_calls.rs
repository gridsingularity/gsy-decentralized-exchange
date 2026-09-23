use crate::db_api_schema::grid_topology::FacilitySchema;
use crate::db_api_schema::ids::IdMappingSchema;
use crate::db_api_schema::orders::{DbAttributes, DbRequirements};
use crate::ewds::{EwdsClient, EwdsOperation};
use crate::utils::{bytes16_to_hex, parse_uuid_or_hex_bytes16};
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

pub async fn fetch_onchain_id(
    consumer_client_id_env: &str,
    consumer_client_id_default: &str,
    offchain_id: &str,
) -> Result<String> {
    let mapping: IdMappingSchema = if env::var("OFFCHAIN_STORAGE_TRANSPORT")
        .map(|value| value.eq_ignore_ascii_case("ewds"))
        .unwrap_or(false)
    {
        info!("Fetching onchain_id via EWDS transport");
        let ewds_client =
            EwdsClient::from_env(consumer_client_id_env, consumer_client_id_default, 8_000);

        let mut resp: Vec<IdMappingSchema> = ewds_client
            .query(
                EwdsOperation::IdsQuery,
                serde_json::json!({"offchain_id": offchain_id}),
            )
            .await?;
        let mapping = resp
            .pop()
            .ok_or_else(|| anyhow!("No id mapping returned for offchain_id {}", offchain_id))?;
        mapping
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
        mapping
    };

    if mapping.offchain_id != offchain_id {
        return Err(anyhow!(
            "ID service returned a mapping for a different facility"
        ));
    }
    let bytes = parse_uuid_or_hex_bytes16(&mapping.onchain_id)
        .filter(|bytes| *bytes != [0; 16])
        .ok_or_else(|| anyhow!("Invalid on-chain ID returned for facility {}", offchain_id))?;
    Ok(bytes16_to_hex(bytes))
}

/// Resolve off-chain facility IDs before converting metadata or matching actor IDs.
/// Input identifiers are always off-chain IDs, including UUID and hex-shaped strings.
pub async fn resolve_order_partner_ids(
    requirements: &mut Option<DbRequirements>,
    attributes: &mut Option<DbAttributes>,
    consumer_client_id_env: &str,
    consumer_client_id_default: &str,
) -> Result<()> {
    for partner in [
        requirements
            .as_mut()
            .and_then(|value| value.trading_partner_id.as_mut()),
        attributes
            .as_mut()
            .and_then(|value| value.trading_partner_id.as_mut()),
    ]
    .into_iter()
    .flatten()
    {
        *partner =
            fetch_onchain_id(consumer_client_id_env, consumer_client_id_default, partner).await?;
    }
    Ok(())
}
