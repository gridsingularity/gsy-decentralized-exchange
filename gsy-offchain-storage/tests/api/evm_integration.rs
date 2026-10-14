use crate::helpers::{init_app, stop_app};
use ethers::{
    prelude::*,
    solc::{Project, ProjectPathsConfig},
    utils::Anvil,
};
use gsy_ethers_listener::{
    GsyEthersListener, GsyEventHandler, ListenerConfig, MarketStatusUpdatedFilter,
    OrderCancelledFilter, OrderPlacedFilter, TradeSettledFilter,
};
use gsy_offchain_storage::evm_handler::OffchainStorageEvmHandler;
use primitives::db_api_schema::ids::IdMappingSchema;
use primitives::db_api_schema::orders::{EnergyType, OrderEnum, OrderStatus};
use primitives::db_api_schema::trades::TradeStatus;
use primitives::ewds::dto::EwdsOrderDto;
use primitives::utils::{bytes16_to_hex, epoch_to_rfc3339};
use std::{fs::File, io::Write, sync::Arc, time::Duration};
use tempfile::TempDir;

abigen!(
    MockEmitter,
    r#"[
        event OrderPlaced(bytes16 indexed orderId, bytes16 indexed createdBy, bytes16 indexed marketId, uint64 timeSlot, uint64 creationTime, uint64 energy, uint64 energyRate, uint8 energySourcePreference, uint8 energyType, bool isBid, bytes16 preferredTradingPartner, uint64 preferredEnergyRate)
        function emitOrderPlaced(bytes16 orderId, bytes16 createdBy, uint64 energy, uint64 rate) external
        function emitMarketStatus(bool isOpen) external
    ]"#
);

#[tokio::test]
async fn test_order_listener_rejects_unknown_partner_mapping() {
    let app = init_app().await;
    let handler = OffchainStorageEvmHandler {
        db: app.db_wrapper.clone(),
        event_publisher: None,
    };
    let event = OrderPlacedFilter {
        order_id: [0xaa; 16],
        created_by: [0xbb; 16],
        market_id: [0xcc; 16],
        time_slot: 1000,
        creation_time: 900,
        energy: 10000,
        energy_rate: 5000,
        energy_source_preference: 0,
        energy_type: 0,
        is_bid: true,
        preferred_trading_partner: [0x11; 16],
        preferred_energy_rate: 4000,
    };
    let error = handler.handle_order_placed(event, 950).await.unwrap_err();
    assert!(error.to_string().contains("No facility ID mapping"));
    assert!(app
        .db_wrapper
        .orders()
        .get_all_orders()
        .await
        .unwrap()
        .is_empty());
    crate::helpers::stop_app(app).await;
}

