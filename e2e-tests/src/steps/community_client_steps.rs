use crate::world::MyWorld;
use cucumber::{when, then};
use gsy_community_client::topology::TopologyManager;


#[when(regex = r#"the GSY DEX Community Client reads the FEDECOM ontology data"#)]
async fn read_fedecom_ontology_data(world: &mut MyWorld) {
    let topology_manager = TopologyManager::new(
        &world.http_client.clone(), &world.community_client_api);
    let topology = topology_manager.get(world.target_delivery_time).await;
    assert_eq!(topology.len(), 1);
    let market = topology.first().unwrap().clone();
    world.target_delivery_time = market.time_slot as u64;
    world.community_uuid = Some(market.community_uuid);

}

#[then(regex = r#"the ontology data are saved to GSY DEX offchain storage"#)]
async fn fedecom_ontology_saved_to_storage(world: &mut MyWorld) {
    let stored_topology = world.community_client_api.get_existing_market_topology(
        "http://gsy-orderbook:8080/community-market?community_uuid=".to_owned() +
            world.community_uuid.clone().unwrap().as_str() +
            "&time_slot=" + (world.target_delivery_time as u32).to_string().as_str()
    ).await.unwrap();
    assert_eq!(stored_topology.community_uuid, world.community_uuid.clone().unwrap());
    assert_eq!(stored_topology.time_slot, world.target_delivery_time as u32);
}