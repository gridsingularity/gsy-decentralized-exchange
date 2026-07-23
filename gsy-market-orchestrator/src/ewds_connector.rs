use primitives::db_api_schema::{
    market::MarketSchema,
    grid_topology::EnergyCommunitySchema,
};
use primitives::ewds::{query_via_ewds, EwdsQueryRequest, EwdsOperation};
use primitives::utils::convert_uuid_string_to_bytes;
use tracing::info;
use anyhow::Result;


pub async fn fetch_list_of_community_ids_via_ewds(
) -> Result<Vec<[u8; 16]>> {
    let query = serde_json::json!({});
    let community_ids: Vec<EnergyCommunitySchema> = query_via_ewds(EwdsQueryRequest {
        operation: EwdsOperation::CommunitiesQuery,
        query_payload: query.clone(),
        request_topic_env: "EWDS_ONTOLOGY_REQUEST_TOPIC",
        request_topic_default: "ontologyQuery",
        response_topic_env: "EWDS_ONTOLOGY_RESPONSE_TOPIC",
        response_topic_default: "ontologyQueryQueryResponse",
        response_client_id_env: "EWDS_MARKET_ORCHESTRATOR_ID",
        response_client_id_default: "gsyemarketorchestrator",
        timeout_ms_default: 8_000,
    }
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
) -> Result<Vec<MarketSchema>> {
    let query = serde_json::json!({
        "marketType": market_type,
        "openTime": start_time,
        "communityId": community_id
    });
    let markets: Vec<MarketSchema> = query_via_ewds(EwdsQueryRequest {
        operation: EwdsOperation::MarketsQuery,
        query_payload: query.clone(),
        request_topic_env: "EWDS_MARKETS_REQUEST_TOPIC",
        request_topic_default: "marketsQuery",
        response_topic_env: "EWDS_MARKETS_RESPONSE_TOPIC",
        response_topic_default: "marketsQueryResponse",
        response_client_id_env: "EWDS_MARKET_ORCHESTRATOR_ID",
        response_client_id_default: "gsyemarketorchestrator",
        timeout_ms_default: 8_000,
    })
        .await?;
    Ok(markets)
}
