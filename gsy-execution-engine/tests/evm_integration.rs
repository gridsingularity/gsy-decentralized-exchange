use ::primitives::{
    constants::GLOBAL_CONSTANTS,
    db_api_schema::trades::{DbTradeSchema, TradeParameters, TradeStatus},
    ewds::dto::EwdsTradeDto,
    offchain_storage::OffchainStorageTransport,
    utils::{
        bytes16_to_hex, create_encrypted_bytes16_from_string, epoch_to_rfc3339,
        parse_uuid_or_hex_bytes16, timestamp_to_string_with_padding,
    },
};
use chrono::{DateTime, Duration, Utc};
use ethers::{
    prelude::*,
    utils::{Anvil, AnvilInstance},
};
use ethers_solc::{artifacts::Severity, Project, ProjectPathsConfig};
use gsy_execution_engine::{
    connectors::evm_connector::submit_penalties, primitives::penalty_calculator::Penalty,
    services::execution_orchestrator::run_execution_cycle, timeslot_scheduler::TimeslotScheduler,
};
use serde_json::json;
use std::{fs::File, io::Write, sync::Arc};
use tempfile::TempDir;
use uuid::Uuid;
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

const PRIVATE_KEY: &str = "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80";
type TestClient = SignerMiddleware<Provider<Ws>, LocalWallet>;

abigen!(
    MockTradeSettlement,
    r#"[
        function penaltyEnergyByTrade(bytes16 tradeId) external view returns (uint256)
        function penaltyEnergyByActor(bytes16 actorId) external view returns (uint256)
    ]"#
);

