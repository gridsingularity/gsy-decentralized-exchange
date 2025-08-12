use crate::world::{gsy_node, MyWorld};
use cucumber::{then, when};
use gsy_offchain_primitives::db_api_schema::profiles::MeasurementSchema;
use gsy_node::runtime_types::gsy_primitives::orders::{
    InputBid, InputOffer, InputOrder, OrderComponent,
};
use subxt::utils::H256;

#[when(regex = r#""([^"]*)" submits a bid for (\d+) energy at a rate of (\d+)"#)]
async fn submit_bid(world: &mut MyWorld, user_name: String, energy: u64, rate: u64) {
    let user = world.users.get(&user_name).unwrap().clone();
    let market_id = H256::random();
    world.last_market_id = Some(market_id);

    let bid = InputOrder::Bid(InputBid {
        buyer: user.public_key().into(),
        bid_component: OrderComponent {
            area_uuid: H256::random(),
            market_id,
            time_slot: 1677453190,
            creation_time: 1677453190,
            energy,
            energy_rate: rate,
        },
    });

    let tx = gsy_node::tx().orderbook_worker().insert_orders(vec![bid]);
    world
        .subxt_client
        .tx()
        .sign_and_submit_then_watch_default(&tx, &user)
        .await
        .expect("Failed to submit bid")
        .wait_for_finalized_success()
        .await
        .expect("Bid extrinsic failed");
    println!("Submitted bid for {}", user_name);
}

#[when(regex = r#""([^"]*)" submits an offer for (\d+) energy at a rate of (\d+)"#)]
async fn submit_offer(world: &mut MyWorld, user_name: String, energy: u64, rate: u64) {
    let user = world.users.get(&user_name).unwrap().clone();
    let market_id = world.last_market_id.expect("Market ID not set by bid step");

    let offer = InputOrder::Offer(InputOffer {
        seller: user.public_key().into(),
        offer_component: OrderComponent {
            area_uuid: H256::random(),
            market_id,
            time_slot: 1677453190,
            creation_time: 1677453190,
            energy,
            energy_rate: rate,
        },
    });

    let tx = gsy_node::tx().orderbook_worker().insert_orders(vec![offer]);
    world
        .subxt_client
        .tx()
        .sign_and_submit_then_watch_default(&tx, &user)
        .await
        .expect("Failed to submit offer")
        .wait_for_finalized_success()
        .await
        .expect("Offer extrinsic failed");
    println!("Submitted offer for {}", user_name);
}

#[when(regex = r#"measurements for "([^"]*)" and "([^"]*)" assets are submitted"#)]
async fn submit_measurements(world: &mut MyWorld, _user1: String, _user2: String) {
    let orderbook_url = std::env::var("ORDERBOOK_SERVICE_URL")
        .unwrap_or_else(|_| "http://127.0.0.1:8080".to_string());

    let measurements = vec![
        MeasurementSchema {
            area_uuid: "area-alice".to_string(),
            community_uuid: "community1".to_string(),
            energy_kwh: 10.0,
            time_slot: 1,
            creation_time: 1,
        },
        MeasurementSchema {
            area_uuid: "area-bob".to_string(),
            community_uuid: "community1".to_string(),
            energy_kwh: -10.0,
            time_slot: 1,
            creation_time: 1,
        },
    ];

    let res = world
        .http_client
        .post(format!("{}/measurements", orderbook_url))
        .json(&measurements)
        .send()
        .await
        .unwrap();

    assert!(res.status().is_success(), "Failed to post measurements");
    println!("Submitted measurements");
}

#[then("the matching engine matches the bid and offer and a trade is settled on-chain")]
async fn verify_trade_on_chain(world: &mut MyWorld) {
    let mut block_sub = world
        .subxt_client
        .blocks()
        .subscribe_finalized()
        .await
        .expect("Failed to subscribe to finalized blocks");

    for i in 0..30 {
        println!("Waiting for match, block {}/30...", i + 1);

        let block = tokio::time::timeout(std::time::Duration::from_secs(12), block_sub.next())
            .await
            .expect("Timeout waiting for new block")
            .unwrap()
            .unwrap();

        let events = block.events().await.unwrap();

        let order_executed_event =
            events.find_first::<gsy_node::orderbook_registry::events::OrderExecuted>();

        if let Ok(Some(event)) = order_executed_event {
            println!("OrderExecuted event found: {:?}", event.0);
            let trade = event.0;
            let alice_pubkey = world.users.get("alice").unwrap().public_key();
            let bob_pubkey = world.users.get("bob").unwrap().public_key();
            let alice_account_id: subxt::utils::AccountId32 = alice_pubkey.into();
            let bob_account_id: subxt::utils::AccountId32 = bob_pubkey.into();

            assert_eq!(trade.buyer, alice_account_id);
            assert_eq!(trade.seller, bob_account_id);
            assert_eq!(trade.parameters.selected_energy, 100);
            assert_eq!(trade.parameters.energy_rate, 15);
            return;
        }
    }
    panic!("Timeout: Did not find OrderExecuted event after 30 blocks");
}