use crate::helpers::init_app;
use ethers::{
    prelude::*,
    solc::{Project, ProjectPathsConfig},
    utils::Anvil,
};
use gsy_ethers_listener::{GsyEthersListener, ListenerConfig};
use gsy_offchain_primitives::db_api_schema::orders::{IntelligentEnergyType, OrderEnum};
use gsy_offchain_storage::evm_handler::OffchainStorageEvmHandler;
use std::{fs::File, io::Write, sync::Arc, time::Duration};
use tempfile::TempDir;

abigen!(
    MockEmitter,
    r#"[
        event OrderPlaced(bytes16 indexed orderId, bytes16 indexed createdBy, bytes16 indexed marketId, uint64 timeSlot, uint64 creationTime, uint64 energy, uint64 energyRate, uint8 energySourcePreference, uint8 energyType, bool isBid)
        function emitOrderPlaced(bytes16 orderId, bytes16 createdBy, uint64 energy, uint64 rate) external
    ]"#
);

#[tokio::test]
async fn test_evm_order_listener_persists_to_db() {
    let app = init_app().await;
    let db = app.db_wrapper.clone();

    let anvil = Anvil::new().spawn();
    let ws_endpoint = anvil.ws_endpoint();
    let wallet: LocalWallet = anvil.keys()[0].clone().into();
    let provider = Provider::<Ws>::connect(&ws_endpoint).await.unwrap();
    let client = Arc::new(SignerMiddleware::new(
        provider,
        wallet.with_chain_id(anvil.chain_id()),
    ));

    let temp_dir = TempDir::new().unwrap();
    let contracts_dir = temp_dir.path().join("contracts");
    std::fs::create_dir(&contracts_dir).unwrap();
    let source_path = contracts_dir.join("MockEmitter.sol");

    let source = r#"
        // SPDX-License-Identifier: MIT
        pragma solidity ^0.8.0;
        contract MockEmitter {
            event OrderPlaced(bytes16 indexed orderId, bytes16 indexed createdBy, bytes16 indexed marketId, uint64 timeSlot, uint64 creationTime, uint64 energy, uint64 energyRate, uint8 energySourcePreference, uint8 energyType, bool isBid);
            function emitOrderPlaced(bytes16 orderId, bytes16 createdBy, uint64 energy, uint64 rate) external {
                // emit with hardcoded filler data for non-indexed fields not critical for this test
                emit OrderPlaced(orderId, createdBy, bytes16(0), 1000, 1234567890, energy, rate, 1, 0, true);
            }
        }
    "#;
    {
        let mut file = File::create(&source_path).unwrap();
        file.write_all(source.as_bytes()).unwrap();
    }

    let paths = ProjectPathsConfig::builder()
        .root(temp_dir.path())
        .sources(contracts_dir)
        .build()
        .unwrap();
    let project = Project::builder()
        .paths(paths)
        .ephemeral()
        .no_artifacts()
        .build()
        .unwrap();

    let compiled = project.compile().unwrap();
    let output = compiled.output();

    for err in &output.errors {
        if err.severity == ethers::solc::artifacts::Severity::Error {
            panic!("Solidity compilation error: {}", err.message);
        }
    }

    let contract_list = output
        .contracts
        .values()
        .flat_map(|inner| inner.iter())
        .find(|(name, _)| *name == "MockEmitter")
        .map(|(_, artifact)| artifact)
        .expect("MockEmitter artifact not found");

    let contract = &contract_list
        .first()
        .expect("No versioned contract found in artifact")
        .contract;

    let bytecode = contract
        .evm
        .as_ref()
        .expect("No EVM object found")
        .bytecode
        .as_ref()
        .expect("No bytecode found")
        .object
        .as_bytes()
        .expect("Bytecode not bytes")
        .clone();

    let abi = contract.abi.as_ref().expect("No ABI found").clone();

    let factory = ContractFactory::new(abi.into(), bytecode, client.clone());
    let contract = factory.deploy(()).unwrap().send().await.unwrap();
    let contract_address = contract.address();

    let config = ListenerConfig {
        node_url: ws_endpoint.clone(),
        order_registry_address: contract_address,
        trade_settlement_address: Address::zero(),
        market_controller_address: Address::zero(),
    };

    let handler = OffchainStorageEvmHandler { db: db.clone() };
    let listener = GsyEthersListener::new(config, handler);

    tokio::spawn(async move {
        listener.run().await.unwrap();
    });

    tokio::time::sleep(Duration::from_millis(1000)).await;

    let mock_contract = MockEmitter::new(contract_address, client.clone());
    let order_id = [0xAA; 16];
    let created_by = [0xBB; 16];
    let energy_val = 10000; // 1.0 energy scaled
    let rate_val = 5000; // 0.5 rate scaled

    let _tx = mock_contract
        .emit_order_placed(order_id, created_by, energy_val, rate_val)
        .send()
        .await
        .unwrap()
        .await
        .unwrap();

    let expected_id = format!("0x{}", hex::encode(order_id));

    let mut found = false;
    for _ in 0..20 {
        let order_bson = mongodb::bson::to_bson(&expected_id).unwrap();
        if let Ok(Some(order)) = db.orders().get_order_by_id(&order_bson).await {
            found = true;
            assert_eq!(order.order_type, OrderEnum::Bid);
            assert_eq!(order.energy_kWh, 1.0);
            assert_eq!(order.energy_rate, 0.5);
            assert_eq!(
                order
                    .requirements
                    .and_then(|requirements| requirements.energy_type),
                Some(IntelligentEnergyType::Green)
            );
            break;
        }
        tokio::time::sleep(Duration::from_millis(200)).await;
    }

    assert!(found, "Order was not found in MongoDB after 4 seconds");
}
