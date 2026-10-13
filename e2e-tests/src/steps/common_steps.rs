use crate::world::{gsy_node, MyWorld};
use cucumber::given;
use std::time::Duration;
use subxt::tx::Payload;
use subxt::utils::AccountId32;
use subxt_signer::sr25519::Keypair;

const MAX_SUBMIT_ATTEMPTS: usize = 6;

fn is_transient_submit_error(error: &subxt::Error) -> bool {
	let msg = format!("{error:?}").to_lowercase();
	msg.contains("transaction is outdated")
		|| msg.contains("priority is too low")
		|| msg.contains("stale")
		|| msg.contains("future")
		|| msg.contains("already imported")
		|| msg.contains("temporarily banned")
}

async fn submit_and_finalize<C: Payload>(
	world: &MyWorld,
	call: &C,
	signer: &Keypair,
	label: &str,
) {
	let mut attempt = 1;
	let progress = loop {
		match world.subxt_client.tx().sign_and_submit_then_watch_default(call, signer).await {
			Ok(progress) => break progress,
			Err(error) if attempt < MAX_SUBMIT_ATTEMPTS && is_transient_submit_error(&error) => {
				println!(
					"Transient error submitting {} (attempt {}/{}): {:?}; retrying...",
					label, attempt, MAX_SUBMIT_ATTEMPTS, error
				);
				attempt += 1;
				tokio::time::sleep(Duration::from_secs(3)).await;
			},
			Err(error) => panic!("Failed to submit {} tx: {:?}", label, error),
		}
	};

	progress
		.wait_for_finalized_success()
		.await
		.unwrap_or_else(|error| panic!("{} extrinsic failed: {:?}", label, error));
}

#[given("the GSY DEX services are running")]
async fn services_are_running(world: &mut MyWorld) {
	let orderbook_url = std::env::var("OFFCHAIN_STORAGE_URL")
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
	regex = r#"users "([^"]*)" and "([^"]*)" are registered and have collateral, with "[^"]*" as the matching engine operator"#
)]
async fn users_are_registered(
	world: &mut MyWorld,
	seller_name: String,
	buyer_name: String,
) {
	let sudo_signer = subxt_signer::sr25519::dev::alice();
	let user_keys = [
		world.users.get(&seller_name).unwrap(),
		world.users.get(&buyer_name).unwrap(),
	];

	for keypair in user_keys.iter() {
		let account_id: AccountId32 = keypair.public_key().into();

		// Do not try to reregister the same users to avoid the "Priority is too low" error. 
		if is_user_registered(world, &account_id).await {
			println!("User already registered, skipping: {:?}", account_id);
			continue;
		}
		println!("Registering user: {:?}", account_id);

		let register_user_call =
			gsy_node::runtime_types::gsy_node_runtime::RuntimeCall::GsyCollateral(
				gsy_node::runtime_types::gsy_collateral::pallet::Call::register_user {
					user_account: account_id.clone(),
				},
			);

		let sudo_tx = gsy_node::tx().sudo().sudo(register_user_call);
		submit_and_finalize(world, &sudo_tx, &sudo_signer, "register_user").await;

		let deposit_tx = gsy_node::tx().gsy_collateral().deposit_collateral(500000000000000);
		submit_and_finalize(world, &deposit_tx, *keypair, "deposit_collateral").await;
	}

	// The matching engine and market orchestrator settle/operate signed by the
	// sudo/root account (dev::alice), so that account must be the exchange operator.
	let operator_account_id: AccountId32 = sudo_signer.public_key().into();
	if is_operator_registered(world, &operator_account_id).await {
		println!("Exchange operator already registered, skipping: {:?}", operator_account_id);
		return;
	}
	println!("Registering market orchestrator/matching engine operator: {:?}", operator_account_id);

	let register_me_call = gsy_node::runtime_types::gsy_node_runtime::RuntimeCall::GsyCollateral(
		gsy_node::runtime_types::gsy_collateral::pallet::Call::register_exchange_operator {
			operator_account: operator_account_id,
		},
	);

	let sudo_tx_me = gsy_node::tx().sudo().sudo(register_me_call);
	submit_and_finalize(world, &sudo_tx_me, &sudo_signer, "register_exchange_operator").await;
}

async fn is_user_registered(world: &MyWorld, account_id: &AccountId32) -> bool {
	let storage_address = gsy_node::storage().gsy_collateral().registered_user(account_id.clone());
	world
		.subxt_client
		.storage()
		.at_latest()
		.await
		.expect("Failed to read latest storage")
		.fetch(&storage_address)
		.await
		.expect("Failed to fetch RegisteredUser storage")
		.is_some()
}

async fn is_operator_registered(world: &MyWorld, account_id: &AccountId32) -> bool {
	let storage_address =
		gsy_node::storage().gsy_collateral().registered_exchange_operator(account_id.clone());
	world
		.subxt_client
		.storage()
		.at_latest()
		.await
		.expect("Failed to read latest storage")
		.fetch(&storage_address)
		.await
		.expect("Failed to fetch RegisteredExchangeOperator storage")
		.is_some()
}