async fn deploy_trade_settlement() -> (AnvilInstance, MockTradeSettlement<TestClient>) {
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
    let source_path = contracts_dir.join("MockTradeSettlement.sol");
    let source = r#"
        // SPDX-License-Identifier: MIT
        pragma solidity ^0.8.20;

        contract MockTradeSettlement {
            bytes32 public constant EXECUTION_ENGINE_ROLE = keccak256("EXECUTION_ENGINE_ROLE");
            mapping(address => mapping(bytes32 => bool)) private roles;
            mapping(bytes16 => uint256) public penaltyEnergyByTrade;
            mapping(bytes16 => uint256) public penaltyEnergyByActor;

            struct TradePenalty {
                bytes16 penalizedActorId;
                bytes16 marketId;
                bytes16 tradeId;
                uint64 penaltyEnergy;
            }

            constructor() {
                roles[msg.sender][EXECUTION_ENGINE_ROLE] = true;
            }

            function hasRole(bytes32 role, address account) external view returns (bool) {
                return roles[account][role];
            }

            function submitPenalties(TradePenalty[] calldata penalties) external {
                require(roles[msg.sender][EXECUTION_ENGINE_ROLE], "missing execution engine role");
                for (uint256 i = 0; i < penalties.length; i++) {
                    penaltyEnergyByTrade[penalties[i].tradeId] += penalties[i].penaltyEnergy;
                    penaltyEnergyByActor[penalties[i].penalizedActorId] += penalties[i].penaltyEnergy;
                }
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
        if err.severity == Severity::Error {
            panic!("Solidity compilation error: {}", err.message);
        }
    }

    let contract_list = output
        .contracts
        .values()
        .flat_map(|inner| inner.iter())
        .find(|(name, _)| *name == "MockTradeSettlement")
        .map(|(_, artifact)| artifact)
        .expect("MockTradeSettlement artifact not found");

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

    (anvil, MockTradeSettlement::new(contract_address, client))
}

#[tokio::test]
async fn test_submit_penalties_persists_to_trade_settlement_contract() {
    let (anvil, mock_contract) = deploy_trade_settlement().await;
    let penalized = format!("0x{}", "aa".repeat(16));
    let trade_uuid = Uuid::new_v4().to_string();
    let penalties = vec![
        Penalty {
            penalized_account: penalized.clone(),
            market_id: format!("0x{}", "11".repeat(16)),
            trade_uuid: trade_uuid.clone(),
            penalty_cost: 100,
        },
        Penalty {
            penalized_account: penalized.clone(),
            market_id: format!("0x{}", "11".repeat(16)),
            trade_uuid: trade_uuid.clone(),
            penalty_cost: 150,
        },
    ];

    submit_penalties(
        &anvil.ws_endpoint(),
        &format!("{:?}", mock_contract.address()),
        PRIVATE_KEY,
        penalties,
    )
    .await
    .unwrap();

    eprintln!("trade_uuid {:?}", trade_uuid);
    eprintln!("penalized {:?}", penalized);
    let expected_trade_id = parse_uuid_or_hex_bytes16(&trade_uuid).expect("Failed to parse uuid");
    let expected_actor_id = parse_uuid_or_hex_bytes16(&penalized).expect("Failed to parse uuid");

    assert_eq!(
        mock_contract
            .penalty_energy_by_trade(expected_trade_id)
            .call()
            .await
            .unwrap(),
        U256::from(250u64)
    );
    assert_eq!(
        mock_contract
            .penalty_energy_by_actor(expected_actor_id)
            .call()
            .await
            .unwrap(),
        U256::from(250u64)
    );
}

fn late_trade(id: u8, market: u8, creation_time: u64) -> DbTradeSchema {
    DbTradeSchema {
        trade_uuid: bytes16_to_hex([id; 16]),
        status: TradeStatus::Settled,
        seller: bytes16_to_hex(create_encrypted_bytes16_from_string("bob")),
        buyer: bytes16_to_hex(create_encrypted_bytes16_from_string("alice")),
        market_id: bytes16_to_hex([market; 16]),
        creation_time,
        offer_hash: bytes16_to_hex([id + 10; 16]),
        bid_hash: bytes16_to_hex([id + 20; 16]),
        residual_offer_id: None,
        residual_bid_id: None,
        parameters: TradeParameters {
            selected_energy_kWh: 10.0,
            energy_rate: 1.0,
        },
    }
}

async fn mount_slot_records(
    server: &MockServer,
    slot: u64,
    trades: &[DbTradeSchema],
    measured_energy: Option<f64>,
    fail_query: bool,
) {
    let end = slot + GLOBAL_CONSTANTS.time_slot_sec - 1;
    let response = if fail_query {
        ResponseTemplate::new(500)
    } else {
        ResponseTemplate::new(200).set_body_json(
            trades
                .iter()
                .cloned()
                .map(EwdsTradeDto::from)
                .collect::<Vec<_>>(),
        )
    };
    Mock::given(method("GET"))
        .and(path("/trades"))
        .and(query_param("start_time", epoch_to_rfc3339(slot)))
        .and(query_param("end_time", epoch_to_rfc3339(end)))
        .respond_with(response)
        .mount(server)
        .await;
    let timeseries: Vec<_> = measured_energy
        .map(|value| {
            json!({
                "measurement_point": "measurement-alice",
                "timestamp": timestamp_to_string_with_padding(slot),
                "value": value,
            })
        })
        .into_iter()
        .collect();
    Mock::given(method("GET"))
        .and(path("/timeseries"))
        .and(query_param(
            "start_time",
            timestamp_to_string_with_padding(slot),
        ))
        .and(query_param(
            "end_time",
            timestamp_to_string_with_padding(end),
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(timeseries))
        .mount(server)
        .await;
}

#[tokio::test]
async fn late_records_after_rollover_are_processed_once_without_blocking_the_current_slot() {
    assert_eq!(
        OffchainStorageTransport::from_env(),
        OffchainStorageTransport::Http,
        "Run this HTTP regression test with OFFCHAIN_STORAGE_TRANSPORT=http"
    );
    let (anvil, contract) = deploy_trade_settlement().await;
    let client = contract.client();
    let initial_nonce = client
        .get_transaction_count(client.address(), None)
        .await
        .unwrap();
    let server = MockServer::start().await;
    let slot_duration = GLOBAL_CONSTANTS.time_slot_sec;
    let current = (1_800_000_000 / slot_duration) * slot_duration;
    let retained = current - slot_duration;
    let rollover = DateTime::<Utc>::from_timestamp(current as i64, 0).unwrap()
        + Duration::minutes(GLOBAL_CONSTANTS.execution_engine_offset_min);
    let grace_seconds = slot_duration / 2;
    let mut scheduler = TimeslotScheduler::new(grace_seconds);
    assert_eq!(
        scheduler.calculate_timeslot_at(rollover - Duration::seconds(1)),
        retained
    );
    let actor_id = create_encrypted_bytes16_from_string("alice");
    let current_trade = late_trade(3, 11, current);
    // The retained-slot trades are created after rollover, not inside their delivery window.
    let retained_trades = [
        late_trade(1, 10, current + 3),
        late_trade(2, 10, current + 5),
    ];

    // No wall-clock sleeps: control availability and the scheduler's clock independently.
    for (elapsed, trade_count, has_measurement, fail_query) in [
        (0, 0, false, false),
        (1, 0, false, true),
        (2, 0, false, false),
        (3, 1, false, false),
        (4, 1, true, false),
        (5, 2, true, false),
        (6, 2, true, false),
    ] {
        if elapsed == 6 {
            scheduler = TimeslotScheduler::new(grace_seconds);
        }
        server.reset().await;
        Mock::given(method("GET"))
            .and(path("/measurement-points"))
            .and(query_param("type", "Measurement"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!([{
                "type": "Measurement",
                "measurement_id": "measurement-alice",
                "property_measured": "energy_measured",
                "unit": "kWh",
                "direction": "Import",
                "energy_accumulated": false,
                "time_resolution": "PT15M",
                "phase": 0,
                "asset_name": "facility-alice",
                "datasource_name": "community-1",
            }])))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/facilities"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!([{
                "facility_id": "facility-alice",
                "facility_name": "Alice",
                "site_id": "site-1",
                "owner_id": "alice",
            }])))
            .mount(&server)
            .await;
        mount_slot_records(
            &server,
            current,
            std::slice::from_ref(&current_trade),
            Some(15.0),
            false,
        )
        .await;
        mount_slot_records(
            &server,
            retained,
            &retained_trades[..trade_count],
            has_measurement.then_some(12.0),
            fail_query,
        )
        .await;

        for expected_slot in [current, retained] {
            let timeslot = scheduler.calculate_timeslot_at(rollover + Duration::seconds(elapsed));
            assert_eq!(timeslot, expected_slot);
            let result = run_execution_cycle(
                &server.uri(),
                &anvil.ws_endpoint(),
                &format!("{:?}", contract.address()),
                PRIVATE_KEY,
                timeslot,
                0.10,
                slot_duration,
            )
            .await;
            if timeslot == retained && fail_query {
                assert!(result.unwrap_err().to_string().contains("HTTP 500"));
            } else {
                let expected_count = if timeslot == current {
                    1
                } else if has_measurement {
                    trade_count
                } else {
                    0
                };
                // The result counts existing on-chain penalties as well as newly submitted ones.
                assert_eq!(result.unwrap(), expected_count);
            }
            assert_eq!(
                contract
                    .penalty_energy_by_trade([3; 16])
                    .call()
                    .await
                    .unwrap(),
                U256::from(5_000),
                "Current-slot penalty must not wait for retained-slot records"
            );
        }

        let first_penalty = if elapsed >= 4 { 2_000u64 } else { 0 };
        let second_penalty = if elapsed >= 5 { 2_000u64 } else { 0 };
        for (id, expected) in [(1, first_penalty), (2, second_penalty)] {
            assert_eq!(
                contract
                    .penalty_energy_by_trade([id; 16])
                    .call()
                    .await
                    .unwrap(),
                U256::from(expected)
            );
        }
        assert_eq!(
            contract
                .penalty_energy_by_actor(actor_id)
                .call()
                .await
                .unwrap(),
            U256::from(5_000 + first_penalty + second_penalty),
            "Repeated polls and restart must not charge an actor twice"
        );
        let expected_transactions = 1u64 + u64::from(elapsed >= 4) + u64::from(elapsed >= 5);
        assert_eq!(
            client
                .get_transaction_count(client.address(), None)
                .await
                .unwrap(),
            initial_nonce + U256::from(expected_transactions),
            "Already-recorded penalties must not generate another transaction"
        );
    }

    // The deadline applies even though records remain queryable in the old slot.
    let deadline = rollover + Duration::seconds(grace_seconds as i64);
    for _ in 0..4 {
        assert_eq!(scheduler.calculate_timeslot_at(deadline), current);
    }
}
