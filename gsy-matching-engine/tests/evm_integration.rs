use ethers::{prelude::*, utils::Anvil};
use ethers_solc::{artifacts::Severity, Project, ProjectPathsConfig};
use gsy_matching_engine::connectors::evm_connector::send_settle_batch_transaction;
use gsy_offchain_primitives::db_api_schema::orders::{DbOrderSchema, OrderEnum, OrderStatus};
use gsy_offchain_primitives::types::{BidOfferMatch, Order};
use gsy_offchain_primitives::utils::{
    string_to_account_id, string_to_h256, NODE_FLOAT_SCALING_FACTOR,
};
use std::{collections::HashMap, fs::File, io::Write, sync::Arc};
use tempfile::TempDir;

abigen!(
    MockTradeSettlement,
    r#"[
        function settledCount() external view returns (uint256)
        function lastSelectedEnergy() external view returns (uint256)
        function lastClearingPrice() external view returns (uint256)
        function lastBidOwner() external view returns (address)
        function lastAskOwner() external view returns (address)
    ]"#
);

#[tokio::test]
async fn test_settle_batch_submits_matches_to_trade_settlement_contract() {
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
            bytes32 public constant OPERATOR_ROLE = keccak256("OPERATOR_ROLE");
            mapping(address => mapping(bytes32 => bool)) private roles;

            struct OrderData {
                address owner;
                uint64 nonce;
                bytes32 areaUuid;
                bytes32 marketId;
                uint64 timeSlot;
                uint64 creationTime;
                uint64 energy;
                uint64 energyRate;
            }

            struct Match {
                OrderData bid;
                OrderData ask;
                uint256 selectedEnergy;
                uint256 clearingPrice;
            }

            uint256 public settledCount;
            uint256 public lastSelectedEnergy;
            uint256 public lastClearingPrice;
            address public lastBidOwner;
            address public lastAskOwner;

            constructor() {
                roles[msg.sender][OPERATOR_ROLE] = true;
            }

            function hasRole(bytes32 role, address account) external view returns (bool) {
                return roles[account][role];
            }

            function settleBatch(Match[] calldata matches) external {
                require(roles[msg.sender][OPERATOR_ROLE], "missing operator role");
                settledCount += matches.length;
                if (matches.length > 0) {
                    Match calldata first = matches[0];
                    lastSelectedEnergy = first.selectedEnergy;
                    lastClearingPrice = first.clearingPrice;
                    lastBidOwner = first.bid.owner;
                    lastAskOwner = first.ask.owner;
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

    let bid_owner = anvil.addresses()[1];
    let ask_owner = anvil.addresses()[2];
    let canonical_account_id =
        string_to_account_id("5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY".to_string())
            .unwrap();

    let bid_order_id = format!("0x{}", "11".repeat(32));
    let ask_order_id = format!("0x{}", "22".repeat(32));
    let market_id = format!("0x{}", "33".repeat(32));
    let bid_area = format!("0x{}", "44".repeat(32));
    let ask_area = format!("0x{}", "55".repeat(32));

    let bid_db = DbOrderSchema {
        order_id: bid_order_id.clone(),
        status: OrderStatus::Open,
        order_type: OrderEnum::Bid,
        area_uuid: bid_area.clone(),
        market_id: market_id.clone(),
        nonce: Some(1),
        time_slot: 1000,
        creation_time: 900,
        energy_kWh: 100.0,
        energy_rate: 50.0,
        created_by: format!("{:?}", bid_owner),
        requirements: None,
        attributes: None,
    };
    let ask_db = DbOrderSchema {
        order_id: ask_order_id.clone(),
        status: OrderStatus::Open,
        order_type: OrderEnum::Offer,
        area_uuid: ask_area.clone(),
        market_id: market_id.clone(),
        nonce: Some(2),
        time_slot: 1000,
        creation_time: 900,
        energy_kWh: 80.0,
        energy_rate: 40.0,
        created_by: format!("{:?}", ask_owner),
        requirements: None,
        attributes: None,
    };

    let bid_order = Order {
        order_id: string_to_h256(bid_order_id.clone()),
        order_type: OrderEnum::Bid,
        status: OrderStatus::Open,
        area_uuid: string_to_h256(bid_area),
        market_id: string_to_h256(market_id.clone()),
        time_slot: 1000,
        creation_time: 900,
        energy: (100.0 * NODE_FLOAT_SCALING_FACTOR) as u64,
        energy_rate: (50.0 * NODE_FLOAT_SCALING_FACTOR) as u64,
        created_by: canonical_account_id.clone(),
        requirements: None,
        attributes: None,
    };

    let ask_order = Order {
        order_id: string_to_h256(ask_order_id.clone()),
        order_type: OrderEnum::Offer,
        status: OrderStatus::Open,
        area_uuid: string_to_h256(ask_area),
        market_id: string_to_h256(market_id),
        time_slot: 1000,
        creation_time: 900,
        energy: (80.0 * NODE_FLOAT_SCALING_FACTOR) as u64,
        energy_rate: (40.0 * NODE_FLOAT_SCALING_FACTOR) as u64,
        created_by: canonical_account_id,
        requirements: None,
        attributes: None,
    };

    let selected_energy = (80.0 * NODE_FLOAT_SCALING_FACTOR) as u64;
    let clearing_price = (50.0 * NODE_FLOAT_SCALING_FACTOR) as u64;
    let matches = vec![BidOfferMatch {
        market_id: string_to_h256(format!("0x{}", "33".repeat(32))),
        time_slot: 1000,
        bid: bid_order,
        offer: ask_order,
        residual_bid: None,
        residual_offer: None,
        selected_energy,
        energy_rate: clearing_price,
    }];

    let mut lookup = HashMap::new();
    lookup.insert(bid_order_id, bid_db);
    lookup.insert(ask_order_id, ask_db);

    send_settle_batch_transaction(
        &ws_endpoint,
        &format!("{:?}", contract_address),
        "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80",
        matches,
        lookup,
    )
    .await
    .unwrap();

    let mock_contract = MockTradeSettlement::new(contract_address, client.clone());

    assert_eq!(
        mock_contract.settled_count().call().await.unwrap(),
        U256::from(1u64)
    );
    assert_eq!(
        mock_contract.last_selected_energy().call().await.unwrap(),
        U256::from(selected_energy)
    );
    assert_eq!(
        mock_contract.last_clearing_price().call().await.unwrap(),
        U256::from(clearing_price)
    );
    assert_eq!(
        mock_contract.last_bid_owner().call().await.unwrap(),
        bid_owner
    );
    assert_eq!(
        mock_contract.last_ask_owner().call().await.unwrap(),
        ask_owner
    );
}
