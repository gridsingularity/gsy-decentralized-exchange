use crate::world::MyWorld;
use cucumber::given;
use ethers::prelude::*;
use ethers::utils::keccak256;
use std::collections::HashSet;
use std::sync::Arc;

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
    ActorRegistryContract,
    r#"[
        function registerActor(bytes16 actorId, address wallet) external
        function hasRole(bytes32 role, address account) external view returns (bool)
    ]"#
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

#[given(
    regex = r#"users "([^"]*)", "([^"]*)", and "([^"]*)" the matching engine operator are registered"#
)]
#[given(regex = r#"users "([^"]*)", "([^"]*)", and "([^"]*)" are registered"#)]
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

    let actor_registrar_wallet = std::env::var("ACTOR_REGISTRAR_PRIVATE_KEY")
        .or_else(|_| std::env::var("ORCHESTRATOR_SIGNER_PRIVATE_KEY"))
        .unwrap_or_else(|_| world.private_key_for_user("alice"))
        .parse::<LocalWallet>()
        .expect("Invalid actor registrar private key")
        .with_chain_id(world.chain_id);
    let actor_registrar_signer = Arc::new(SignerMiddleware::new(
        world.provider.clone(),
        actor_registrar_wallet.clone(),
    ));
    let actor_registry =
        ActorRegistryContract::new(world.actor_registry_address, actor_registrar_signer.clone());
    let actor_registrar_role = keccak256("ACTOR_REGISTRAR_ROLE");

    assert!(
        actor_registry
            .has_role(actor_registrar_role, actor_registrar_wallet.address())
            .call()
            .await
            .expect("Failed to check ACTOR_REGISTRAR_ROLE"),
        "Actor registrar account does not have ACTOR_REGISTRAR_ROLE"
    );

    for user_name in users {
        let wallet = world.wallet_for_user(user_name);
        let actor_id = world.actor_id_for_user(user_name);
        if seen.insert(actor_id) {
            let register_call = actor_registry.register_actor(actor_id, wallet.address());
            let register_receipt = register_call
                .send()
                .await
                .expect("Failed to submit actor registration transaction")
                .await
                .expect("Failed awaiting actor registration receipt");
            assert!(
                register_receipt.is_some(),
                "Actor registration tx was dropped without receipt"
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
