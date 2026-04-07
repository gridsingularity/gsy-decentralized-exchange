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

  const gsyVaultFactory = await ethers.getContractFactory("GsyVault");
  const gsyVault = await gsyVaultFactory.deploy();
  await gsyVault.waitForDeployment();
  const gsyVaultAddress = await gsyVault.getAddress();

  const marketControllerFactory =
    await ethers.getContractFactory("MarketController");
  const marketController = await marketControllerFactory.deploy();
  await marketController.waitForDeployment();
  const marketControllerAddress = await marketController.getAddress();

  const orderRegistryFactory = await ethers.getContractFactory("OrderRegistry");
  const orderRegistry = await orderRegistryFactory.deploy(
    marketControllerAddress,
    gsyVaultAddress,
  );
  await orderRegistry.waitForDeployment();
  const orderRegistryAddress = await orderRegistry.getAddress();

  const tradeSettlementFactory =
    await ethers.getContractFactory("TradeSettlement");
  const tradeSettlement = await tradeSettlementFactory.deploy(
    orderRegistryAddress,
    gsyVaultAddress,
  );
  await tradeSettlement.waitForDeployment();
  const tradeSettlementAddress = await tradeSettlement.getAddress();

  const ORCHESTRATOR_ROLE = ethers.id("ORCHESTRATOR_ROLE");
  const SETTLEMENT_ROLE = ethers.id("SETTLEMENT_ROLE");
  const OPERATOR_ROLE = ethers.id("OPERATOR_ROLE");
  const EXECUTION_ENGINE_ROLE = ethers.id("EXECUTION_ENGINE_ROLE");

  await (
    await marketController.grantRole(ORCHESTRATOR_ROLE, orchestratorAddress)
  ).wait();
  await (
    await orderRegistry.grantRole(SETTLEMENT_ROLE, tradeSettlementAddress)
  ).wait();
  await (await gsyVault.grantRole(SETTLEMENT_ROLE, tradeSettlementAddress)).wait();
  await (
    await tradeSettlement.grantRole(OPERATOR_ROLE, matchingEngineAddress)
  ).wait();
  await (
    await tradeSettlement.grantRole(
      EXECUTION_ENGINE_ROLE,
      executionEngineAddress,
    )
  ).wait();

  const envFilePath = process.env.CONTRACTS_ENV_PATH ?? "/contracts/addresses.env";
  const envFileContent = [
    `export GSY_VAULT_ADDRESS=${gsyVaultAddress}`,
    `export MARKET_CONTROLLER_ADDRESS=${marketControllerAddress}`,
    `export CONTRACT_MARKET_CONTROLLER=${marketControllerAddress}`,
    `export ORDER_REGISTRY_ADDRESS=${orderRegistryAddress}`,
    `export CONTRACT_ORDER_REGISTRY=${orderRegistryAddress}`,
    `export TRADE_SETTLEMENT_ADDRESS=${tradeSettlementAddress}`,
    `export CONTRACT_TRADE_SETTLEMENT=${tradeSettlementAddress}`,
    "",
  ].join("\n");

  mkdirSync(dirname(envFilePath), { recursive: true });
  writeFileSync(envFilePath, envFileContent);

  console.log("Contracts deployed and roles granted:");
  console.log(`  deployer               ${deployerAddress}`);
  console.log(`  gsyVault               ${gsyVaultAddress}`);
  console.log(`  marketController       ${marketControllerAddress}`);
  console.log(`  orderRegistry          ${orderRegistryAddress}`);
  console.log(`  tradeSettlement        ${tradeSettlementAddress}`);
  console.log(`  orchestratorRole       ${orchestratorAddress}`);
  console.log(`  operatorRole           ${matchingEngineAddress}`);
  console.log(`  executionEngineRole    ${executionEngineAddress}`);
  console.log(`  envFile                ${envFilePath}`);
}

main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
