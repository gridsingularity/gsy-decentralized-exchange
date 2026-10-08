use ethers::prelude::*;
use ethers_solc::{artifacts::Severity, Project, ProjectPathsConfig};
use std::{fs::File, io::Write, sync::Arc};
use tempfile::TempDir;

/// Compiles `source_code` with the local solc and deploys `contract_name` without constructor
/// arguments.
pub async fn compile_and_deploy_contract(
    client: Arc<SignerMiddleware<Provider<Ws>, LocalWallet>>,
    source_code: &str,
    contract_name: &str,
) -> Address {
    let temp_dir = TempDir::new().unwrap();
    let contracts_dir = temp_dir.path().join("contracts");
    std::fs::create_dir(&contracts_dir).unwrap();
    let source_path = contracts_dir.join("MockContract.sol");
    {
        let mut file = File::create(&source_path).unwrap();
        file.write_all(source_code.as_bytes()).unwrap();
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
        .find(|(name, _)| *name == contract_name)
        .map(|(_, artifact)| artifact)
        .expect("Contract artifact not found");

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
    factory.deploy(()).unwrap().send().await.unwrap().address()
}
