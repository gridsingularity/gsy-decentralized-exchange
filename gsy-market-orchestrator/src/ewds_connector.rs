use gsy_offchain_primitives::db_api_schema::{
    market::IntelligentMarketSchema,
    grid_topology::EnergyCommunitySchema,
};
use gsy_offchain_primitives::ewds_utils::query_via_ewds;
use gsy_offchain_primitives::utils::convert_uuid_string_to_bytes;
use tracing::info;
use anyhow::Result;

pub async fn fetch_list_of_community_ids_via_ewds(
) -> Result<Vec<[u8; 16]>> {
    let query = serde_json::json!({});
    let community_ids: Vec<EnergyCommunitySchema> = query_via_ewds(
        "communities.query",
        query.clone(),
        "EWDS_ONTOLOGY_REQUEST_TOPIC",
        "ontologyQuery",
        "EWDS_ONTOLOGY_RESPONSE_TOPIC",
        "ontologyQueryQueryResponse",
    )
        .await?;
    let community_id_strings: Vec<String> = community_ids
        .into_iter()
        .map(|c| c.community_id)
        .collect();

    info!("communities {:?}", community_id_strings);

    let community_ids: Vec<[u8; 16]> = community_id_strings
        .iter()
        .map(|s| convert_uuid_string_to_bytes(s))
        .collect::<Result<Vec<_>>>()?;
    Ok(community_ids)
}

pub async fn fetch_list_of_markets_via_ewds(
    market_type: u8,
    start_time: u64,
    community_id: [u8; 16],
) -> Result<Vec<IntelligentMarketSchema>> {
    let query = serde_json::json!({
        "marketType": market_type,
        "openTime": start_time,
        "communityId": community_id
    });
    let markets: Vec<IntelligentMarketSchema> = query_via_ewds(
        "markets.query",
        query.clone(),
        "EWDS_MARKETS_REQUEST_TOPIC",
        "marketsQuery",
        "EWDS_MARKETS_RESPONSE_TOPIC",
        "marketsQueryResponse",
    )
        .await?;
    Ok(markets)
}