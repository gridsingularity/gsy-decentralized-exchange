use crate::world::{gsy_node, MyWorld};
use cucumber::given;
use subxt::utils::AccountId32;

#[given("the GSY DEX services are running")]
async fn services_are_running(world: &mut MyWorld) {
	let orderbook_url = std::env::var("ORDERBOOK_SERVICE_URL")
		.unwrap_or_else(|_| "http://127.0.0.1:8080".to_string());
	let res = world
		.http_client
		.get(format!("{}/health_check", orderbook_url))
		.send()
		.await
		.expect("Failed to contact orderbook service");
	assert!(res.status().is_success(), "Orderbook service is not healthy");

	let block_number = world
		.subxt_client
		.blocks()
		.at_latest()
		.await
		.expect("Failed to contact gsy-node")
		.number();
	assert!(block_number > 0, "Node is not producing blocks.");
	println!("Services are running. Current block: {}", block_number);
}

#[given(
	regex = r#"users "([^"]*)", "([^"]*)", and "([^"]*)" the matching engine operator are registered and have collateral"#
)]
async fn users_are_registered(
	world: &mut MyWorld,
	alice_name: String,
	bob_name: String,
	charlie_name: String,
) {
	let sudo_signer = subxt_signer::sr25519::dev::alice();
	let user_keys = [
		world.users.get(&alice_name).unwrap(),
		world.users.get(&bob_name).unwrap(),
		world.users.get(&charlie_name).unwrap(),
	];

	for keypair in user_keys.iter() {
		let account_id: AccountId32 = keypair.public_key().into();
		println!("Registering user: {:?}", account_id);

		let register_user_call =
			gsy_node::runtime_types::gsy_node_runtime::RuntimeCall::GsyCollateral(
				gsy_node::runtime_types::gsy_collateral::pallet::Call::register_user {
					user_account: account_id.clone(),
				},
			);

		let sudo_tx = gsy_node::tx().sudo().sudo(register_user_call);

		world
			.subxt_client
			.tx()
			.sign_and_submit_then_watch_default(&sudo_tx, &sudo_signer)
			.await
			.expect("Failed to submit register_user tx")
			.wait_for_finalized_success()
			.await
			.expect("register_user extrinsic failed");

		let deposit_tx = gsy_node::tx().gsy_collateral().deposit_collateral(500000000000000);
		world
			.subxt_client
			.tx()
			.sign_and_submit_then_watch_default(&deposit_tx, *keypair)
			.await
			.expect("Failed to submit deposit_collateral tx")
			.wait_for_finalized_success()
			.await
			.expect("deposit_collateral extrinsic failed");
	}

	let alice_account_id: AccountId32 = world.users.get(&alice_name).unwrap().public_key().into();
	println!("Registering market orchestrator/matching engine operator: {:?}", alice_account_id);

	let register_me_call = gsy_node::runtime_types::gsy_node_runtime::RuntimeCall::GsyCollateral(
		gsy_node::runtime_types::gsy_collateral::pallet::Call::register_exchange_operator {
			operator_account: alice_account_id,
		},
	);

	let sudo_tx_me = gsy_node::tx().sudo().sudo(register_me_call);
	world
		.subxt_client
		.tx()
		.sign_and_submit_then_watch_default(&sudo_tx_me, &sudo_signer)
		.await
		.expect("Failed to submit register_exchange_operator tx")
		.wait_for_finalized_success()
		.await
		.expect("register_exchange_operator extrinsic failed");
}
