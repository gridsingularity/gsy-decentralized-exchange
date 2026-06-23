import { ethers } from "hardhat";
import { mkdirSync, writeFileSync } from "fs";
import { dirname } from "path";

const RPC_RETRY_ATTEMPTS = 60;
const RPC_RETRY_DELAY_MS = 1000;
const RPC_CALL_TIMEOUT_MS = Number(process.env.RPC_CALL_TIMEOUT_MS ?? 10000);
const CONTRACT_DEPLOY_TX_TIMEOUT_MS = Number(
  process.env.CONTRACT_DEPLOY_TX_TIMEOUT_MS ?? 300000,
);
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

async function withTimeout<T>(
  label: string,
  promise: Promise<T>,
  timeoutMs: number,
): Promise<T> {
  let timeout: ReturnType<typeof setTimeout> | undefined;
  try {
    return await Promise.race([
      promise,
      new Promise<T>((_, reject) => {
        timeout = setTimeout(
          () => reject(new Error(`${label} timed out after ${timeoutMs}ms`)),
          timeoutMs,
        );
      }),
    ]);
  } finally {
    if (timeout) {
      clearTimeout(timeout);
    }
  }
}

async function waitForRpcAndGetDeployer() {
  let lastError: unknown;

  for (let attempt = 1; attempt <= RPC_RETRY_ATTEMPTS; attempt++) {
    try {
      const [deployer] = await ethers.getSigners();
      await withTimeout(
        "RPC network detection",
        ethers.provider.getNetwork(),
        RPC_CALL_TIMEOUT_MS,
      );
      await withTimeout(
        "RPC block number fetch",
        ethers.provider.getBlockNumber(),
        RPC_CALL_TIMEOUT_MS,
      );
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

async function waitForTransaction(label: string, tx: any) {
  if (!tx) {
    throw new Error(`${label} did not return a transaction`);
  }

  console.log(`${label}: submitted ${tx.hash}`);
  const receipt = await withTimeout(
    `${label} mining`,
    tx.wait(),
    CONTRACT_DEPLOY_TX_TIMEOUT_MS,
  );
  if (!receipt) {
    throw new Error(`${label} did not return a receipt`);
  }
  console.log(
    `${label}: mined in block ${receipt.blockNumber}, gasUsed=${receipt.gasUsed}`,
  );
  return receipt;
}

async function deployUpgradeableContract(
  contractName: string,
  proxyAdminOwner: string,
  initializerArgs: any[] = [],
): Promise<readonly [any, string, string, string]> {
  console.log(`Deploying ${contractName} implementation...`);
  const factory = await ethers.getContractFactory(contractName);
  const implementation = await factory.deploy();
  await waitForTransaction(
    `${contractName} implementation deployment`,
    implementation.deploymentTransaction(),
  );
  const implementationAddress = await implementation.getAddress();
  console.log(
    `${contractName} implementation deployed at ${implementationAddress}`,
  );

  const initData = factory.interface.encodeFunctionData(
    "initialize",
    initializerArgs,
  );
  const proxyFactory = await ethers.getContractFactory(
    "TransparentUpgradeableProxy",
  );
  console.log(`Deploying ${contractName} proxy...`);
  const proxy = await proxyFactory.deploy(
    implementationAddress,
    proxyAdminOwner,
    initData,
  );
  await waitForTransaction(
    `${contractName} proxy deployment`,
    proxy.deploymentTransaction(),
  );
  const proxyAddress = await proxy.getAddress();
  const proxyAdminAddress = await getProxyAdminAddress(proxyAddress);
  const proxiedContract = factory.attach(proxyAddress);
  console.log(
    `${contractName} proxy deployed at ${proxyAddress}, ProxyAdmin=${proxyAdminAddress}`,
  );

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
  const network = await ethers.provider.getNetwork();

  if (
    network.chainId === 246n &&
    process.env.ALLOW_EWC_MAINNET_DEPLOY !== "true"
  ) {
    throw new Error(
      "Refusing to deploy to Energy Web Chain mainnet without ALLOW_EWC_MAINNET_DEPLOY=true",
    );
  }

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

  await waitForTransaction(
    "Grant MarketController.ORCHESTRATOR_ROLE",
    await marketController.grantRole(ORCHESTRATOR_ROLE, orchestratorAddress),
  );
  await waitForTransaction(
    "Grant OrderRegistry.SETTLEMENT_ROLE",
    await orderRegistry.grantRole(SETTLEMENT_ROLE, tradeSettlementAddress),
  );
  await waitForTransaction(
    "Grant TradeSettlement.OPERATOR_ROLE",
    await tradeSettlement.grantRole(OPERATOR_ROLE, matchingEngineAddress),
  );
  await waitForTransaction(
    "Grant TradeSettlement.EXECUTION_ENGINE_ROLE",
    await tradeSettlement.grantRole(
      EXECUTION_ENGINE_ROLE,
      executionEngineAddress,
    ),
  );
  await waitForTransaction(
    "Grant ActorRegistry.ACTOR_REGISTRAR_ROLE",
    await actorRegistry.grantRole(ACTOR_REGISTRAR_ROLE, actorRegistrarAddress),
  );

  const envFilePath =
    process.env.CONTRACTS_ENV_PATH ?? "/contracts/addresses.env";
  const envFileContent = [
    `ACTOR_REGISTRY_ADDRESS=${actorRegistryAddress}`,
    `ACTOR_REGISTRY_IMPLEMENTATION_ADDRESS=${actorRegistryImplementationAddress}`,
    `ACTOR_REGISTRY_PROXY_ADMIN_ADDRESS=${actorRegistryProxyAdminAddress}`,
    `MARKET_CONTROLLER_ADDRESS=${marketControllerAddress}`,
    `MARKET_CONTROLLER_IMPLEMENTATION_ADDRESS=${marketControllerImplementationAddress}`,
    `MARKET_CONTROLLER_PROXY_ADMIN_ADDRESS=${marketControllerProxyAdminAddress}`,
    `CONTRACT_MARKET_CONTROLLER=${marketControllerAddress}`,
    `ORDER_REGISTRY_ADDRESS=${orderRegistryAddress}`,
    `ORDER_REGISTRY_IMPLEMENTATION_ADDRESS=${orderRegistryImplementationAddress}`,
    `ORDER_REGISTRY_PROXY_ADMIN_ADDRESS=${orderRegistryProxyAdminAddress}`,
    `CONTRACT_ORDER_REGISTRY=${orderRegistryAddress}`,
    `TRADE_SETTLEMENT_ADDRESS=${tradeSettlementAddress}`,
    `TRADE_SETTLEMENT_IMPLEMENTATION_ADDRESS=${tradeSettlementImplementationAddress}`,
    `TRADE_SETTLEMENT_PROXY_ADMIN_ADDRESS=${tradeSettlementProxyAdminAddress}`,
    `CONTRACT_TRADE_SETTLEMENT=${tradeSettlementAddress}`,
    `ACTOR_REGISTRAR_ADDRESS=${actorRegistrarAddress}`,
    `PROXY_ADMIN_OWNER_ADDRESS=${proxyAdminOwnerAddress}`,
    "",
  ].join("\n");

  mkdirSync(dirname(envFilePath), { recursive: true });
  writeFileSync(envFilePath, envFileContent);

  console.log("Contracts deployed and roles granted:");
  console.log(`  network                ${network.name} (${network.chainId})`);
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
