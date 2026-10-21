use ethers::{prelude::*, utils::Anvil};
use ethers_solc::{artifacts::Severity, Project, ProjectPathsConfig};
use gsy_matching_engine::connectors::evm_connector::{
    send_settle_batch_transaction, ClearingResult, MarketMatches,
};
use gsy_matching_engine::models::{BidOfferMatch, MatchType, Order};
use primitives::db_api_schema::orders::{
    DbAttributes, DbOrderSchema, DbRequirements, EnergyType, OrderEnum, OrderStatus,
};
use primitives::db_api_schema::trades::ClearingStatus;
use primitives::ewds::dto::EwdsTradeDto;
use primitives::offchain_storage::{OffchainStorageClient, OffchainStorageTransport};
use primitives::utils::{bytes16_to_hex, parse_uuid_or_hex_bytes16, NODE_FLOAT_SCALING_FACTOR};
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    fs::File,
    io::Write,
    sync::{Arc, Mutex},
};
use tempfile::TempDir;
use wiremock::http::Method;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, Request, ResponseTemplate};

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
        function lastBidPreferredTradingPartner() external view returns (bytes16)
        function lastBidPreferredEnergyRate() external view returns (uint64)
        function matchTypes(uint256 index) external view returns (uint8)
        function tradeIds(uint256 index) external view returns (bytes16)
        function setRejectSettlement(bool value) external
    ]"#
);

