use ethers::{prelude::*, utils::Anvil};
use ethers_solc::{artifacts::Severity, Project, ProjectPathsConfig};
use gsy_matching_engine::connectors::evm_connector::{
    send_settle_batch_transaction, MarketMatches,
};
use gsy_matching_engine::connectors::evm_connector::ClearingResult;
use primitives::db_api_schema::trades::ClearingStatus;
use gsy_matching_engine::models::{BidOfferMatch, Order};
use primitives::db_api_schema::orders::{DbOrderSchema, OrderEnum, OrderStatus};
use primitives::utils::{parse_or_hash_bytes16, NODE_FLOAT_SCALING_FACTOR};
use std::{collections::HashMap, fs::File, io::Write, sync::Arc};
use tempfile::TempDir;

abigen!(
    MockTradeSettlement,
    r#"[
        function settledCount() external view returns (uint256)
        function lastSelectedEnergy() external view returns (uint256)
        function lastClearingPrice() external view returns (uint256)
        function lastBidCreatedBy() external view returns (bytes16)
        function lastOfferCreatedBy() external view returns (bytes16)
        function lastTradedQuantity() external view returns (uint256)
        function lastMarketId() external view returns (bytes16)
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
                bytes16 orderId;
                bytes16 createdBy;
                bytes16 marketId;
                uint64 timeSlot;
                uint64 creationTime;
                uint64 energy;
                uint64 energyRate;
                uint8 energySourcePreference;
                uint8 energyType;
            }

            struct Match {
                bytes16 tradeId;
                OrderData bid;
                OrderData offer;
                bytes16 residualBidId;
                bytes16 residualOfferId;
                uint256 selectedEnergy;
                uint256 clearingPrice;
            }

            struct ClearingResult {
                bytes16 marketId;
                uint8 clearingStatus;
                uint256 clearingPrice;
                uint256 totalSupply;
                uint256 totalDemand;
                uint256 tradedQuantity;
                uint32 numTrades;
            }

            struct MarketSettlement {
                Match[] matches;
                ClearingResult clearingResult;
            }

            uint256 public settledCount;
            uint256 public lastSelectedEnergy;
            uint256 public lastClearingPrice;
            bytes16 public lastBidCreatedBy;
            bytes16 public lastOfferCreatedBy;
            uint256 public lastTradedQuantity;
            bytes16 public lastMarketId;

            constructor() {
                roles[msg.sender][OPERATOR_ROLE] = true;
            }

            function hasRole(bytes32 role, address account) external view returns (bool) {
                return roles[account][role];
            }

            function _sumSelectedEnergy(Match[] calldata matches) internal pure returns (uint256 total) {
                for (uint256 i = 0; i < matches.length; i++) {
                    total += matches[i].selectedEnergy;
                }
            }

            function settleBatch(MarketSettlement[] calldata settlements) external {
                require(roles[msg.sender][OPERATOR_ROLE], "missing operator role");
                for (uint256 m = 0; m < settlements.length; m++) {
                    MarketSettlement calldata settlement = settlements[m];
                    require(
                        _sumSelectedEnergy(settlement.matches) == settlement.clearingResult.tradedQuantity,
                        "traded quantity mismatch"
                    );
                    settledCount += settlement.matches.length;
                    if (settlement.matches.length > 0) {
                        Match calldata first = settlement.matches[0];
                        lastSelectedEnergy = first.selectedEnergy;
                        lastClearingPrice = first.clearingPrice;
                        lastBidCreatedBy = first.bid.createdBy;
                        lastOfferCreatedBy = first.offer.createdBy;
                    }
                    lastTradedQuantity = settlement.clearingResult.tradedQuantity;
                    lastMarketId = settlement.clearingResult.marketId;
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

    let bid_actor_id = format!("0x{}", "aa".repeat(16));
    let ask_actor_id = format!("0x{}", "bb".repeat(16));
    let bid_order_id = format!("0x{}", "11".repeat(16));
    let ask_order_id = format!("0x{}", "22".repeat(16));
    let market_id = format!("0x{}", "33".repeat(16));
    let bid_area = bid_actor_id.clone();
    let ask_area = ask_actor_id.clone();

    let bid_db = DbOrderSchema {
        order_id: bid_order_id.clone(),
        status: OrderStatus::Submitted,
        order_type: OrderEnum::Bid,
        area_uuid: bid_area.clone(),
        market_id: market_id.clone(),
        time_slot: 1000,
        creation_time: 900,
        energy_kWh: 100.0,
        energy_rate: 50.0,
        created_by: bid_actor_id.clone(),
        requirements: None,
        attributes: None,
    };
    let ask_db = DbOrderSchema {
        order_id: ask_order_id.clone(),
        status: OrderStatus::Submitted,
        order_type: OrderEnum::Offer,
        area_uuid: ask_area.clone(),
        market_id: market_id.clone(),
        time_slot: 1000,
        creation_time: 900,
        energy_kWh: 80.0,
        energy_rate: 40.0,
        created_by: ask_actor_id.clone(),
        requirements: None,
        attributes: None,
    };

    let bid_order = Order {
        order_id: bid_order_id.clone(),
        order_type: OrderEnum::Bid,
        status: OrderStatus::Submitted,
        area_uuid: bid_area.clone(),
        market_id: market_id.clone(),
        time_slot: 1000,
        creation_time: 900,
        energy: (100.0 * NODE_FLOAT_SCALING_FACTOR) as u64,
        energy_rate: (50.0 * NODE_FLOAT_SCALING_FACTOR) as u64,
        created_by: bid_actor_id.clone(),
        requirements: None,
        attributes: None,
    };

    let ask_order = Order {
        order_id: ask_order_id.clone(),
        order_type: OrderEnum::Offer,
        status: OrderStatus::Submitted,
        area_uuid: ask_area.clone(),
        market_id: market_id.clone(),
        time_slot: 1000,
        creation_time: 900,
        energy: (80.0 * NODE_FLOAT_SCALING_FACTOR) as u64,
        energy_rate: (40.0 * NODE_FLOAT_SCALING_FACTOR) as u64,
        created_by: ask_actor_id.clone(),
        requirements: None,
        attributes: None,
    };

    let selected_energy = (80.0 * NODE_FLOAT_SCALING_FACTOR) as u64;
    let clearing_price = (50.0 * NODE_FLOAT_SCALING_FACTOR) as u64;

    let bid_offer_matches = vec![BidOfferMatch {
        market_id: market_id.clone(),
        time_slot: 1000,
        bid: bid_order,
        offer: ask_order,
        residual_bid: None,
        residual_offer: None,
        selected_energy,
        energy_rate: clearing_price,
    }];

    let clearing_result = ClearingResult {
        market_id: Some(market_id.clone()),
        clearing_status: ClearingStatus::Final,
        clearing_price: Some(clearing_price),
        total_supply: Some((80.0 * NODE_FLOAT_SCALING_FACTOR) as u64),
        total_demand: Some((100.0 * NODE_FLOAT_SCALING_FACTOR) as u64),
        traded_quantity: Some(selected_energy),
        num_trades: Some(1),
    };

    let market_matches = vec![MarketMatches { bid_offer_matches, clearing_result }];

    let mut lookup = HashMap::new();
    lookup.insert(bid_order_id, bid_db);
    lookup.insert(ask_order_id, ask_db);

    send_settle_batch_transaction(
        &ws_endpoint,
        &format!("{:?}", contract_address),
        "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80",
        market_matches,
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
        mock_contract.last_bid_created_by().call().await.unwrap(),
        parse_or_hash_bytes16(&bid_actor_id)
    );
    assert_eq!(
        mock_contract.last_offer_created_by().call().await.unwrap(),
        parse_or_hash_bytes16(&ask_actor_id)
    );
    assert_eq!(
        mock_contract.last_traded_quantity().call().await.unwrap(),
        U256::from(selected_energy)
    );
    assert_eq!(
        mock_contract.last_market_id().call().await.unwrap(),
        parse_or_hash_bytes16(&market_id)
    );
}