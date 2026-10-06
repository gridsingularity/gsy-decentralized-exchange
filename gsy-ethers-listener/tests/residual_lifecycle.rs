use anyhow::Result;
use async_trait::async_trait;
use ethers::{
    prelude::*,
    solc::{Project, ProjectPathsConfig},
    utils::Anvil,
};
use gsy_ethers_listener::{
    GsyEthersListener, GsyEventHandler, ListenerConfig, MarketStatusUpdatedFilter,
    OrderCancelledFilter, OrderPlacedFilter, TradeSettledFilter, MarketClearingFilter,
};
use std::collections::HashMap;
use std::fs::File;
use std::io::Write;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tempfile::TempDir;

mod mock_contract {
    use ethers::prelude::abigen;
    abigen!(
        MockEmitter,
        r#"[
            event OrderPlaced(bytes16 indexed orderId, bytes16 indexed createdBy, bytes16 indexed marketId, uint64 timeSlot, uint64 creationTime, uint64 energy, uint64 energyRate, uint8 energySourcePreference, uint8 energyType, bool isBid, bytes16 preferredTradingPartner, uint64 preferredEnergyRate)
            function emitOrderPlaced(bytes16 orderId, bytes16 createdBy) external
            function emitResidualBatch(address registry, bool residualIsBid, bool consumeResidual) external
            function emitFollowup(bool residualIsBid) external
            function emitMarketStatus() external
            function emitCancellation(bytes16 orderId) external
        ]"#
    );
}
use mock_contract::MockEmitter;

#[derive(Default)]
struct Projection {
    events: Vec<(&'static str, [u8; 16])>,
    orders: HashMap<[u8; 16], (OrderPlacedFilter, bool)>,
    missing_orders: Vec<[u8; 16]>,
}

struct OrderedHandler(Arc<Mutex<Projection>>);

#[async_trait]
impl GsyEventHandler for OrderedHandler {
    async fn handle_market_clearing(
        &self, _: MarketClearingFilter, _: LogMeta, _: u64,
    ) -> Result<()> {
        Ok(())
    }

    async fn handle_order_placed(&self, event: OrderPlacedFilter) -> Result<()> {
        // Allow later logs to queue while placement is being indexed.
        tokio::time::sleep(Duration::from_millis(10)).await;
        let mut state = self.0.lock().unwrap();
        state.events.push(("placed", event.order_id));
        state.orders.insert(event.order_id, (event, false));
        Ok(())
    }

    async fn handle_order_cancelled(&self, event: OrderCancelledFilter) -> Result<()> {
        self.0
            .lock()
            .unwrap()
            .events
            .push(("cancelled", event.order_id));
        Ok(())
    }

    async fn handle_trade_settled(&self, event: TradeSettledFilter) -> Result<()> {
        let mut state = self.0.lock().unwrap();
        state.events.push(("settled", event.trade_id));
        for id in [event.bid_id, event.offer_id] {
            if let Some((_, executed)) = state.orders.get_mut(&id) {
                *executed = true;
            } else {
                state.missing_orders.push(id);
            }
        }
        Ok(())
    }

