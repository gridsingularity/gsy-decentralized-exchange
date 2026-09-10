use crate::world::MyWorld;
use cucumber::{when, then};
use gsy_community_client::topology::TopologyManager;


#[when(regex = r#"the GSY DEX Community Client reads the FEDECOM ontology data"#)]
async fn read_fedecom_ontology_data(world: &mut MyWorld) {
    let topology_manager = TopologyManager::new(
        &world.http_client.clone(), &world.community_client_api);
    let topology = topology_manager.get(world.target_delivery_time).await;
    assert_eq!(topology.len(), 3);
    let market = topology.first().unwrap().clone();
    world.target_delivery_time = market.time_slot as u64;
    world.community_uuid = Some(market.community_name.clone());

}

#[then(regex = r#"the ontology data are saved to GSY DEX offchain storage"#)]
async fn fedecom_ontology_saved_to_storage(world: &mut MyWorld) {
    let orderbook_url = std::env::var("OFFCHAIN_STORAGE_URL")
        .unwrap_or_else(|_| "http://127.0.0.1:8080".to_string());
    // The /community-market endpoint filters by start_time/end_time, not a `time_slot`
    // param (which it silently ignores). Scope the query to this delivery slot so the
    // assertion counts only this slot's market rather than every slot ever stored for
    // the community.
    let slot = (world.target_delivery_time as u32).to_string();
    let community_market_endpoint = orderbook_url + "/community-market?community_name=" +
        world.community_uuid.clone().unwrap().as_str() +
        "&start_time=" + slot.as_str() + "&end_time=" + slot.as_str();
    let stored_topology_res = world.community_client_api.get_existing_market_topology(
        community_market_endpoint
    ).await;
    assert_eq!(stored_topology_res.len(), 1);
    let stored_topology = stored_topology_res.get(0).unwrap();
    assert_eq!(stored_topology.community_name, world.community_uuid.clone().unwrap());
    assert_eq!(stored_topology.time_slot, world.target_delivery_time as u32);
}