#[tokio::test]
async fn test_evm_order_listener_persists_to_db() {
    let app = init_app().await;
    let db = app.db_wrapper.clone();
    // Register external facility IDs independently of their on-chain representation.
    for (offchain_id, onchain_id) in [(
        "00112233-4455-6677-8899-aabbccddeeff",
        "0x11111111111111111111111111111111",
    )] {
        db.ids()
            .insert_one(IdMappingSchema {
                offchain_id: offchain_id.to_string(),
                onchain_id: onchain_id.to_string(),
                creation_time: 1,
            })
            .await
            .unwrap();
    }

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
            event MarketStatusUpdated(bytes16 indexed marketId, bool isOpen);
            function emitMarketStatus(bool isOpen) external {
                emit MarketStatusUpdated(bytes16(0), isOpen);
            }
            event OrderPlaced(bytes16 indexed orderId, bytes16 indexed createdBy, bytes16 indexed marketId, uint64 timeSlot, uint64 creationTime, uint64 energy, uint64 energyRate, uint8 energySourcePreference, uint8 energyType, bool isBid, bytes16 preferredTradingPartner, uint64 preferredEnergyRate);
            function emitOrderPlaced(bytes16 orderId, bytes16 createdBy, uint64 energy, uint64 rate) external {
                // Emit representative order metadata so indexing is verified end-to-end.
                emit OrderPlaced(
                    orderId,
                    createdBy,
                    bytes16(0),
                    1000,
                    1234567890,
                    energy,
                    rate,
                    1,
                    2,
                    true,
                    hex"11111111111111111111111111111111",
                    110000
                );
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
        market_controller_address: contract_address,
    };

    let handler = OffchainStorageEvmHandler {
        db: db.clone(),
        event_publisher: None,
    };
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

    let receipt = mock_contract
        .emit_order_placed(order_id, created_by, energy_val, rate_val)
        .send()
        .await
        .unwrap()
        .await
        .unwrap()
        .unwrap();
    let block_timestamp = client
        .get_block(receipt.block_hash.unwrap())
        .await
        .unwrap()
        .unwrap()
        .timestamp
        .as_u64();

    let expected_id = format!("0x{}", hex::encode(order_id));

    let mut found = false;
    for _ in 0..20 {
        let order_bson = mongodb::bson::to_bson(&expected_id).unwrap();
        if let Ok(Some(order)) = db.orders().get_order_by_id(&order_bson).await {
            found = true;
            assert_eq!(order.order_type, OrderEnum::Bid);
            assert_eq!(order.energy_kWh, 1.0);
            assert_eq!(order.energy_rate, 0.5);
            assert_eq!(order.creation_time, 1234567890);
            assert_eq!(order.status_updated_at, Some(block_timestamp));
            assert_eq!(order.area_uuid, format!("0x{}", hex::encode(created_by)));
            assert_eq!(
                order
                    .requirements
                    .as_ref()
                    .and_then(|requirements| requirements.energy_type.clone()),
                Some(EnergyType::Green)
            );
            assert_eq!(
                order
                    .requirements
                    .as_ref()
                    .and_then(|requirements| requirements.trading_partner_id.as_deref()),
                Some("00112233-4455-6677-8899-aabbccddeeff")
            );
            assert_eq!(
                order
                    .requirements
                    .as_ref()
                    .and_then(|requirements| requirements.preferred_energy_rate),
                Some(11.0)
            );
            let attributes = order.attributes.as_ref().expect("attributes missing");
            assert_eq!(attributes.energy_type, EnergyType::Pv);
            break;
        }
        tokio::time::sleep(Duration::from_millis(200)).await;
    }

    assert!(found, "Order was not found in MongoDB after 4 seconds");
    let receipt = mock_contract
        .emit_market_status(false)
        .send()
        .await
        .unwrap()
        .await
        .unwrap()
        .unwrap();
    let closed_at = client
        .get_block(receipt.block_hash.unwrap())
        .await
        .unwrap()
        .unwrap()
        .timestamp
        .as_u64();
    let order_id = expected_id.into();
    tokio::time::timeout(Duration::from_secs(4), async {
        loop {
            let order = db
                .orders()
                .get_order_by_id(&order_id)
                .await
                .unwrap()
                .unwrap();
            if order.status == OrderStatus::Expired {
                assert_eq!(order.status_updated_at, Some(closed_at));
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("Market closure did not expire the indexed order");
    stop_app(app).await;
}

fn placed_order(order_id: [u8; 16], is_bid: bool, energy: u64) -> OrderPlacedFilter {
    OrderPlacedFilter {
        order_id,
        created_by: if is_bid { [0xaa; 16] } else { [0xbb; 16] },
        market_id: [0xcc; 16],
        time_slot: 1000,
        creation_time: 900,
        energy,
        energy_rate: 5000,
        energy_source_preference: 0,
        energy_type: 0,
        is_bid,
        preferred_trading_partner: [0; 16],
        preferred_energy_rate: 0,
    }
}

#[tokio::test]
async fn cancellation_uses_block_time_and_preserves_order_metadata() {
    let app = init_app().await;
    let handler = OffchainStorageEvmHandler {
        db: app.db_wrapper.clone(),
        event_publisher: None,
    };
    handler
        .handle_order_placed(placed_order([1; 16], true, 10000), 950)
        .await
        .unwrap();
    let orders = app.db_wrapper.orders();
    let order_id = bytes16_to_hex([1; 16]).into();
    let mut expected = orders.get_order_by_id(&order_id).await.unwrap().unwrap();
    assert_eq!(expected.status_updated_at, Some(950));
    expected.status = OrderStatus::Cancelled;
    expected.status_updated_at = Some(980);
    // Reprocessing the same event must not replace chain time with wall-clock time.
    for _ in 0..2 {
        handler
            .handle_order_cancelled(OrderCancelledFilter { order_id: [1; 16] }, 980)
            .await
            .unwrap();
        assert_eq!(
            orders.get_order_by_id(&order_id).await.unwrap().unwrap(),
            expected
        );
    }
    stop_app(app).await;
}

#[tokio::test]
async fn market_closure_expires_only_submitted_orders_and_preserves_expiry_on_reopen() {
    // Closure, not the delivery slot (1000), determines expiry in both cases.
    for closed_at in [980, 1020] {
        let app = init_app().await;
        let handler = OffchainStorageEvmHandler {
            db: app.db_wrapper.clone(),
            event_publisher: None,
        };
        let orders = app.db_wrapper.orders();
        let market_id = [0xcc; 16];
        let other_market = [0xdd; 16];
        for (index, status) in [
            OrderStatus::Submitted,
            OrderStatus::PartiallyFilled,
            OrderStatus::Filled,
            OrderStatus::Cancelled,
            OrderStatus::Expired,
            OrderStatus::Executed,
        ]
        .into_iter()
        .enumerate()
        {
            for (offset, market) in [(0, market_id), (10, other_market)] {
                for (side, is_bid) in [(0, true), (20, false)] {
                    let order_id = [(index + offset + side + 1) as u8; 16];
                    let mut event = placed_order(order_id, is_bid, 10000);
                    event.market_id = market;
                    handler.handle_order_placed(event, 950).await.unwrap();
                    orders
                        .update_order_status_by_id(
                            &bytes16_to_hex(order_id).into(),
                            status.clone(),
                            950,
                        )
                        .await
                        .unwrap();
                }
            }
        }
        let before = orders.get_all_orders().await.unwrap();
        handler
            .handle_market_status(
                MarketStatusUpdatedFilter {
                    market_id,
                    is_open: true,
                },
                closed_at,
            )
            .await
            .unwrap();
        for order in &before {
            assert_eq!(
                orders
                    .get_order_by_id(&order.order_id.clone().into())
                    .await
                    .unwrap()
                    .as_ref(),
                Some(order)
            );
        }
        // A repeated close or reopen must not change terminal statuses or their timestamps.
        for (is_open, timestamp) in [
            (false, closed_at),
            (false, closed_at + 1),
            (true, closed_at + 2),
        ] {
            handler
                .handle_market_status(MarketStatusUpdatedFilter { market_id, is_open }, timestamp)
                .await
                .unwrap();
            for order in &before {
                let mut expected = order.clone();
                if order.market_id == bytes16_to_hex(market_id)
                    && order.status == OrderStatus::Submitted
                {
                    expected.status = OrderStatus::Expired;
                    expected.status_updated_at = Some(closed_at);
                }
                assert_eq!(
                    orders
                        .get_order_by_id(&order.order_id.clone().into())
                        .await
                        .unwrap(),
                    Some(expected)
                );
            }
        }
        let response = reqwest::Client::new()
            .get(format!("{}/orders", app.address))
            .send()
            .await
            .unwrap()
            .error_for_status()
            .unwrap()
            .json::<Vec<EwdsOrderDto>>()
            .await
            .unwrap();
        let expired = response
            .iter()
            .find(|order| order.order_id == bytes16_to_hex([1; 16]))
            .unwrap();
        assert_eq!(expired.order_status, "expired");
        assert_eq!(expired.updated_at, Some(epoch_to_rfc3339(closed_at)));
        handler
            .handle_order_placed(placed_order([99; 16], true, 10000), closed_at + 3)
            .await
            .unwrap();
        let new_order = orders
            .get_order_by_id(&bytes16_to_hex([99; 16]).into())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(new_order.status, OrderStatus::Submitted);
        stop_app(app).await;
    }
}

#[tokio::test]
async fn settlement_indexes_fill_statuses_and_keeps_residuals_independent() {
    for (partial_bid, partial_offer) in [(false, false), (true, false), (false, true), (true, true)]
    {
        let app = init_app().await;
        let handler = OffchainStorageEvmHandler {
            db: app.db_wrapper.clone(),
            event_publisher: None,
        };
        let sides = [
            (true, [1; 16], [3; 16], partial_bid),
            (false, [2; 16], [4; 16], partial_offer),
        ];
        for (is_bid, original, residual, partial) in sides {
            handler
                .handle_order_placed(
                    placed_order(original, is_bid, if partial { 20000 } else { 10000 }),
                    950,
                )
                .await
                .unwrap();
            let submitted = app
                .db_wrapper
                .orders()
                .get_order_by_id(&bytes16_to_hex(original).into())
                .await
                .unwrap()
                .unwrap();
            assert_eq!(submitted.status_updated_at, Some(950));
            assert_eq!(submitted.creation_time, 900);
            if partial {
                handler
                    .handle_order_placed(placed_order(residual, is_bid, 10000), 980)
                    .await
                    .unwrap();
            }
        }
        let event = TradeSettledFilter {
            trade_id: [10; 16],
            bid_id: [1; 16],
            offer_id: [2; 16],
            buyer_id: [0xaa; 16],
            seller_id: [0xbb; 16],
            market_id: [0xcc; 16],
            time_slot: 1000,
            residual_bid_id: if partial_bid { [3; 16] } else { [0; 16] },
            residual_offer_id: if partial_offer { [4; 16] } else { [0; 16] },
            energy: U256::from(10000u64),
            price: U256::from(5000u64),
        };
        handler
            .handle_trade_settled(event.clone(), 980)
            .await
            .unwrap();
        let orders = app.db_wrapper.orders();
        let indexed = orders.get_all_orders().await.unwrap();
        let returned = reqwest::Client::new()
            .get(format!("{}/orders", app.address))
            .send()
            .await
            .unwrap()
            .error_for_status()
            .unwrap()
            .json::<Vec<EwdsOrderDto>>()
            .await
            .unwrap();

        for (is_bid, original, residual, partial) in sides {
            let original_id = bytes16_to_hex(original);
            let original_order = indexed
                .iter()
                .find(|order| order.order_id == original_id)
                .unwrap();
            assert_eq!(
                original_order.status,
                if partial {
                    OrderStatus::PartiallyFilled
                } else {
                    OrderStatus::Filled
                }
            );
            assert_eq!(original_order.energy_kWh, if partial { 2.0 } else { 1.0 });
            assert_eq!(original_order.status_updated_at, Some(980));
            assert_eq!(original_order.creation_time, 900);
            let dto = returned
                .iter()
                .find(|order| order.order_id == original_id)
                .unwrap();
            assert_eq!(dto.updated_at, Some(epoch_to_rfc3339(980)));
            assert_eq!(
                dto.order_status,
                if partial {
                    "partially_filled"
                } else {
                    "filled"
                }
            );
            if !partial {
                continue;
            }
            let residual_id = bytes16_to_hex(residual);
            let mut expected_residual = original_order.clone();
            expected_residual.order_id = residual_id.clone();
            expected_residual.energy_kWh = 1.0;
            expected_residual.status = OrderStatus::Submitted;
            assert_eq!(
                indexed
                    .iter()
                    .find(|order| order.order_id == residual_id)
                    .unwrap(),
                &expected_residual
            );

            let counterpart = if is_bid { [5; 16] } else { [6; 16] };
            handler
                .handle_order_placed(placed_order(counterpart, !is_bid, 10000), 990)
                .await
                .unwrap();
            handler
                .handle_trade_settled(
                    TradeSettledFilter {
                        trade_id: if is_bid { [11; 16] } else { [12; 16] },
                        bid_id: if is_bid { residual } else { counterpart },
                        offer_id: if is_bid { counterpart } else { residual },
                        residual_bid_id: [0; 16],
                        residual_offer_id: [0; 16],
                        ..event.clone()
                    },
                    1000,
                )
                .await
                .unwrap();

            expected_residual.status = OrderStatus::Filled;
            expected_residual.status_updated_at = Some(1000);
            let stored_residual = orders
                .get_order_by_id(&residual_id.into())
                .await
                .unwrap()
                .unwrap();
            assert_eq!(stored_residual, expected_residual);
            let stored_original = orders
                .get_order_by_id(&original_id.into())
                .await
                .unwrap()
                .unwrap();
            assert_eq!(&stored_original, original_order);
            let stored_counterpart = orders
                .get_order_by_id(&bytes16_to_hex(counterpart).into())
                .await
                .unwrap()
                .unwrap();
            assert_eq!(stored_counterpart.status, OrderStatus::Filled);
            assert_eq!(stored_counterpart.status_updated_at, Some(1000));
        }
        let trades = app.db_wrapper.trades().get_all_trades().await.unwrap();
        assert_eq!(
            trades.len(),
            1 + usize::from(partial_bid) + usize::from(partial_offer)
        );
        assert!(trades
            .iter()
            .all(|trade| trade.status == TradeStatus::Settled));
        stop_app(app).await;
    }
}
