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
    let community_market_endpoint = "http://gsy-orderbook:8080/community-market?community_name=".to_owned() +
        world.community_uuid.clone().unwrap().as_str() +
        "&time_slot=" + (world.target_delivery_time as u32).to_string().as_str();
    let stored_topology_res = world.community_client_api.get_existing_market_topology(
        community_market_endpoint
    ).await;
    assert_eq!(stored_topology_res.len(), 1);
    let stored_topology = stored_topology_res.get(0).unwrap();
    assert_eq!(stored_topology.community_name, world.community_uuid.clone().unwrap());
    assert_eq!(stored_topology.time_slot, world.target_delivery_time as u32);
}