use ethers::{prelude::*, utils::Anvil};
use ethers_solc::{artifacts::Severity, Project, ProjectPathsConfig};
use gsy_execution_engine::{
    connectors::evm_connector::submit_penalties, primitives::penalty_calculator::Penalty,
};
use std::{fs::File, io::Write, sync::Arc};
use tempfile::TempDir;

abigen!(
    MockTradeSettlement,
    r#"[
        function penaltyEnergyByTrade(bytes32 tradeId) external view returns (uint256)
        function penaltyEnergyByAccount(address account) external view returns (uint256)
    ]"#
);

#[tokio::test]
async fn test_submit_penalties_persists_to_trade_settlement_contract() {
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
            mapping(bytes32 => uint256) public penaltyEnergyByTrade;
            mapping(address => uint256) public penaltyEnergyByAccount;

            struct TradePenalty {
                address penalizedAccount;
                bytes32 marketId;
                bytes32 tradeId;
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
                    penaltyEnergyByAccount[penalties[i].penalizedAccount] += penalties[i].penaltyEnergy;
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

    let penalized = anvil.addresses()[1];
    let trade_uuid = "trade-uuid-123";
    let penalties = vec![
        Penalty {
            penalized_account: format!("{:?}", penalized),
            market_id: format!("0x{}", "11".repeat(32)),
            trade_uuid: trade_uuid.to_string(),
            penalty_cost: 100,
        },
        Penalty {
            penalized_account: format!("{:?}", penalized),
            market_id: format!("0x{}", "11".repeat(32)),
            trade_uuid: trade_uuid.to_string(),
            penalty_cost: 150,
        },
    ];

    submit_penalties(
        &ws_endpoint,
        &format!("{:?}", contract_address),
        "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80",
        penalties,
    )
    .await
    .unwrap();

    let mock_contract = MockTradeSettlement::new(contract_address, client.clone());
    let expected_trade_id = ethers::utils::keccak256(trade_uuid.as_bytes());

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
            .penalty_energy_by_account(penalized)
            .call()
            .await
            .unwrap(),
        U256::from(250u64)
    );
}
