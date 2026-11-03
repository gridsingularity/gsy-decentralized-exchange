use crate::helpers::{init_app, stop_app};
use anyhow::{Context, Result};
use ethers::{
    abi::{Abi, Tokenize},
    prelude::*,
    utils::{keccak256, Anvil},
};
use gsy_ethers_listener::{GsyEthersListener, ListenerConfig};
use gsy_execution_engine::{
    connectors::evm_connector::complete_delivery_verification,
    primitives::penalty_calculator::DeliveryVerification,
    services::execution_orchestrator::run_execution_cycle,
};
use gsy_offchain_storage::{
    db::measurements_service::insert_measurements, evm_handler::OffchainStorageEvmHandler,
};
use primitives::{
    db_api_schema::{
        grid_topology::FacilitySchema, orders::OrderStatus, profiles::MeasurementSchema,
        trades::TradeStatus,
    },
    utils::{bytes16_to_hex, create_encrypted_bytes16_from_string},
};
use std::{path::PathBuf, sync::Arc, time::Duration};

type Client = SignerMiddleware<Provider<Ws>, LocalWallet>;

async fn deploy_verification_contracts(
    client: Arc<Client>,
) -> Result<(Contract<Client>, Contract<Client>, Contract<Client>)> {
    let admin = client.address();
    let controller = deploy(client.clone(), "MarketController", admin).await?;
    let registry = deploy(
        client.clone(),
        "OrderRegistry",
        (admin, controller.address()),
    )
    .await?;
    let settlement = deploy(client, "TradeSettlement", (admin, registry.address())).await?;
    for (contract, role, account) in [
        (&controller, "ORCHESTRATOR_ROLE", admin),
        (&registry, "COMMUNITY_CLIENT_ROLE", admin),
        (&registry, "SETTLEMENT_ROLE", settlement.address()),
        (&settlement, "OPERATOR_ROLE", admin),
        (&settlement, "EXECUTION_ENGINE_ROLE", admin),
    ] {
        send(contract, "grantRole", (keccak256(role), account)).await?;
    }
    Ok((controller, registry, settlement))
}

fn artifact(path: &str) -> Result<(Abi, Bytes)> {
    let root = std::env::var_os("GSY_CONTRACT_ARTIFACTS")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../gsy-contracts/artifacts")
        });
    let json: serde_json::Value = serde_json::from_slice(
        &std::fs::read(root.join(path))
            .context("Run `npx hardhat compile` in gsy-contracts before this test")?,
    )?;
    Ok((
        serde_json::from_value(json["abi"].clone())?,
        json["bytecode"].as_str().unwrap().parse()?,
    ))
}

async fn deploy(client: Arc<Client>, name: &str, args: impl Tokenize) -> Result<Contract<Client>> {
    let (abi, bytecode) = artifact(&format!("contracts/{name}.sol/{name}.json"))?;
    let implementation = ContractFactory::new(abi.clone(), bytecode, client.clone())
        .deploy(())?
        .send()
        .await?;
    let init = abi
        .function("initialize")?
        .encode_input(&args.into_tokens())?;
    let (proxy_abi, proxy_bytecode) = artifact("@openzeppelin/contracts/proxy/transparent/TransparentUpgradeableProxy.sol/TransparentUpgradeableProxy.json")?;
    let proxy = ContractFactory::new(proxy_abi, proxy_bytecode, client.clone())
        .deploy((
            implementation.address(),
            client.address(),
            Bytes::from(init),
        ))?
        .send()
        .await?;
    Ok(Contract::new(proxy.address(), abi, client))
}

async fn send(contract: &Contract<Client>, name: &str, args: impl Tokenize) -> Result<()> {
    let call = contract.method::<_, ()>(name, args)?;
    let receipt = call.send().await?.await?.context("Missing receipt")?;
    anyhow::ensure!(receipt.status == Some(1u64.into()), "Transaction reverted");
    Ok(())
}