    async fn handle_market_status(&self, event: MarketStatusUpdatedFilter) -> Result<()> {
        self.0
            .lock()
            .unwrap()
            .events
            .push(("market", event.market_id));
        Ok(())
    }
}

fn id(value: u8) -> [u8; 16] {
    let mut id = [0; 16];
    id[15] = value;
    id
}

async fn wait_for_events(state: &Arc<Mutex<Projection>>, count: usize) {
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if state.lock().unwrap().events.len() >= count {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("Timed out waiting for ordered events");
}

#[tokio::test]
async fn residual_events_are_processed_in_chain_order_from_the_correct_contracts() -> Result<()> {
    let anvil = Anvil::new().spawn();
    let wallet: LocalWallet = anvil.keys()[0].clone().into();
    let provider = Provider::<Ws>::connect(anvil.ws_endpoint())
        .await?
        .interval(Duration::from_millis(20));
    let client = Arc::new(SignerMiddleware::new(
        provider,
        wallet.with_chain_id(anvil.chain_id()),
    ));
    let registry_address = deploy_emitter(client.clone()).await?;
    let settlement_address = deploy_emitter(client.clone()).await?;
    let controller_address = deploy_emitter(client.clone()).await?;
    let registry = MockEmitter::new(registry_address, client.clone());
    let settlement = MockEmitter::new(settlement_address, client.clone());
    let controller = MockEmitter::new(controller_address, client);
    let state = Arc::new(Mutex::new(Projection::default()));
    let listener = GsyEthersListener::new(
        ListenerConfig {
            node_url: anvil.ws_endpoint(),
            order_registry_address: registry_address,
            trade_settlement_address: settlement_address,
            market_controller_address: controller_address,
        },
        OrderedHandler(state.clone()),
    );
    let handle = tokio::spawn(async move { listener.run().await.unwrap() });
    tokio::time::sleep(Duration::from_secs(1)).await;

    for residual_is_bid in [true, false] {
        for same_batch in [true, false] {
            *state.lock().unwrap() = Projection::default();
            let base = if residual_is_bid { 1 } else { 11 };
            settlement
                .emit_residual_batch(registry_address, residual_is_bid, same_batch)
                .send()
                .await?
                .await?;
            wait_for_events(&state, if same_batch { 6 } else { 5 }).await;
            if !same_batch {
                assert!(!state.lock().unwrap().orders[&id(base + 3)].1);
                settlement
                    .emit_followup(residual_is_bid)
                    .send()
                    .await?
                    .await?;
                wait_for_events(&state, 6).await;
            }
            let snapshot = state.lock().unwrap();
            assert_eq!(
                snapshot.events,
                vec![
                    ("placed", id(base)),
                    ("placed", id(base + 1)),
                    ("placed", id(base + 2)),
                    ("placed", id(base + 3)),
                    ("settled", id(base + 4)),
                    ("settled", id(base + 5)),
                ]
            );
            assert!(snapshot.missing_orders.is_empty());
            assert!(snapshot.orders.values().all(|(_, executed)| *executed));
            let residual = &snapshot.orders[&id(base + 3)].0;
            assert_eq!(residual.energy, 40);
            assert_eq!(residual.is_bid, residual_is_bid);
            assert_eq!(residual.market_id, id(99));
            assert_eq!(residual.time_slot, 100);
            assert_eq!(residual.creation_time, 90);
            assert_eq!(residual.energy_source_preference, 1);
            assert_eq!(residual.energy_type, 2);
        }
    }

    *state.lock().unwrap() = Projection::default();
    // Correct signatures from the wrong configured contract must not be dispatched.
    registry.emit_followup(true).send().await?.await?;
    settlement
        .emit_order_placed(id(42), id(100))
        .send()
        .await?
        .await?;
    registry.emit_market_status().send().await?.await?;
    settlement.emit_cancellation(id(42)).send().await?.await?;
    registry.emit_cancellation(id(1)).send().await?.await?;
    controller.emit_market_status().send().await?.await?;
    wait_for_events(&state, 2).await;
    assert_eq!(
        state.lock().unwrap().events,
        vec![("cancelled", id(1)), ("market", id(99))]
    );
    handle.abort();
    Ok(())
}

async fn deploy_emitter(
    client: Arc<SignerMiddleware<Provider<Ws>, LocalWallet>>,
) -> Result<Address> {
    let temp_dir = TempDir::new()?;
    let contracts_dir = temp_dir.path().join("contracts");
    std::fs::create_dir(&contracts_dir)?;

    let source_path = contracts_dir.join("MockEmitter.sol");
    let source = r#"
        // SPDX-License-Identifier: MIT
        pragma solidity ^0.8.0;
        contract MockEmitter {
            event OrderPlaced(bytes16 indexed orderId, bytes16 indexed createdBy, bytes16 indexed marketId, uint64 timeSlot, uint64 creationTime, uint64 energy, uint64 energyRate, uint8 energySourcePreference, uint8 energyType, bool isBid, bytes16 preferredTradingPartner, uint64 preferredEnergyRate);
            event TradeSettled(bytes16 indexed tradeId, bytes16 indexed bidId, bytes16 indexed offerId, bytes16 buyerId, bytes16 sellerId, bytes16 marketId, uint64 timeSlot, bytes16 residualBidId, bytes16 residualOfferId, uint256 energy, uint256 price);
            event MarketStatusUpdated(bytes16 indexed marketId, bool isOpen);
            event OrderCancelled(bytes16 indexed orderId);
            function emitOrderPlaced(bytes16 orderId, bytes16 createdBy) external {
                emit OrderPlaced(orderId, createdBy, bytes16(0), 100, 100, 1000, 50, 1, 0, true, bytes16(0), 0);
            }
            function place(uint128 id, uint64 energy, bool isBid) external {
                emit OrderPlaced(bytes16(id), bytes16(uint128(isBid ? 100 : 200)), bytes16(uint128(99)), 100, 90, energy, 50, 1, 2, isBid, bytes16(0), 0);
            }
            function emitResidualBatch(MockEmitter registry, bool residualIsBid, bool consumeResidual) external {
                uint128 base = residualIsBid ? 1 : 11;
                registry.place(base, residualIsBid ? 100 : 60, true);
                registry.place(base + 1, residualIsBid ? 60 : 100, false);
                registry.place(base + 2, 40, !residualIsBid);
                // Registry emits the residual placement before the settlement event.
                registry.place(base + 3, 40, residualIsBid);
                emit TradeSettled(bytes16(base + 4), bytes16(base), bytes16(base + 1), bytes16(uint128(100)), bytes16(uint128(200)), bytes16(uint128(99)), 100, residualIsBid ? bytes16(base + 3) : bytes16(0), residualIsBid ? bytes16(0) : bytes16(base + 3), 60, 50);
                if (consumeResidual) emitFollowup(residualIsBid);
            }
            function emitFollowup(bool residualIsBid) public {
                uint128 base = residualIsBid ? 1 : 11;
                emit TradeSettled(bytes16(base + 5), bytes16(base + (residualIsBid ? 3 : 2)), bytes16(base + (residualIsBid ? 2 : 3)), bytes16(uint128(100)), bytes16(uint128(200)), bytes16(uint128(99)), 100, bytes16(0), bytes16(0), 40, 50);
            }
            function emitMarketStatus() external {
                emit MarketStatusUpdated(bytes16(uint128(99)), true);
            }
            function emitCancellation(bytes16 orderId) external {
                emit OrderCancelled(orderId);
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

    Ok(contract_address)
}
