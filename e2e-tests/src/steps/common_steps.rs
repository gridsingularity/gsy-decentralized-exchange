use crate::world::MyWorld;
use cucumber::given;
use ethers::prelude::*;
use ethers::utils::keccak256;
use primitives::MatchingAlgorithm;
use std::env;
use std::str::FromStr;

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
    OrderRegistryContract,
    r#"[function hasRole(bytes32 role, address account) external view returns (bool)]"#
);

#[given("the GSY DEX services are running")]
async fn services_are_running(world: &mut MyWorld) {
    let res = world
        .http_client
        .get(format!("{}/health_check", world.offchain_storage_url))
        .send()
        .await
        .expect("Failed to contact off-chain storage service");
    assert!(
        res.status().is_success(),
        "Off-chain storage service is not healthy"
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
    let actor_registry_code = world
        .provider
        .get_code(world.actor_registry_address, None)
        .await
        .expect("Failed to read ActorRegistry bytecode");
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
        !actor_registry_code.0.is_empty(),
        "ActorRegistry is not deployed"
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

#[given(expr = "the matching engine uses {string}")]
async fn matching_engine_uses_algorithm(_world: &mut MyWorld, expected_algorithm: String) {
    let expected = MatchingAlgorithm::from_str(expected_algorithm.as_str())
        .unwrap_or_else(|error| panic!("Invalid matching algorithm in feature: {}", error));
    let configured_value =
        env::var("MATCHING_ALGORITHM").unwrap_or_else(|_| MatchingAlgorithm::default().to_string());
    let configured = MatchingAlgorithm::from_str(configured_value.as_str())
        .unwrap_or_else(|error| panic!("Invalid MATCHING_ALGORITHM: {}", error));

    assert_eq!(
        configured, expected,
        "This feature requires MATCHING_ALGORITHM={} but the E2E stack is configured for {}",
        expected, configured
    );
}

#[given("the DEX service accounts are authorized")]
async fn service_accounts_are_authorized(world: &mut MyWorld) {
    let registry = OrderRegistryContract::new(world.order_registry_address, world.provider.clone());
    assert!(
        registry
            .has_role(
                keccak256("COMMUNITY_CLIENT_ROLE"),
                world.community_client_wallet().address()
            )
            .call()
            .await
            .unwrap(),
        "Community client lacks COMMUNITY_CLIENT_ROLE"
    );

    let orchestrator_wallet = std::env::var("ORCHESTRATOR_SIGNER_PRIVATE_KEY")
        .unwrap_or_else(|_| world.community_client_private_key())
        .parse::<LocalWallet>()
        .expect("Invalid orchestrator private key")
        .with_chain_id(world.chain_id);

    let matching_wallet = std::env::var("MATCHING_ENGINE_PRIVATE_KEY")
        .unwrap_or_else(|_| world.community_client_private_key())
        .parse::<LocalWallet>()
        .expect("Invalid matching engine private key")
        .with_chain_id(world.chain_id);

    let execution_wallet = std::env::var("EXECUTION_ENGINE_PRIVATE_KEY")
        .unwrap_or_else(|_| world.community_client_private_key())
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