#[tokio::test]
#[ignore = "requires compiled Hardhat contracts, Anvil and MongoDB; run with --ignored"]
async fn delivery_verification_runs_through_real_contracts_engine_and_storage() -> Result<()> {
    let app = init_app().await;
    let anvil = Anvil::new().spawn();
    let provider = Provider::<Ws>::connect(anvil.ws_endpoint())
        .await?
        .interval(Duration::from_millis(10));
    let wallet: LocalWallet = anvil.keys()[0].clone().into();
    let private_key = format!("0x{}", hex::encode(anvil.keys()[0].to_bytes()));
    let client = Arc::new(SignerMiddleware::new(
        provider,
        wallet.with_chain_id(anvil.chain_id()),
    ));
    let (controller, registry, settlement) = deploy_verification_contracts(client.clone()).await?;

    let listener = GsyEthersListener::new(
        ListenerConfig {
            node_url: anvil.ws_endpoint(),
            order_registry_address: registry.address(),
            trade_settlement_address: settlement.address(),
            market_controller_address: controller.address(),
        },
        OffchainStorageEvmHandler {
            db: app.db_wrapper.clone(),
            event_publisher: None,
        },
    );
    let listener_task = tokio::spawn(async move { listener.run().await });
    tokio::time::sleep(Duration::from_secs(1)).await;

    let market_id = [0x99u8; 16];
    let slot = 1800u64;
    let now = client
        .get_block(BlockNumber::Latest)
        .await?
        .unwrap()
        .timestamp
        .as_u64();
    send(
        &controller,
        "createMarkets",
        ((0..2u8)
            .map(|index| {
                (
                    [0x99 + index; 16],
                    [0x77u8; 16],
                    now,
                    now + 3600,
                    slot + u64::from(index) * 900,
                    slot + u64::from(index + 1) * 900,
                    0u8,
                    0u8,
                )
            })
            .collect::<Vec<_>>(),),
    )
    .await?;
    let mut measurements = Vec::new();
    let mut seller_measurements = Vec::new();
    for index in 0..3u8 {
        // Reuse one buyer across two slots. A broad query must not combine their
        // traded totals or let the earlier meter reading verify the later trade.
        let buyer_index = if index == 2 { 0 } else { index };
        let buyer = format!("buyer-{buyer_index}");
        let facility = format!("meter-{buyer_index}");
        let market_id = if index == 2 { [0x9a; 16] } else { market_id };
        let slot = if index == 2 { slot + 900 } else { slot };
        let seller_facility = format!("seller-meter-{index}");
        let buyer_id = create_encrypted_bytes16_from_string(&facility);
        let seller_id = create_encrypted_bytes16_from_string(&seller_facility);
        let bid = (
            [index + 1; 16],
            buyer_id,
            market_id,
            slot,
            900u64,
            20000u64,
            200000u64,
            0u8,
            0u8,
            true,
            [0u8; 16],
            0u64,
        );
        let offer = (
            [index + 11; 16],
            seller_id,
            market_id,
            slot,
            900u64,
            20000u64,
            100000u64,
            0u8,
            0u8,
            false,
            [0u8; 16],
            0u64,
        );
        send(&registry, "placeOrder", (bid,)).await?;
        send(&registry, "placeOrder", (offer,)).await?;
        let matched = (
            [index + 21; 16],
            bid,
            offer,
            [index + 31; 16],
            [index + 41; 16],
            U256::from(10000),
            U256::from(150000),
            0u8, // Standard match.
        );
        let clearing = (
            market_id,
            1u8,
            U256::from(150000),
            U256::from(20000),
            U256::from(20000),
            U256::from(10000),
            1u32,
        );
        send(
            &settlement,
            "settleBatch",
            (vec![(vec![matched], clearing)],),
        )
        .await?;
        app.db_wrapper
            .facilities()
            .upsert(FacilitySchema {
                facility_id: facility.clone(),
                facility_name: facility.clone(),
                site_id: "site".into(),
                owner_id: buyer,
            })
            .await?;
        measurements.push(MeasurementSchema {
            facility_id: facility,
            community_uuid: "community".into(),
            time_slot: slot,
            creation_time: slot,
            energy_kwh: [1.0, 1.0, 1.2][index as usize],
        });
        seller_measurements.push(MeasurementSchema {
            facility_id: seller_facility,
            community_uuid: "community".into(),
            time_slot: slot,
            creation_time: slot,
            energy_kwh: [-1.0, -0.8, -1.0][index as usize],
        });
    }
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            if app
                .db_wrapper
                .trades()
                .get_all_trades()
                .await
                .unwrap()
                .len()
                == 3
            {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await?;
    let confirmed = app.db_wrapper.trades().get_all_trades().await?;
    let mapping_count = app
        .db_wrapper
        .ids()
        .count_documents(mongodb::bson::doc! {})
        .await?;
    assert!(confirmed.iter().all(|trade| {
        trade.status == TradeStatus::Matched
            && trade.status_updated_at == Some(trade.creation_time)
    }));
    let endpoint = anvil.ws_endpoint();
    let address = format!("{:?}", settlement.address());
    // Nothing measured: no completion. Only part measured: complete that part,
    // but keep the slot pending until the last measurement arrives.
    assert_eq!(
        run_execution_cycle(
            &app.address,
            &endpoint,
            &address,
            &private_key,
            slot,
            0.1,
            1800,
            5.0
        )
        .await?,
        0
    );
    insert_measurements(&app.db_wrapper, &measurements[..2]).await?;
    // Buyers alone cannot complete either side of a trade.
    run_execution_cycle(
        &app.address, &endpoint, &address, &private_key, slot, 0.1, 1800, 5.0,
    )
    .await?;
    assert!(app
        .db_wrapper
        .trades()
        .get_all_trades()
        .await?
        .iter()
        .all(|trade| trade.status == TradeStatus::Matched));
    assert!(settlement
        .event::<gsy_ethers_listener::DeliveryVerifiedFilter>()
        .from_block(0u64)
        .query()
        .await?
        .is_empty());
    insert_measurements(&app.db_wrapper, &seller_measurements[..2]).await?;
    assert_eq!(
        run_execution_cycle(
            &app.address,
            &endpoint,
            &address,
            &private_key,
            slot,
            0.1,
            1800,
            5.0
        )
        .await?,
        0
    );
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            let trades = app.db_wrapper.trades().get_all_trades().await.unwrap();
            if trades
                .iter()
                .filter(|trade| {
                    matches!(trade.status, TradeStatus::Executed | TradeStatus::Penalized)
                })
                .count()
                == 2
            {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await?;
    // The later seller is measured, but this buyer's earlier-slot reading must
    // not substitute for its still-missing later-slot reading.
    insert_measurements(&app.db_wrapper, &seller_measurements[2..]).await?;
    assert_eq!(
        run_execution_cycle(
            &app.address, &endpoint, &address, &private_key, slot, 0.1, 1800, 5.0,
        )
        .await?,
        0
    );
    assert_eq!(
        registry
            .method::<_, u8>("getStatus", [3u8; 16])?
            .call()
            .await?,
        5
    ); // still partially filled

    // A penalty submitted separately must not prevent the engine from finishing
    // verification later, or be counted a second time by that verification.
    send(
        &settlement,
        "submitPenalties",
        (vec![(
            create_encrypted_bytes16_from_string("meter-0"),
            [0x9au8; 16],
            [23u8; 16],
            200u64,
        )],),
    )
    .await?;
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            let trade = app
                .db_wrapper
                .trades()
                .find_one(mongodb::bson::doc! {
                    "trade_uuid": bytes16_to_hex([23; 16])
                })
                .await
                .unwrap()
                .unwrap();
            if trade.status == TradeStatus::Penalized {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await?;
    let pending_order = app
        .db_wrapper
        .orders()
        .get_order_by_id(&bytes16_to_hex([3; 16]).into())
        .await?
        .unwrap();
    assert_eq!(pending_order.status, OrderStatus::PartiallyFilled);
    assert_eq!(
        app.db_wrapper.ids().count_documents(mongodb::bson::doc! {}).await?,
        mapping_count,
        "Execution cycles must not create facility ID mappings"
    );
    insert_measurements(&app.db_wrapper, &measurements[2..]).await?;
    assert_eq!(
        run_execution_cycle(
            &app.address,
            &endpoint,
            &address,
            &private_key,
            slot,
            0.1,
            1800,
            5.0
        )
        .await?,
        2 // The already verified Penalized trade is skipped on-chain.
    );
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            let orders = app.db_wrapper.orders().get_all_orders().await.unwrap();
            if orders
                .iter()
                .filter(|order| order.status == OrderStatus::Executed)
                .count()
                == 6
            {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await?;
    let trades = app.db_wrapper.trades().get_all_trades().await?;
    let mut replay = Vec::new();
    for index in 0..3u8 {
        let trade_id = [index + 21; 16];
        let penalty = if index == 0 { 0u64 } else { 200 };
        assert_eq!(
            settlement
                .method::<_, U256>("penaltyEnergyByTrade", trade_id)?
                .call()
                .await?,
            U256::from(penalty)
        );
        let trade = trades
            .iter()
            .find(|trade| trade.trade_uuid == bytes16_to_hex(trade_id))
            .unwrap();
        assert_eq!(
            trade.status,
            if index == 0 {
                TradeStatus::Executed
            } else {
                TradeStatus::Penalized
            }
        );
        let events = settlement
            .event::<gsy_ethers_listener::DeliveryVerifiedFilter>()
            .from_block(0u64)
            .query_with_meta()
            .await?;
        let (_, meta) = events
            .iter()
            .find(|(event, _)| event.trade_id == trade_id)
            .unwrap();
        let timestamp = client
            .get_block(meta.block_hash)
            .await?
            .unwrap()
            .timestamp
            .as_u64();
        let verdict_timestamp = if index == 0 {
            timestamp
        } else {
            let penalties = settlement
                .event::<gsy_ethers_listener::PenaltyRecordedFilter>()
                .from_block(0u64)
                .query_with_meta()
                .await?;
            let (_, penalty_meta) = penalties
                .iter()
                .find(|(event, _)| event.trade_id == trade_id)
                .unwrap();
            client
                .get_block(penalty_meta.block_hash)
                .await?
                .unwrap()
                .timestamp
                .as_u64()
        };
        assert_eq!(trade.status_updated_at, Some(verdict_timestamp));
        for id in [[index + 1; 16], [index + 11; 16]] {
            assert_eq!(registry.method::<_, u8>("getStatus", id)?.call().await?, 2);
            let order = app
                .db_wrapper
                .orders()
                .get_order_by_id(&bytes16_to_hex(id).into())
                .await?
                .unwrap();
            assert_eq!(order.status, OrderStatus::Executed);
            assert_eq!(order.status_updated_at, Some(timestamp));
        }
        for id in [[index + 31; 16], [index + 41; 16]] {
            assert_eq!(registry.method::<_, u8>("getStatus", id)?.call().await?, 1);
            let order = app
                .db_wrapper
                .orders()
                .get_order_by_id(&bytes16_to_hex(id).into())
                .await?
                .unwrap();
            assert_eq!(order.status, OrderStatus::Submitted);
        }
        replay.push(DeliveryVerification {
            trade_uuid: bytes16_to_hex(trade_id),
            buyer_penalty_cost: if index == 2 { penalty } else { 0 },
            seller_penalty_cost: if index == 1 { penalty } else { 0 },
        });
    }
    let before_block = client.get_block_number().await?;
    assert_eq!(
        complete_delivery_verification(&endpoint, &address, &private_key, replay).await?,
        3
    );
    assert_eq!(client.get_block_number().await?, before_block); // no duplicate transaction
    assert_eq!(
        run_execution_cycle(
            &app.address,
            &endpoint,
            &address,
            &private_key,
            slot,
            0.1,
            1800,
            5.0
        )
        .await?,
        2
    );
    assert_eq!(client.get_block_number().await?, before_block);
    assert_eq!(app.db_wrapper.trades().get_all_trades().await?, trades);
    assert_eq!(
        app.db_wrapper
            .trades()
            .filter_trades_by_status_change(0, None, Some(TradeStatus::Executed))
            .await?
            .len(),
        1
    );
    assert_eq!(
        app.db_wrapper
            .trades()
            .filter_trades_by_status_change(0, None, Some(TradeStatus::Penalized))
            .await?
            .len(),
        2
    );
    listener_task.abort();
    stop_app(app).await;
    Ok(())
}

#[tokio::test]
#[ignore = "requires compiled Hardhat contracts, Anvil and MongoDB; run with --ignored"]
async fn delivery_verification_allocates_net_slot_penalties_without_recharging_completed_trades(
) -> Result<()> {
    // A buys 4 + 6 and sells 2: net consumption 8, gross allocation weight 12.
    // With a reading of 10, its 2,000 penalty units split 667 / 1,000 / 333.
    // With a reading of 8, the first trade becomes Executed (not Penalized),
    // and must still contribute to the baseline when later meters arrive.
    for (a_reading, a_shares) in [(10.0, [667u64, 1000, 333]), (8.0, [0u64; 3])] {
        let app = init_app().await;
        let anvil = Anvil::new().spawn();
        let endpoint = anvil.ws_endpoint();
        let provider = Provider::<Ws>::connect(&endpoint)
            .await?
            .interval(Duration::from_millis(10));
        let wallet: LocalWallet = anvil.keys()[0].clone().into();
        let private_key = format!("0x{}", hex::encode(anvil.keys()[0].to_bytes()));
        let client = Arc::new(SignerMiddleware::new(
            provider,
            wallet.with_chain_id(anvil.chain_id()),
        ));
        let (controller, registry, settlement) =
            deploy_verification_contracts(client.clone()).await?;
        let address = format!("{:?}", settlement.address());
        let listener = GsyEthersListener::new(
            ListenerConfig {
                node_url: endpoint.clone(),
                order_registry_address: registry.address(),
                trade_settlement_address: settlement.address(),
                market_controller_address: controller.address(),
            },
            OffchainStorageEvmHandler {
                db: app.db_wrapper.clone(),
                event_publisher: None,
            },
        );
        let listener_task = tokio::spawn(async move { listener.run().await });
        tokio::time::sleep(Duration::from_secs(1)).await;
        let market_id = [0x99u8; 16];
        let slot = 1800u64;
        let now = client
            .get_block(BlockNumber::Latest)
            .await?
            .unwrap()
            .timestamp
            .as_u64();
        send(
            &controller,
            "createMarkets",
            (vec![(
                market_id,
                [0x77u8; 16],
                now,
                now + 3600,
                slot,
                slot + 900,
                0u8,
                0u8,
            )],),
        )
        .await?;
        let pairs = [("a", "b", 40000u64), ("a", "c", 60000), ("d", "a", 20000)];
        for (index, (buyer, seller, energy)) in pairs.iter().enumerate() {
            let index = index as u8;
            let bid = (
                [index + 1; 16],
                create_encrypted_bytes16_from_string(buyer),
                market_id,
                slot,
                now,
                *energy,
                200000u64,
                0u8,
                0u8,
                true,
                [0u8; 16],
                0u64,
            );
            let offer = (
                [index + 11; 16],
                create_encrypted_bytes16_from_string(seller),
                market_id,
                slot,
                now,
                *energy,
                100000u64,
                0u8,
                0u8,
                false,
                [0u8; 16],
                0u64,
            );
            send(&registry, "placeOrder", (bid,)).await?;
            send(&registry, "placeOrder", (offer,)).await?;
            let matched = (
                [index + 21; 16],
                bid,
                offer,
                [0u8; 16],
                [0u8; 16],
                U256::from(*energy),
                U256::from(150000),
                0u8,
            );
            let clearing = (
                market_id,
                1u8,
                U256::from(150000),
                U256::from(*energy),
                U256::from(*energy),
                U256::from(*energy),
                1u32,
            );
            send(
                &settlement,
                "settleBatch",
                (vec![(vec![matched], clearing)],),
            )
            .await?;
        }
        tokio::time::timeout(Duration::from_secs(10), async {
            while app
                .db_wrapper
                .trades()
                .get_all_trades()
                .await
                .unwrap()
                .len()
                != 3
            {
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        })
        .await?;
        let measurement = |facility: &str, energy| MeasurementSchema {
            facility_id: facility.into(),
            community_uuid: "community".into(),
            time_slot: slot,
            creation_time: slot,
            energy_kwh: energy,
        };
        insert_measurements(
            &app.db_wrapper,
            &[measurement("a", a_reading), measurement("b", -4.0)],
        )
        .await?;
        assert_eq!(
            run_execution_cycle(
                &app.address,
                &endpoint,
                &address,
                &private_key,
                slot,
                0.1,
                900,
                5.0,
            )
            .await?,
            0,
            "Missing counterparty readings must keep the slot pending"
        );
        let first_status = if a_shares[0] == 0 {
            TradeStatus::Executed
        } else {
            TradeStatus::Penalized
        };
        tokio::time::timeout(Duration::from_secs(10), async {
            loop {
                let trades = app.db_wrapper.trades().get_all_trades().await.unwrap();
                if trades.iter().any(|trade| {
                    trade.trade_uuid == bytes16_to_hex([21; 16]) && trade.status == first_status
                }) {
                    assert_eq!(
                        trades
                            .iter()
                            .filter(|trade| trade.status == TradeStatus::Matched)
                            .count(),
                        2
                    );
                    break;
                }
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        })
        .await?;
        assert_eq!(
            settlement
                .event::<gsy_ethers_listener::DeliveryVerifiedFilter>()
                .from_block(0u64)
                .query()
                .await?
                .len(),
            1
        );
        assert_eq!(
            settlement
                .method::<_, U256>("penaltyEnergyByTrade", [21u8; 16])?
                .call()
                .await?,
            U256::from(a_shares[0])
        );
        // Repeating while C and D are missing must not charge A again.
        let block = client.get_block_number().await?;
        run_execution_cycle(
            &app.address,
            &endpoint,
            &address,
            &private_key,
            slot,
            0.1,
            900,
            5.0,
        )
        .await?;
        assert_eq!(client.get_block_number().await?, block);

        // C underproduces by 1 (1,000 penalty units); D overconsumes by .2 (200).
        insert_measurements(
            &app.db_wrapper,
            &[measurement("c", -5.0), measurement("d", 2.2)],
        )
        .await?;
        run_execution_cycle(
            &app.address,
            &endpoint,
            &address,
            &private_key,
            slot,
            0.1,
            900,
            5.0,
        )
        .await?;
        tokio::time::timeout(Duration::from_secs(10), async {
            while app
                .db_wrapper
                .orders()
                .get_all_orders()
                .await
                .unwrap()
                .iter()
                .filter(|order| order.status == OrderStatus::Executed)
                .count()
                != 6
            {
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        })
        .await?;
        let penalties = settlement
            .event::<gsy_ethers_listener::PenaltyRecordedFilter>()
            .from_block(0u64)
            .query()
            .await?;
        let verifications = settlement
            .event::<gsy_ethers_listener::DeliveryVerifiedFilter>()
            .from_block(0u64)
            .query()
            .await?;
        let trades = app.db_wrapper.trades().get_all_trades().await?;
        assert_eq!(verifications.len(), 3);
        let costs = [(a_shares[0], 0u64), (a_shares[1], 1000), (200, a_shares[2])];
        assert_eq!(
            penalties.len(),
            costs
                .iter()
                .map(|(buyer, seller)| usize::from(*buyer > 0) + usize::from(*seller > 0))
                .sum::<usize>()
        );
        for (index, ((buyer, seller, _), (buyer_cost, seller_cost))) in
            pairs.iter().zip(costs).enumerate()
        {
            let index = index as u8;
            let trade_id = [index + 21; 16];
            assert_eq!(
                verifications
                    .iter()
                    .filter(|event| event.trade_id == trade_id)
                    .count(),
                1
            );
            for (party, cost) in [(buyer, buyer_cost), (seller, seller_cost)] {
                assert_eq!(
                    penalties
                        .iter()
                        .filter(|event| event.trade_id == trade_id
                            && event.penalized_actor_id
                                == create_encrypted_bytes16_from_string(party))
                        .map(|event| event.penalty_energy)
                        .sum::<u64>(),
                    cost
                );
            }
            assert_eq!(
                settlement
                    .method::<_, U256>("penaltyEnergyByTrade", trade_id)?
                    .call()
                    .await?,
                U256::from(buyer_cost + seller_cost)
            );
            assert_eq!(
                trades
                    .iter()
                    .find(|trade| trade.trade_uuid == bytes16_to_hex(trade_id))
                    .unwrap()
                    .status,
                if buyer_cost + seller_cost == 0 {
                    TradeStatus::Executed
                } else {
                    TradeStatus::Penalized
                }
            );
            for order_id in [[index + 1; 16], [index + 11; 16]] {
                assert_eq!(
                    registry
                        .method::<_, u8>("getStatus", order_id)?
                        .call()
                        .await?,
                    2
                );
            }
        }
        assert_eq!(
            penalties
                .iter()
                .filter(
                    |event| event.penalized_actor_id == create_encrypted_bytes16_from_string("a")
                )
                .map(|event| event.penalty_energy)
                .sum::<u64>(),
            a_shares.iter().sum::<u64>()
        );
        let block = client.get_block_number().await?;
        run_execution_cycle(
            &app.address,
            &endpoint,
            &address,
            &private_key,
            slot,
            0.1,
            900,
            5.0,
        )
        .await?;
        assert_eq!(client.get_block_number().await?, block);
        assert_eq!(
            settlement
                .event::<gsy_ethers_listener::PenaltyRecordedFilter>()
                .from_block(0u64)
                .query()
                .await?,
            penalties
        );
        assert_eq!(
            settlement
                .event::<gsy_ethers_listener::DeliveryVerifiedFilter>()
                .from_block(0u64)
                .query()
                .await?,
            verifications
        );
        assert_eq!(app.db_wrapper.trades().get_all_trades().await?, trades);
        listener_task.abort();
        stop_app(app).await;
    }
    Ok(())
}
