import { ethers } from "hardhat";
import { mkdirSync, writeFileSync } from "fs";
import { dirname } from "path";

const RPC_RETRY_ATTEMPTS = 60;
const RPC_RETRY_DELAY_MS = 1000;
const ERC1967_ADMIN_SLOT =
  "0xb53127684a568b3173ae13b9f8a6016e243e63b6e8ee1178d6a717850b5d6103";

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

async function getProxyAdminAddress(proxyAddress: string): Promise<string> {
  const storageValue = await ethers.provider.getStorage(
    proxyAddress,
    ERC1967_ADMIN_SLOT,
  );
  return ethers.getAddress(`0x${storageValue.slice(-40)}`);
}

async function deployUpgradeableContract(
  contractName: string,
  proxyAdminOwner: string,
  initializerArgs: any[] = [],
): Promise<
  readonly [any, string, string, string]
> {
  const factory = await ethers.getContractFactory(contractName);
  const implementation = await factory.deploy();
  await implementation.waitForDeployment();
  const implementationAddress = await implementation.getAddress();

  const initData = factory.interface.encodeFunctionData(
    "initialize",
    initializerArgs,
  );
  const proxyFactory = await ethers.getContractFactory(
    "TransparentUpgradeableProxy",
  );
  const proxy = await proxyFactory.deploy(
    implementationAddress,
    proxyAdminOwner,
    initData,
  );
  await proxy.waitForDeployment();
  const proxyAddress = await proxy.getAddress();
  const proxyAdminAddress = await getProxyAdminAddress(proxyAddress);
  const proxiedContract = factory.attach(proxyAddress);

  return [
    proxiedContract,
    proxyAddress,
    implementationAddress,
    proxyAdminAddress,
  ] as const;
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
  const proxyAdminOwnerAddress = getAddressFromPrivateKey(
    process.env.PROXY_ADMIN_PRIVATE_KEY,
    deployerAddress,
  );

  const [
    actorRegistry,
    actorRegistryAddress,
    actorRegistryImplementationAddress,
    actorRegistryProxyAdminAddress,
  ] = await deployUpgradeableContract(
    "ActorRegistry",
    proxyAdminOwnerAddress,
    [deployerAddress],
  );
  const [
    marketController,
    marketControllerAddress,
    marketControllerImplementationAddress,
    marketControllerProxyAdminAddress,
  ] = await deployUpgradeableContract(
    "MarketController",
    proxyAdminOwnerAddress,
    [deployerAddress],
  );
  const [
    orderRegistry,
    orderRegistryAddress,
    orderRegistryImplementationAddress,
    orderRegistryProxyAdminAddress,
  ] = await deployUpgradeableContract(
    "OrderRegistry",
    proxyAdminOwnerAddress,
    [deployerAddress, marketControllerAddress, actorRegistryAddress],
  );
  const [
    tradeSettlement,
    tradeSettlementAddress,
    tradeSettlementImplementationAddress,
    tradeSettlementProxyAdminAddress,
  ] = await deployUpgradeableContract(
    "TradeSettlement",
    proxyAdminOwnerAddress,
    [deployerAddress, orderRegistryAddress],
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
    `export ACTOR_REGISTRY_IMPLEMENTATION_ADDRESS=${actorRegistryImplementationAddress}`,
    `export ACTOR_REGISTRY_PROXY_ADMIN_ADDRESS=${actorRegistryProxyAdminAddress}`,
    `export MARKET_CONTROLLER_ADDRESS=${marketControllerAddress}`,
    `export MARKET_CONTROLLER_IMPLEMENTATION_ADDRESS=${marketControllerImplementationAddress}`,
    `export MARKET_CONTROLLER_PROXY_ADMIN_ADDRESS=${marketControllerProxyAdminAddress}`,
    `export CONTRACT_MARKET_CONTROLLER=${marketControllerAddress}`,
    `export ORDER_REGISTRY_ADDRESS=${orderRegistryAddress}`,
    `export ORDER_REGISTRY_IMPLEMENTATION_ADDRESS=${orderRegistryImplementationAddress}`,
    `export ORDER_REGISTRY_PROXY_ADMIN_ADDRESS=${orderRegistryProxyAdminAddress}`,
    `export CONTRACT_ORDER_REGISTRY=${orderRegistryAddress}`,
    `export TRADE_SETTLEMENT_ADDRESS=${tradeSettlementAddress}`,
    `export TRADE_SETTLEMENT_IMPLEMENTATION_ADDRESS=${tradeSettlementImplementationAddress}`,
    `export TRADE_SETTLEMENT_PROXY_ADMIN_ADDRESS=${tradeSettlementProxyAdminAddress}`,
    `export CONTRACT_TRADE_SETTLEMENT=${tradeSettlementAddress}`,
    `export ACTOR_REGISTRAR_ADDRESS=${actorRegistrarAddress}`,
    `export PROXY_ADMIN_OWNER_ADDRESS=${proxyAdminOwnerAddress}`,
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
  console.log(`  actorRegistryImpl      ${actorRegistryImplementationAddress}`);
  console.log(`  marketControllerImpl   ${marketControllerImplementationAddress}`);
  console.log(`  orderRegistryImpl      ${orderRegistryImplementationAddress}`);
  console.log(`  tradeSettlementImpl    ${tradeSettlementImplementationAddress}`);
  console.log(`  actorRegistryAdmin     ${actorRegistryProxyAdminAddress}`);
  console.log(`  marketControllerAdmin  ${marketControllerProxyAdminAddress}`);
  console.log(`  orderRegistryAdmin     ${orderRegistryProxyAdminAddress}`);
  console.log(`  tradeSettlementAdmin   ${tradeSettlementProxyAdminAddress}`);
  console.log(`  proxyAdminOwner        ${proxyAdminOwnerAddress}`);
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
