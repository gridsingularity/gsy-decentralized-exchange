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
    let admin = client.address();
    let controller = deploy(client.clone(), "MarketController", admin).await?;
    let actors = deploy(client.clone(), "ActorRegistry", admin).await?;
    let registry = deploy(
        client.clone(),
        "OrderRegistry",
        (admin, controller.address(), actors.address()),
    )
    .await?;
    let settlement = deploy(
        client.clone(),
        "TradeSettlement",
        (admin, registry.address()),
    )
    .await?;
    for (contract, role, account) in [
        (&controller, "ORCHESTRATOR_ROLE", admin),
        (&registry, "SETTLEMENT_ROLE", settlement.address()),
        (&settlement, "OPERATOR_ROLE", admin),
        (&settlement, "EXECUTION_ENGINE_ROLE", admin),
    ] {
        send(contract, "grantRole", (keccak256(role), account)).await?;
    }

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
    let mut measurements = Vec::new();
    for index in 0..3u8 {
        let buyer = format!("buyer-{index}");
        let seller = format!("seller-{index}");
        let buyer_id = create_encrypted_bytes16_from_string(&buyer);
        let seller_id = create_encrypted_bytes16_from_string(&seller);
        for id in [buyer_id, seller_id] {
            send(&actors, "registerActor", (id, admin)).await?;
        }
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
        let facility = format!("meter-{index}");
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
            energy_kwh: [1.0, 0.8, 1.2][index as usize],
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
            900
        )
        .await?,
        0
    );
    insert_measurements(&app.db_wrapper, &measurements[..2]).await?;
    assert_eq!(
        run_execution_cycle(
            &app.address,
            &endpoint,
            &address,
            &private_key,
            slot,
            0.1,
            900
        )
        .await?,
        0
    );
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            let trades = app.db_wrapper.trades().get_all_trades().await.unwrap();
            if trades
                .iter()
                .filter(|trade| trade.status == TradeStatus::Executed)
                .count()
                == 2
            {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await?;
    assert_eq!(
        registry
            .method::<_, u8>("getStatus", [3u8; 16])?
            .call()
            .await?,
        5
    ); // still partially filled
    insert_measurements(&app.db_wrapper, &measurements[2..]).await?;
    assert_eq!(
        run_execution_cycle(
            &app.address,
            &endpoint,
            &address,
            &private_key,
            slot,
            0.1,
            900
        )
        .await?,
        1
    );
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            let trades = app.db_wrapper.trades().get_all_trades().await.unwrap();
            if trades
                .iter()
                .all(|trade| trade.status == TradeStatus::Executed)
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
        assert_eq!(trade.status_updated_at, Some(timestamp));
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
        app.db_wrapper
            .trades()
            .filter_trades_by_status_change(0, None, Some(TradeStatus::Executed))
            .await?
            .len(),
        3
    );
    listener_task.abort();
    stop_app(app).await;
    Ok(())
}
