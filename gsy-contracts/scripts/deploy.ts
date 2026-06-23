import { ethers } from "hardhat";
import { mkdirSync, writeFileSync } from "fs";
import { dirname } from "path";

const RPC_RETRY_ATTEMPTS = 60;
const RPC_RETRY_DELAY_MS = 1000;

function getAddressFromPrivateKey(
  privateKey: string | undefined,
  fallback: string,
): string {
  if (!privateKey || privateKey.trim().length === 0) {
    return fallback;
  }
  return new ethers.Wallet(privateKey).address;
}

async function waitForRpcAndGetDeployer() {
  let lastError: unknown;

  for (let attempt = 1; attempt <= RPC_RETRY_ATTEMPTS; attempt++) {
    try {
      const [deployer] = await ethers.getSigners();
      if (attempt > 1) {
        console.log(`Connected to RPC on attempt ${attempt}`);
      }
      return deployer;
    } catch (error) {
      lastError = error;
      console.log(
        `Waiting for RPC (${attempt}/${RPC_RETRY_ATTEMPTS}): ${String(error)}`,
      );
      await new Promise((resolve) => setTimeout(resolve, RPC_RETRY_DELAY_MS));
    }
  }

  throw new Error(
    `Could not connect to RPC after ${RPC_RETRY_ATTEMPTS} attempts. Last error: ${String(lastError)}`,
  );
}

async function deployContract(contractName: string, args: any[] = []): Promise<
  readonly [any, string]
> {
  const factory = await ethers.getContractFactory(contractName);
  const contract = await factory.deploy(...args);
  await contract.waitForDeployment();
  const address = await contract.getAddress();

  return [contract, address] as const;
}

async function main() {
  const deployer = await waitForRpcAndGetDeployer();
  const deployerAddress = await deployer.getAddress();

  const orchestratorAddress = getAddressFromPrivateKey(
    process.env.ORCHESTRATOR_SIGNER_PRIVATE_KEY,
    deployerAddress,
  );
  const matchingEngineAddress = getAddressFromPrivateKey(
    process.env.MATCHING_ENGINE_PRIVATE_KEY,
    deployerAddress,
  );
  const executionEngineAddress = getAddressFromPrivateKey(
    process.env.EXECUTION_ENGINE_PRIVATE_KEY,
    deployerAddress,
  );
  const actorRegistrarAddress = getAddressFromPrivateKey(
    process.env.ACTOR_REGISTRAR_PRIVATE_KEY,
    deployerAddress,
  );

  const [actorRegistry, actorRegistryAddress] =
    await deployContract("ActorRegistry");
  const [marketController, marketControllerAddress] =
    await deployContract("MarketController");
  const [orderRegistry, orderRegistryAddress] = await deployContract(
    "OrderRegistry",
    [marketControllerAddress, actorRegistryAddress],
  );
  const [tradeSettlement, tradeSettlementAddress] = await deployContract(
    "TradeSettlement",
    [orderRegistryAddress],
  );

  const ORCHESTRATOR_ROLE = ethers.id("ORCHESTRATOR_ROLE");
  const SETTLEMENT_ROLE = ethers.id("SETTLEMENT_ROLE");
  const OPERATOR_ROLE = ethers.id("OPERATOR_ROLE");
  const EXECUTION_ENGINE_ROLE = ethers.id("EXECUTION_ENGINE_ROLE");
  const ACTOR_REGISTRAR_ROLE = ethers.id("ACTOR_REGISTRAR_ROLE");

  await (
    await marketController.grantRole(ORCHESTRATOR_ROLE, orchestratorAddress)
  ).wait();
  await (
    await orderRegistry.grantRole(SETTLEMENT_ROLE, tradeSettlementAddress)
  ).wait();
  await (
    await tradeSettlement.grantRole(OPERATOR_ROLE, matchingEngineAddress)
  ).wait();
  await (
    await tradeSettlement.grantRole(
      EXECUTION_ENGINE_ROLE,
      executionEngineAddress,
    )
  ).wait();
  await (
    await actorRegistry.grantRole(ACTOR_REGISTRAR_ROLE, actorRegistrarAddress)
  ).wait();

  const envFilePath = process.env.CONTRACTS_ENV_PATH ?? "/contracts/addresses.env";
  const envFileContent = [
    `export ACTOR_REGISTRY_ADDRESS=${actorRegistryAddress}`,
    `export MARKET_CONTROLLER_ADDRESS=${marketControllerAddress}`,
    `export CONTRACT_MARKET_CONTROLLER=${marketControllerAddress}`,
    `export ORDER_REGISTRY_ADDRESS=${orderRegistryAddress}`,
    `export CONTRACT_ORDER_REGISTRY=${orderRegistryAddress}`,
    `export TRADE_SETTLEMENT_ADDRESS=${tradeSettlementAddress}`,
    `export CONTRACT_TRADE_SETTLEMENT=${tradeSettlementAddress}`,
    `export ACTOR_REGISTRAR_ADDRESS=${actorRegistrarAddress}`,
    "",
  ].join("\n");

  mkdirSync(dirname(envFilePath), { recursive: true });
  writeFileSync(envFilePath, envFileContent);

  console.log("Contracts deployed and roles granted:");
  console.log(`  deployer               ${deployerAddress}`);
  console.log(`  actorRegistry          ${actorRegistryAddress}`);
  console.log(`  marketController       ${marketControllerAddress}`);
  console.log(`  orderRegistry          ${orderRegistryAddress}`);
  console.log(`  tradeSettlement        ${tradeSettlementAddress}`);
  console.log(`  orchestratorRole       ${orchestratorAddress}`);
  console.log(`  operatorRole           ${matchingEngineAddress}`);
  console.log(`  executionEngineRole    ${executionEngineAddress}`);
  console.log(`  actorRegistrarRole     ${actorRegistrarAddress}`);
  console.log(`  envFile                ${envFilePath}`);
}

main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
