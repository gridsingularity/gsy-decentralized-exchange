use crate::world::MyWorld;
use cucumber::given;
use ethers::prelude::*;
use ethers::utils::keccak256;
use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;

abigen!(
    MarketControllerContract,
    r#"[
        function hasRole(bytes32 role, address account) external view returns (bool)
    ]"#
);

abigen!(
    TradeSettlementContract,
    r#"[
        function hasRole(bytes32 role, address account) external view returns (bool)
    ]"#
);

abigen!(
    GsyVaultContract,
    r#"[
        function deposit() external payable
        function balances(address account) external view returns (uint256)
    ]"#
);

#[given("the GSY DEX services are running")]
async fn services_are_running(world: &mut MyWorld) {
    let res = world
        .http_client
        .get(format!("{}/health_check", world.orderbook_service_url))
        .send()
        .await
        .expect("Failed to contact orderbook service");
    assert!(
        res.status().is_success(),
        "Orderbook service is not healthy"
    );

    let chain_id = world
        .provider
        .get_chainid()
        .await
        .expect("Failed to contact EVM node")
        .as_u64();

    let market_controller_code = world
        .provider
        .get_code(world.market_controller_address, None)
        .await
        .expect("Failed to read MarketController bytecode");
    let order_registry_code = world
        .provider
        .get_code(world.order_registry_address, None)
        .await
        .expect("Failed to read OrderRegistry bytecode");
    let trade_settlement_code = world
        .provider
        .get_code(world.trade_settlement_address, None)
        .await
        .expect("Failed to read TradeSettlement bytecode");

    assert!(
        !market_controller_code.0.is_empty(),
        "MarketController is not deployed"
    );
    assert!(
        !order_registry_code.0.is_empty(),
        "OrderRegistry is not deployed"
    );
    assert!(
        !trade_settlement_code.0.is_empty(),
        "TradeSettlement is not deployed"
    );

    println!("Services are running. chain_id={}", chain_id);
}

#[given(
    regex = r#"users "([^"]*)", "([^"]*)", and "([^"]*)" the matching engine operator are registered and have collateral"#
)]
#[given(regex = r#"users "([^"]*)", "([^"]*)", and "([^"]*)" are registered and have collateral"#)]
async fn users_are_registered(
    world: &mut MyWorld,
    first_user: String,
    second_user: String,
    third_user: String,
) {
    let mut seen = HashSet::new();
    let users = [
        first_user.as_str(),
        second_user.as_str(),
        third_user.as_str(),
    ];

    for user_name in users {
        let wallet = world.wallet_for_user(user_name);
        if seen.insert(wallet.address()) {
            let signer = Arc::new(SignerMiddleware::new(
                world.provider.clone(),
                wallet.clone(),
            ));
            let vault = GsyVaultContract::new(world.gsy_vault_address, signer.clone());

            let existing_balance = vault
                .balances(wallet.address())
                .call()
                .await
                .expect("Failed to query vault balance before deposit");
            if existing_balance > U256::zero() {
                continue;
            }

            let mut deposited = false;
            let mut last_error = String::new();
            for attempt in 0..5 {
                let deposit_call = vault
                    .deposit()
                    .value(U256::from(1_000_000_000_000_000_000u128));
                match deposit_call.send().await {
                    Ok(pending_tx) => {
                        let receipt = pending_tx
                            .await
                            .expect("Failed awaiting collateral deposit receipt");
                        assert!(
                            receipt.is_some(),
                            "Collateral deposit tx was dropped without receipt"
                        );
                        deposited = true;
                        break;
                    }
                    Err(error) => {
                        last_error = error.to_string();
                        let is_retryable_nonce_error = last_error.contains("nonce too low")
                            || last_error.contains("already known")
                            || last_error.contains("replacement transaction underpriced");
                        if is_retryable_nonce_error && attempt < 4 {
                            sleep(Duration::from_millis(300)).await;
                            continue;
                        }

                        panic!(
                            "Failed to submit collateral deposit transaction: {:?}",
                            error
                        );
                    }
                };
            }

            assert!(
                deposited,
                "Could not submit collateral deposit transaction after retries: {}",
                last_error
            );

            let balance = vault
                .balances(wallet.address())
                .call()
                .await
                .expect("Failed to query vault balance");

            assert!(
                balance > U256::zero(),
                "Vault balance for {} is zero after deposit",
                user_name
            );
        }
    }

    let orchestrator_wallet = std::env::var("ORCHESTRATOR_SIGNER_PRIVATE_KEY")
        .unwrap_or_else(|_| world.private_key_for_user("alice"))
        .parse::<LocalWallet>()
        .expect("Invalid orchestrator private key")
        .with_chain_id(world.chain_id);

    let matching_wallet = std::env::var("MATCHING_ENGINE_PRIVATE_KEY")
        .unwrap_or_else(|_| world.private_key_for_user("alice"))
        .parse::<LocalWallet>()
        .expect("Invalid matching engine private key")
        .with_chain_id(world.chain_id);

    let execution_wallet = std::env::var("EXECUTION_ENGINE_PRIVATE_KEY")
        .unwrap_or_else(|_| world.private_key_for_user("alice"))
        .parse::<LocalWallet>()
        .expect("Invalid execution engine private key")
        .with_chain_id(world.chain_id);

    let market_controller =
        MarketControllerContract::new(world.market_controller_address, world.provider.clone());
    let trade_settlement =
        TradeSettlementContract::new(world.trade_settlement_address, world.provider.clone());

    let orchestrator_role = keccak256("ORCHESTRATOR_ROLE");
    let operator_role = keccak256("OPERATOR_ROLE");
    let execution_role = keccak256("EXECUTION_ENGINE_ROLE");

    assert!(
        market_controller
            .has_role(orchestrator_role, orchestrator_wallet.address())
            .call()
            .await
            .expect("Failed to check ORCHESTRATOR_ROLE"),
        "Orchestrator account does not have ORCHESTRATOR_ROLE"
    );

    assert!(
        trade_settlement
            .has_role(operator_role, matching_wallet.address())
            .call()
            .await
            .expect("Failed to check OPERATOR_ROLE"),
        "Matching engine account does not have OPERATOR_ROLE"
    );

    assert!(
        trade_settlement
            .has_role(execution_role, execution_wallet.address())
            .call()
            .await
            .expect("Failed to check EXECUTION_ENGINE_ROLE"),
        "Execution engine account does not have EXECUTION_ENGINE_ROLE"
    );
}
