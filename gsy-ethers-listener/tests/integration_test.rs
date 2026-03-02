use anyhow::Result;
use async_trait::async_trait;
use ethers::{
    prelude::*,
    solc::{Project, ProjectPathsConfig},
    utils::Anvil,
};
use gsy_ethers_listener::{
    GsyEthersListener, GsyEventHandler, ListenerConfig, MarketStatusUpdatedFilter,
    OrderCancelledFilter, OrderPlacedFilter, TradeSettledFilter,
};
use std::fs::File;
use std::io::Write;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tempfile::TempDir;

struct MockHandler {
    pub received_hashes: Arc<Mutex<Vec<[u8; 32]>>>,
}

#[async_trait]
impl GsyEventHandler for MockHandler {
    async fn handle_order_placed(&self, event: OrderPlacedFilter) -> Result<()> {
        let mut store = self.received_hashes.lock().unwrap();
        store.push(event.order_hash);
        Ok(())
    }
    async fn handle_order_cancelled(&self, _: OrderCancelledFilter) -> Result<()> {
        Ok(())
    }
    async fn handle_trade_settled(&self, _: TradeSettledFilter) -> Result<()> {
        Ok(())
    }
    async fn handle_market_status(&self, _: MarketStatusUpdatedFilter) -> Result<()> {
        Ok(())
    }
}

mod mock_contract {
    use ethers::prelude::abigen;
    abigen!(
        MockEmitter,
        r#"[
            event OrderPlaced(bytes32 indexed orderHash, address indexed owner, bytes32 indexed marketId, bytes32 areaUuid, uint64 nonce, uint64 timeSlot, uint64 creationTime, uint64 energy, uint64 energyRate, bool isBid)
            function emitOrderPlaced(bytes32 orderHash, address owner) external
        ]"#
    );
}
use mock_contract::MockEmitter;

#[tokio::test]
async fn test_listener_captures_event_from_chain() -> Result<()> {
    let anvil = Anvil::new().spawn();
    let ws_endpoint = anvil.ws_endpoint();

    let wallet: LocalWallet = anvil.keys()[0].clone().into();
    let provider = Provider::<Ws>::connect(&ws_endpoint).await?;
    let client = Arc::new(SignerMiddleware::new(
        provider,
        wallet.with_chain_id(anvil.chain_id()),
    ));

    let temp_dir = TempDir::new()?;
    let contracts_dir = temp_dir.path().join("contracts");
    std::fs::create_dir(&contracts_dir)?;

    let source_path = contracts_dir.join("MockEmitter.sol");
    let source = r#"
        // SPDX-License-Identifier: MIT
        pragma solidity ^0.8.0;
        contract MockEmitter {
            event OrderPlaced(bytes32 indexed orderHash, address indexed owner, bytes32 indexed marketId, bytes32 areaUuid, uint64 nonce, uint64 timeSlot, uint64 creationTime, uint64 energy, uint64 energyRate, bool isBid);
            function emitOrderPlaced(bytes32 orderHash, address owner) external {
                emit OrderPlaced(orderHash, owner, bytes32(0), bytes32(0), 1, 100, 100, 1000, 50, true);
            }
        }
    "#;

    {
        let mut file = File::create(&source_path)?;
        file.write_all(source.as_bytes())?;
    }

    let paths = ProjectPathsConfig::builder()
        .root(temp_dir.path())
        .sources(contracts_dir)
        .build()?;

    let project = Project::builder()
        .paths(paths)
        .ephemeral()
        .no_artifacts()
        .build()?;

    let compiled = project.compile()?;
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
        .expect("Could not find MockEmitter artifact after compilation");

    let contract = &contract_list
        .first()
        .expect("No versioned contract found in artifact")
        .contract;

    let bytecode_object = contract
        .evm
        .as_ref()
        .expect("No EVM object found")
        .bytecode
        .as_ref()
        .expect("No bytecode found in contract")
        .object
        .as_bytes()
        .expect("Bytecode object is not bytes")
        .clone();

    let abi = contract.abi.as_ref().expect("No ABI found").clone();

    let factory = ContractFactory::new(abi.into(), bytecode_object, client.clone());

    let contract = factory.deploy(())?.send().await?;
    let contract_address = contract.address();

    let received_store = Arc::new(Mutex::new(Vec::new()));
    let handler = MockHandler {
        received_hashes: received_store.clone(),
    };

    let config = ListenerConfig {
        node_url: ws_endpoint.clone(),
        order_registry_address: contract_address,
        trade_settlement_address: contract_address,
        market_controller_address: contract_address,
    };

    let listener = GsyEthersListener::new(config, handler);

    let _handle = tokio::spawn(async move {
        listener.run().await.unwrap();
    });

    tokio::time::sleep(Duration::from_millis(1000)).await;

    let mock_contract = MockEmitter::new(contract_address, client.clone());
    let test_hash = [1u8; 32];
    let _tx = mock_contract
        .emit_order_placed(test_hash, anvil.addresses()[0])
        .send()
        .await?
        .await?;

    for _ in 0..50 {
        {
            let store = received_store.lock().unwrap();
            if !store.is_empty() {
                assert_eq!(store[0], test_hash);
                return Ok(());
            }
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    panic!("Timeout: Event not received by listener");
}
