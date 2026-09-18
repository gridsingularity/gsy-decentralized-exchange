use anyhow::Result;
use async_trait::async_trait;
use ethers::{
    prelude::*,
    solc::{Project, ProjectPathsConfig},
    utils::{Anvil, AnvilInstance},
};
use gsy_ethers_listener::{
    GsyEthersListener, GsyEventHandler, ListenerConfig, MarketStatusUpdatedFilter,
    OrderCancelledFilter, OrderPlacedFilter, TradeSettledFilter, MarketClearingFilter
};
use std::fs::File;
use std::io::Write;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tempfile::TempDir;

struct MockHandler {
    pub received_hashes: Arc<Mutex<Vec<[u8; 16]>>>,
    pub received_clearings: Arc<Mutex<Vec<([u8; 16], H256, u64)>>>,
}

#[async_trait]
impl GsyEventHandler for MockHandler {
    async fn handle_order_placed(&self, event: OrderPlacedFilter) -> Result<()> {
        let mut store = self.received_hashes.lock().unwrap();
        store.push(event.order_id);
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
    async fn handle_market_clearing(
        &self,
        event: MarketClearingFilter,
        meta: LogMeta,
        block_timestamp: u64,
    ) -> Result<()> {
        let mut store = self.received_clearings.lock().unwrap();
        store.push((event.market_id, meta.block_hash, block_timestamp));
        Ok(())
    }
}

mod mock_contract {
    use ethers::prelude::abigen;
    abigen!(
        MockEmitter,
        r#"[
            event OrderPlaced(bytes16 indexed orderId, bytes16 indexed createdBy, bytes16 indexed marketId, uint64 timeSlot, uint64 creationTime, uint64 energy, uint64 energyRate, uint8 energySourcePreference, uint8 energyType, bool isBid, bytes16 preferredTradingPartner, uint64 preferredEnergyRate, bytes16 tradingPartner)
            function emitOrderPlaced(bytes16 orderId, bytes16 createdBy) external
            function emitMarketClearings(bytes16 firstMarketId, bytes16 secondMarketId) external
        ]"#
    );
}
use mock_contract::MockEmitter;

type TestClient = Arc<SignerMiddleware<Provider<Ws>, LocalWallet>>;

struct TestChain {
    // Keeps the Anvil process alive for the duration of the test.
    _anvil: AnvilInstance,
    client: TestClient,
    contract_address: Address,
    received_hashes: Arc<Mutex<Vec<[u8; 16]>>>,
    received_clearings: Arc<Mutex<Vec<([u8; 16], H256, u64)>>>,
}

/// Deploys the mock emitter on a fresh Anvil chain and starts a listener for it.
async fn start_listener_on_mock_chain() -> Result<TestChain> {
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
            event OrderPlaced(bytes16 indexed orderId, bytes16 indexed createdBy, bytes16 indexed marketId, uint64 timeSlot, uint64 creationTime, uint64 energy, uint64 energyRate, uint8 energySourcePreference, uint8 energyType, bool isBid, bytes16 preferredTradingPartner, uint64 preferredEnergyRate, bytes16 tradingPartner);
            function emitOrderPlaced(bytes16 orderId, bytes16 createdBy) external {
                emit OrderPlaced(orderId, createdBy, bytes16(0), 100, 100, 1000, 50, 1, 0, true, bytes16(0), 0, bytes16(0));
            }
            event MarketClearing(bytes16 indexed marketId, uint8 clearingStatus, uint256 clearingPrice, uint256 totalSupply, uint256 totalDemand, uint256 tradedQuantity, uint32 numTrades);
            function emitMarketClearings(bytes16 firstMarketId, bytes16 secondMarketId) external {
                emit MarketClearing(firstMarketId, 0, 10, 20, 30, 20, 1);
                emit MarketClearing(secondMarketId, 0, 10, 20, 30, 20, 1);
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

    let received_hashes = Arc::new(Mutex::new(Vec::new()));
    let received_clearings = Arc::new(Mutex::new(Vec::new()));
    let handler = MockHandler {
        received_hashes: received_hashes.clone(),
        received_clearings: received_clearings.clone(),
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

    Ok(TestChain {
        _anvil: anvil,
        client,
        contract_address,
        received_hashes,
        received_clearings,
    })
}

#[tokio::test]
async fn test_listener_captures_event_from_chain() -> Result<()> {
    let chain = start_listener_on_mock_chain().await?;
    let client = chain.client.clone();
    let contract_address = chain.contract_address;
    let received_store = chain.received_hashes.clone();

    let mock_contract = MockEmitter::new(contract_address, client.clone());
    let test_hash = [1u8; 16];
    let created_by = [2u8; 16];
    let _tx = mock_contract
        .emit_order_placed(test_hash, created_by)
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

#[tokio::test]
async fn test_listener_passes_block_timestamp_to_market_clearing_handler() -> Result<()> {
    let chain = start_listener_on_mock_chain().await?;

    let mock_contract = MockEmitter::new(chain.contract_address, chain.client.clone());
    let first_market_id = [3u8; 16];
    let second_market_id = [4u8; 16];
    let receipt = mock_contract
        .emit_market_clearings(first_market_id, second_market_id)
        .send()
        .await?
        .await?
        .expect("Transaction receipt not found");

    let block_hash = receipt.block_hash.expect("Receipt without block hash");
    let block = chain
        .client
        .get_block(block_hash)
        .await?
        .expect("Block of the transaction not found");
    let expected_timestamp = block.timestamp.as_u64();

    for _ in 0..50 {
        {
            let store = chain.received_clearings.lock().unwrap();
            if store.len() == 2 {
                // Both events of the transaction carry the timestamp of their block.
                assert_eq!(
                    *store,
                    vec![
                        (first_market_id, block_hash, expected_timestamp),
                        (second_market_id, block_hash, expected_timestamp),
                    ]
                );
                return Ok(());
            }
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    panic!("Timeout: MarketClearing events not received by listener");
}