#[tokio::test]
async fn test_settle_batch_registers_matches_before_submitting_to_contract() {
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
                bool isBid;
                bytes16 preferredTradingPartner;
                uint64 preferredEnergyRate;
            }

            enum MatchType { Standard, Preferred }

            struct Match {
                bytes16 tradeId;
                OrderData bid;
                OrderData offer;
                bytes16 residualBidId;
                bytes16 residualOfferId;
                uint256 selectedEnergy;
                uint256 clearingPrice;
                MatchType matchType;
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
            bytes16 public lastBidPreferredTradingPartner;
            uint64 public lastBidPreferredEnergyRate;
            MatchType[] public matchTypes;
            bytes16[] public tradeIds;
            bool private rejectSettlement;

            function setRejectSettlement(bool value) external {
                rejectSettlement = value;
            }

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
                require(!rejectSettlement, "test settlement failure");
                for (uint256 m = 0; m < settlements.length; m++) {
                    MarketSettlement calldata settlement = settlements[m];
                    require(
                        _sumSelectedEnergy(settlement.matches) == settlement.clearingResult.tradedQuantity,
                        "traded quantity mismatch"
                    );
                    settledCount += settlement.matches.length;
                    for (uint256 i = 0; i < settlement.matches.length; i++) {
                        matchTypes.push(settlement.matches[i].matchType);
                        tradeIds.push(settlement.matches[i].tradeId);
                    }
                    if (settlement.matches.length > 0) {
                        Match calldata first = settlement.matches[0];
                        require(first.bid.isBid, "bid must have isBid=true");
                        require(!first.offer.isBid, "offer must have isBid=false");
                        lastSelectedEnergy = first.selectedEnergy;
                        lastClearingPrice = first.clearingPrice;
                        lastBidCreatedBy = first.bid.createdBy;
                        lastOfferCreatedBy = first.offer.createdBy;
                        lastBidPreferredTradingPartner = first.bid.preferredTradingPartner;
                        lastBidPreferredEnergyRate = first.bid.preferredEnergyRate;

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
        status_updated_at: None,
        order_type: OrderEnum::Bid,
        area_uuid: bid_area.clone(),
        market_id: market_id.clone(),
        time_slot: 1000,
        creation_time: 900,
        energy_kWh: 100.0,
        energy_rate: 50.0,
        created_by: bid_actor_id.clone(),
        requirements: Some(DbRequirements {
            trading_partner_id: Some(ask_actor_id.clone()),
            energy_type: Some(EnergyType::Green),
            preferred_energy_rate: Some(45.0),
        }),
        attributes: None,
    };
    let ask_db = DbOrderSchema {
        order_id: ask_order_id.clone(),
        status: OrderStatus::Submitted,
        status_updated_at: None,
        order_type: OrderEnum::Offer,
        area_uuid: ask_area.clone(),
        market_id: market_id.clone(),
        time_slot: 1000,
        creation_time: 900,
        energy_kWh: 80.0,
        energy_rate: 40.0,
        created_by: ask_actor_id.clone(),
        requirements: None,
        attributes: Some(DbAttributes {
            energy_type: EnergyType::Pv,
        }),
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
    let residual_bid = Order {
        order_id: uuid::Uuid::new_v4().to_string(),
        energy: bid_order.energy - selected_energy,
        ..bid_order.clone()
    };
    let second_offer_id = format!("0x{}", "44".repeat(16));
    let second_offer_db = DbOrderSchema {
        order_id: second_offer_id.clone(),
        energy_kWh: 20.0,
        ..ask_db.clone()
    };
    let second_offer = Order {
        order_id: second_offer_id.clone(),
        energy: residual_bid.energy,
        ..ask_order.clone()
    };
    let second_match = BidOfferMatch {
        match_type: MatchType::Standard,
        market_id: market_id.clone(),
        time_slot: 1000,
        selected_energy: residual_bid.energy,
        bid: residual_bid.clone(),
        offer: second_offer,
        residual_bid: None,
        residual_offer: None,
        energy_rate: clearing_price,
    };
    let matches = vec![
        BidOfferMatch {
            match_type: MatchType::Preferred,
            market_id: market_id.clone(),
            time_slot: 1000,
            bid: bid_order,
            offer: ask_order,
            residual_bid: Some(residual_bid),
            residual_offer: None,
            selected_energy,
            energy_rate: clearing_price,
        },
        second_match,
    ];

    let market_matches = || {
        vec![MarketMatches {
            bid_offer_matches: matches.clone(),
            clearing_result: ClearingResult {
                market_id: Some(market_id.clone()),
                clearing_status: ClearingStatus::Final,
                clearing_price: Some(clearing_price),
                total_supply: Some((100.0 * NODE_FLOAT_SCALING_FACTOR) as u64),
                total_demand: Some((100.0 * NODE_FLOAT_SCALING_FACTOR) as u64),
                traded_quantity: Some((100.0 * NODE_FLOAT_SCALING_FACTOR) as u64),
                num_trades: Some(2),
            },
        }]
    };

    let mut lookup = HashMap::new();
    lookup.insert(bid_order_id, bid_db);
    lookup.insert(ask_order_id, ask_db);
    lookup.insert(second_offer_id, second_offer_db);

    let mock_contract = MockTradeSettlement::new(contract_address, client.clone());
    let private_key = "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80";
    let storage_server = MockServer::start().await;
    let storage = OffchainStorageClient::new(
        OffchainStorageTransport::Http,
        storage_server.uri(),
        "UNUSED_ENV",
        "matchingtest",
    );
    Mock::given(method("POST"))
        .and(path("/trades"))
        .respond_with(ResponseTemplate::new(500))
        .expect(1)
        .mount(&storage_server)
        .await;
    let nonce = client
        .get_transaction_count(client.address(), None)
        .await
        .unwrap();
    let error = send_settle_batch_transaction(
        &ws_endpoint,
        &format!("{:?}", contract_address),
        private_key,
        market_matches(),
        lookup.clone(),
        &storage,
    )
    .await
    .unwrap_err();
    assert!(error.to_string().contains("settlement was not submitted"));
    assert_eq!(
        client
            .get_transaction_count(client.address(), None)
            .await
            .unwrap(),
        nonce
    );
    assert_eq!(
        mock_contract.settled_count().call().await.unwrap(),
        U256::zero()
    );
    storage_server.verify().await;
    storage_server.reset().await;

    // Even a contract rejection leaves the already registered records Matched.
    Mock::given(method("POST"))
        .and(path("/trades"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"0": "db-id"})))
        .expect(1)
        .mount(&storage_server)
        .await;
    mock_contract
        .set_reject_settlement(true)
        .send()
        .await
        .unwrap()
        .await
        .unwrap();
    let error = send_settle_batch_transaction(
        &ws_endpoint,
        &format!("{:?}", contract_address),
        private_key,
        market_matches(),
        lookup.clone(),
        &storage,
    )
    .await
    .unwrap_err();
    assert!(error
        .to_string()
        .contains("registered trades remain Matched"));
    let requests = storage_server.received_requests().await.unwrap();
    assert_eq!(
        requests.len(),
        1,
        "no status update or deletion on settlement failure"
    );
    let registered: Vec<EwdsTradeDto> = serde_json::from_slice(&requests[0].body).unwrap();
    assert_eq!(registered.len(), 2);
    assert!(registered
        .iter()
        .all(|trade| trade.trade_status == "matched"));
    assert_eq!(
        mock_contract.settled_count().call().await.unwrap(),
        U256::zero()
    );
    mock_contract
        .set_reject_settlement(false)
        .send()
        .await
        .unwrap()
        .await
        .unwrap();
    storage_server.verify().await;
    storage_server.reset().await;

    Mock::given(method("POST"))
        .and(path("/trades"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"0": "db-id"})))
        .expect(1)
        .mount(&storage_server)
        .await;
    send_settle_batch_transaction(
        &ws_endpoint,
        &format!("{:?}", contract_address),
        private_key,
        market_matches(),
        lookup.clone(),
        &storage,
    )
    .await
    .unwrap();

    let requests = storage_server.received_requests().await.unwrap();
    assert_eq!(
        requests.len(),
        1,
        "successful receipt must not update the stored status"
    );
    let registered: Vec<EwdsTradeDto> = serde_json::from_slice(&requests[0].body).unwrap();
    assert_eq!(registered.len(), 2);
    for (index, trade) in registered.iter().enumerate() {
        assert_eq!(
            trade.trade_id,
            bytes16_to_hex(
                mock_contract
                    .trade_ids(U256::from(index))
                    .call()
                    .await
                    .unwrap()
            )
        );
        assert_eq!(trade.trade_status, "matched");
        assert_eq!(trade.buyer_id, bid_actor_id);
        assert_eq!(trade.seller_id, ask_actor_id);
        assert_eq!(trade.market_id, market_id);
        assert_eq!(trade.trade_price, 50.0);
    }
    assert_eq!(
        registered[0].residual_bid_id.as_ref(),
        Some(&registered[1].bid_id)
    );
    assert_eq!(registered[0].residual_offer_id, None);
    assert_eq!(registered[1].residual_bid_id, None);
    assert_eq!(registered[0].trade_quantity, 80.0);
    assert_eq!(registered[1].trade_quantity, 20.0);
    assert_eq!(
        mock_contract
            .match_types(U256::zero())
            .call()
            .await
            .unwrap(),
        1
    );
    assert_eq!(
        mock_contract.match_types(U256::one()).call().await.unwrap(),
        0
    );

    assert_eq!(
        mock_contract.settled_count().call().await.unwrap(),
        U256::from(2u64)
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
        parse_uuid_or_hex_bytes16(&bid_actor_id).expect("Failed to parse uuid")
    );
    assert_eq!(
        mock_contract.last_offer_created_by().call().await.unwrap(),
        parse_uuid_or_hex_bytes16(&ask_actor_id).expect("Failed to parse uuid")
    );
    assert_eq!(
        mock_contract.last_traded_quantity().call().await.unwrap(),
        U256::from((100.0 * NODE_FLOAT_SCALING_FACTOR) as u64)
    );
    assert_eq!(
        mock_contract.last_market_id().call().await.unwrap(),
        parse_uuid_or_hex_bytes16(&market_id).expect("Failed to parse market id")
    );
    assert_eq!(
        mock_contract
            .last_bid_preferred_trading_partner()
            .call()
            .await
            .unwrap(),
        parse_uuid_or_hex_bytes16(&ask_actor_id).expect("Invalid on-chain actor ID")
    );
    assert_eq!(
        mock_contract
            .last_bid_preferred_energy_rate()
            .call()
            .await
            .unwrap(),
        (45.0 * NODE_FLOAT_SCALING_FACTOR) as u64
    );

    // EWDS registers the same payload without posting to the HTTP trade endpoint.
    let gateway = MockServer::start().await;
    let response = Arc::new(Mutex::new(Value::Null));
    let pending_response = response.clone();
    Mock::given(method("POST"))
        .and(path("/api/v2/messages"))
        .respond_with(move |request: &Request| {
            let message: Value = serde_json::from_slice(&request.body).unwrap();
            let envelope: Value =
                serde_json::from_str(message["payload"].as_str().unwrap()).unwrap();
            assert_eq!(envelope["operation"], "trades.import");
            let ids: Vec<_> = envelope["payload"]
                .as_array()
                .unwrap()
                .iter()
                .map(|trade| trade["tradeId"].clone())
                .collect();
            *pending_response.lock().unwrap() = json!({
                "requestId": envelope["requestId"], "success": true, "data": ids,
            });
            ResponseTemplate::new(200).set_body_json(json!({
                "recipients": {"sent": 1, "failed": 0, "total": 1}
            }))
        })
        .expect(1)
        .mount(&gateway)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/v2/messages"))
        .respond_with(move |_: &Request| {
            ResponseTemplate::new(200).set_body_json(json!([
                {"payload": response.lock().unwrap().to_string()}
            ]))
        })
        .mount(&gateway)
        .await;
    std::env::set_var("EWDS_GATEWAY_URL", gateway.uri());
    std::env::set_var("EWDS_RESPONSE_TIMEOUT_MS", "1000");
    let ewds_storage = OffchainStorageClient::new(
        OffchainStorageTransport::Ewds,
        "http://unused.invalid",
        "UNUSED_ENV",
        "matchingtest",
    );
    let result = send_settle_batch_transaction(
        &ws_endpoint,
        &format!("{:?}", contract_address),
        private_key,
        market_matches(),
        lookup,
        &ewds_storage,
    )
    .await;
    std::env::remove_var("EWDS_GATEWAY_URL");
    std::env::remove_var("EWDS_RESPONSE_TIMEOUT_MS");
    result.unwrap();
    let requests = gateway.received_requests().await.unwrap();
    let sent: Vec<_> = requests
        .iter()
        .filter(|request| request.method == Method::POST)
        .collect();
    assert_eq!(sent.len(), 1);
    let message: Value = serde_json::from_slice(&sent[0].body).unwrap();
    let envelope: Value = serde_json::from_str(message["payload"].as_str().unwrap()).unwrap();
    let registered: Vec<EwdsTradeDto> =
        serde_json::from_value(envelope["payload"].clone()).unwrap();
    assert_eq!(registered.len(), 2);
    for (index, trade) in registered.iter().enumerate() {
        assert_eq!(
            trade.trade_id,
            bytes16_to_hex(
                mock_contract
                    .trade_ids(U256::from(index + 2))
                    .call()
                    .await
                    .unwrap()
            )
        );
        assert_eq!(trade.trade_status, "matched");
    }
    assert_eq!(
        registered[0].residual_bid_id.as_ref(),
        Some(&registered[1].bid_id)
    );

    // Rejected clearing results still reach the chain without a trade import.
    storage_server.reset().await;
    send_settle_batch_transaction(
        &ws_endpoint,
        &format!("{:?}", contract_address),
        private_key,
        vec![MarketMatches {
            bid_offer_matches: vec![],
            clearing_result: ClearingResult {
                market_id: Some(market_id),
                clearing_status: ClearingStatus::Rejected,
                ..Default::default()
            },
        }],
        HashMap::new(),
        &storage,
    )
    .await
    .unwrap();
    assert!(storage_server.received_requests().await.unwrap().is_empty());
    assert_eq!(
        mock_contract.settled_count().call().await.unwrap(),
        U256::from(4)
    );
    assert_eq!(
        mock_contract.last_traded_quantity().call().await.unwrap(),
        U256::zero()
    );
}
