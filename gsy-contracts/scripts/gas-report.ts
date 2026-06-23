import { ethers } from "hardhat";
import { mkdirSync, writeFileSync } from "fs";
import { dirname } from "path";

const ERC1967_ADMIN_SLOT =
  "0xb53127684a568b3173ae13b9f8a6016e243e63b6e8ee1178d6a717850b5d6103";
const RPC_CALL_TIMEOUT_MS = Number(process.env.RPC_CALL_TIMEOUT_MS ?? 10000);
const CONTRACT_DEPLOY_TX_TIMEOUT_MS = Number(
  process.env.CONTRACT_DEPLOY_TX_TIMEOUT_MS ?? 300000,
);

interface GasEntry {
  section: string;
  action: string;
  contract?: string;
  txHash?: string;
  gasUsed: string;
  effectiveGasPriceWei?: string;
  feeWei?: string;
  feeNative?: string;
  notes?: string;
}

interface DeploymentResult {
  contract: any;
  proxyAddress: string;
  implementationAddress: string;
  proxyAdminAddress: string;
}

const entries: GasEntry[] = [];

function bytes16Id(seed: string): string {
  return ethers.dataSlice(ethers.keccak256(ethers.toUtf8Bytes(seed)), 0, 16);
}

function nativeSymbol(chainId: bigint): string {
  if (chainId === 246n) return "EWT";
  if (chainId === 73799n) return "VT";
  return "ETH";
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

async function getProxyAdminAddress(proxyAddress: string): Promise<string> {
  const storageValue = await withTimeout(
    `ProxyAdmin storage read for ${proxyAddress}`,
    ethers.provider.getStorage(proxyAddress, ERC1967_ADMIN_SLOT),
    RPC_CALL_TIMEOUT_MS,
  );
  return ethers.getAddress(`0x${storageValue.slice(-40)}`);
}

async function recordReceipt(
  section: string,
  action: string,
  contract: string | undefined,
  tx: any,
  notes?: string,
): Promise<void> {
  if (!tx) {
    throw new Error(`Missing tx for ${section} / ${action}`);
  }

  console.log(`${section} / ${action}: submitted ${tx.hash}`);
  const receipt = await withTimeout(
    `${section} / ${action} mining`,
    tx.wait(),
    CONTRACT_DEPLOY_TX_TIMEOUT_MS,
  );
  if (!receipt) {
    throw new Error(`Missing receipt for ${section} / ${action}`);
  }
  console.log(
    `${section} / ${action}: mined in block ${receipt.blockNumber}, gasUsed=${receipt.gasUsed}`,
  );

  const gasUsed = BigInt(receipt.gasUsed);
  const effectiveGasPrice = BigInt(
    receipt.gasPrice ?? receipt.effectiveGasPrice ?? 0n,
  );
  const feeWei = gasUsed * effectiveGasPrice;

  entries.push({
    section,
    action,
    contract,
    txHash: receipt.hash,
    gasUsed: gasUsed.toString(),
    effectiveGasPriceWei: effectiveGasPrice.toString(),
    feeWei: feeWei.toString(),
    feeNative: ethers.formatEther(feeWei),
    notes,
  });
}

async function recordTx(
  section: string,
  action: string,
  contract: string,
  txPromise: Promise<any>,
  notes?: string,
): Promise<void> {
  console.log(`${section} / ${action}: submitting...`);
  const tx = await txPromise;
  await recordReceipt(section, action, contract, tx, notes);
}

async function recordEstimate(
  section: string,
  action: string,
  contract: string,
  gasPromise: Promise<bigint>,
  notes?: string,
): Promise<void> {
  console.log(`${section} / ${action}: estimating...`);
  const gasUsed = await withTimeout(
    `${section} / ${action} estimate`,
    gasPromise,
    RPC_CALL_TIMEOUT_MS,
  );
  console.log(`${section} / ${action}: estimate=${gasUsed}`);
  entries.push({
    section,
    action,
    contract,
    gasUsed: gasUsed.toString(),
    notes,
  });
}

async function deployUpgradeableContract(
  contractName: string,
  proxyAdminOwner: string,
  initializerArgs: any[] = [],
): Promise<DeploymentResult> {
  console.log(`Deploying ${contractName} implementation...`);
  const factory = await ethers.getContractFactory(contractName);
  const implementation = await factory.deploy();
  await recordReceipt(
    "Deployment",
    `${contractName} implementation deployment`,
    contractName,
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
  await recordReceipt(
    "Deployment",
    `${contractName} proxy + ProxyAdmin deployment and initialization`,
    contractName,
    proxy.deploymentTransaction(),
    "Includes TransparentUpgradeableProxy deployment, ProxyAdmin deployment, and initializer delegatecall.",
  );
  const proxyAddress = await proxy.getAddress();
  const proxyAdminAddress = await getProxyAdminAddress(proxyAddress);
  console.log(
    `${contractName} proxy deployed at ${proxyAddress}, ProxyAdmin=${proxyAdminAddress}`,
  );

  return {
    contract: factory.attach(proxyAddress),
    proxyAddress,
    implementationAddress,
    proxyAdminAddress,
  };
}

function sectionTotal(section: string): bigint {
  return entries
    .filter((entry) => entry.section === section && entry.feeWei)
    .reduce((sum, entry) => sum + BigInt(entry.feeWei ?? "0"), 0n);
}

function gasTotal(section: string): bigint {
  return entries
    .filter((entry) => entry.section === section)
    .reduce((sum, entry) => sum + BigInt(entry.gasUsed), 0n);
}

function markdownTable(rows: GasEntry[], symbol: string): string {
  const lines = [
    `| Section | Contract | Action | Gas | Gas Price (wei) | Fee (${symbol}) | Tx | Notes |`,
    "|---|---|---|---:|---:|---:|---|---|",
  ];

  for (const row of rows) {
    const tx = row.txHash ? `\`${row.txHash}\`` : "view estimate";
    lines.push(
      `| ${row.section} | ${row.contract ?? ""} | ${row.action} | ${row.gasUsed} | ${row.effectiveGasPriceWei ?? ""} | ${row.feeNative ?? ""} | ${tx} | ${row.notes ?? ""} |`,
    );
  }

  return lines.join("\n");
}

function writeReports(
  reportPath: string,
  jsonPath: string,
  metadata: Record<string, string>,
  symbol: string,
): void {
  const deploymentFee = sectionTotal("Deployment");
  const setupFee = sectionTotal("Role setup");
  const mutatingFee = sectionTotal("Mutating calls");
  const totalFee = deploymentFee + setupFee + mutatingFee;

  const markdown = [
    "# GSY DEX Smart Contract Gas Report",
    "",
    "## Metadata",
    "",
    ...Object.entries(metadata).map(([key, value]) => `- **${key}**: ${value}`),
    "",
    "## Totals",
    "",
    `- Deployment gas: ${gasTotal("Deployment").toString()}`,
    `- Deployment fee: ${ethers.formatEther(deploymentFee)} ${symbol}`,
    `- Role setup gas: ${gasTotal("Role setup").toString()}`,
    `- Role setup fee: ${ethers.formatEther(setupFee)} ${symbol}`,
    `- Mutating call gas: ${gasTotal("Mutating calls").toString()}`,
    `- Mutating call fee: ${ethers.formatEther(mutatingFee)} ${symbol}`,
    `- Deployment + setup + mutating fee: ${ethers.formatEther(totalFee)} ${symbol}`,
    "",
    "## Detailed Values",
    "",
    markdownTable(entries, symbol),
    "",
    "## Notes",
    "",
    "- View functions are reported as `estimateGas` values only; they do not consume gas when called off-chain.",
    "- Proxy deployment rows include the `TransparentUpgradeableProxy`, the internally created `ProxyAdmin`, and initializer delegatecall gas.",
    "- Mainnet/Volta values depend on live gas price at execution time.",
    "",
  ].join("\n");

  mkdirSync(dirname(reportPath), { recursive: true });
  mkdirSync(dirname(jsonPath), { recursive: true });
  writeFileSync(reportPath, markdown);
  writeFileSync(
    jsonPath,
    JSON.stringify(
      {
        metadata,
        totals: {
          deploymentGas: gasTotal("Deployment").toString(),
          deploymentFeeWei: deploymentFee.toString(),
          roleSetupGas: gasTotal("Role setup").toString(),
          roleSetupFeeWei: setupFee.toString(),
          mutatingCallGas: gasTotal("Mutating calls").toString(),
          mutatingCallFeeWei: mutatingFee.toString(),
          totalFeeWei: totalFee.toString(),
        },
        entries,
      },
      null,
      2,
    ),
  );
}

async function main() {
  const signers = await ethers.getSigners();
  const deployer = signers[0];
  const actorRegistrar = signers[1] ?? deployer;

  if (!deployer) {
    throw new Error("No deployer signer configured");
  }

  const deployerAddress = await deployer.getAddress();
  const actorRegistrarAddress = await actorRegistrar.getAddress();
  const signerDelegateAddress = signers[2]
    ? await signers[2].getAddress()
    : undefined;
  const benchmarkDelegateAddress =
    signerDelegateAddress &&
    signerDelegateAddress.toLowerCase() !== deployerAddress.toLowerCase()
      ? signerDelegateAddress
      : "0x000000000000000000000000000000000000dEaD";
  const network = await ethers.provider.getNetwork();
  const symbol = nativeSymbol(network.chainId);
  await withTimeout(
    "RPC block number fetch",
    ethers.provider.getBlockNumber(),
    RPC_CALL_TIMEOUT_MS,
  );

  const isLocal = network.chainId === 31337n || network.chainId === 1337n;
  if (!isLocal && process.env.GAS_REPORT_ALLOW_REMOTE !== "true") {
    throw new Error(
      "Refusing to run gas report on a non-local network without GAS_REPORT_ALLOW_REMOTE=true. This script deploys contracts and sends state-changing transactions.",
    );
  }
  if (
    network.chainId === 246n &&
    process.env.ALLOW_EWC_MAINNET_DEPLOY !== "true"
  ) {
    throw new Error(
      "Refusing to run gas report on Energy Web Chain mainnet without ALLOW_EWC_MAINNET_DEPLOY=true",
    );
  }
  console.log(
    `Running gas report on ${network.name} (${network.chainId}) with deployer ${deployerAddress}`,
  );

  const actorRegistry = await deployUpgradeableContract(
    "ActorRegistry",
    deployerAddress,
    [deployerAddress],
  );
  const marketController = await deployUpgradeableContract(
    "MarketController",
    deployerAddress,
    [deployerAddress],
  );
  const orderRegistry = await deployUpgradeableContract(
    "OrderRegistry",
    deployerAddress,
    [
      deployerAddress,
      marketController.proxyAddress,
      actorRegistry.proxyAddress,
    ],
  );
  const tradeSettlement = await deployUpgradeableContract(
    "TradeSettlement",
    deployerAddress,
    [deployerAddress, orderRegistry.proxyAddress],
  );

  const actorRegistryContract = actorRegistry.contract as any;
  const marketControllerContract = marketController.contract as any;
  const orderRegistryContract = orderRegistry.contract as any;
  const tradeSettlementContract = tradeSettlement.contract as any;

  const ORCHESTRATOR_ROLE = ethers.id("ORCHESTRATOR_ROLE");
  const SETTLEMENT_ROLE = ethers.id("SETTLEMENT_ROLE");
  const OPERATOR_ROLE = ethers.id("OPERATOR_ROLE");
  const EXECUTION_ENGINE_ROLE = ethers.id("EXECUTION_ENGINE_ROLE");
  const ACTOR_REGISTRAR_ROLE = ethers.id("ACTOR_REGISTRAR_ROLE");

  await recordTx(
    "Role setup",
    "grantRole(ORCHESTRATOR_ROLE)",
    "MarketController",
    marketControllerContract.grantRole(ORCHESTRATOR_ROLE, deployerAddress),
  );
  await recordTx(
    "Role setup",
    "grantRole(SETTLEMENT_ROLE, TradeSettlement)",
    "OrderRegistry",
    orderRegistryContract.grantRole(
      SETTLEMENT_ROLE,
      tradeSettlement.proxyAddress,
    ),
  );
  await recordTx(
    "Role setup",
    "grantRole(SETTLEMENT_ROLE, benchmark signer)",
    "OrderRegistry",
    orderRegistryContract.grantRole(SETTLEMENT_ROLE, deployerAddress),
    "Benchmark-only grant used to measure updateStatus directly.",
  );
  await recordTx(
    "Role setup",
    "grantRole(OPERATOR_ROLE)",
    "TradeSettlement",
    tradeSettlementContract.grantRole(OPERATOR_ROLE, deployerAddress),
  );
  await recordTx(
    "Role setup",
    "grantRole(EXECUTION_ENGINE_ROLE)",
    "TradeSettlement",
    tradeSettlementContract.grantRole(EXECUTION_ENGINE_ROLE, deployerAddress),
  );
  await recordTx(
    "Role setup",
    "grantRole(ACTOR_REGISTRAR_ROLE)",
    "ActorRegistry",
    actorRegistryContract.grantRole(ACTOR_REGISTRAR_ROLE, actorRegistrarAddress),
  );

  const actorRegistryRegistrar = actorRegistryContract.connect(
    actorRegistrar,
  ) as any;
  const buyerActor = bytes16Id("gas:buyer-actor");
  const sellerActor = bytes16Id("gas:seller-actor");
  const revokedActor = bytes16Id("gas:revoked-actor");
  const marketId = bytes16Id("gas:market");
  const bidId = bytes16Id("gas:bid");
  const offerId = bytes16Id("gas:offer");
  const cancelOrderId = bytes16Id("gas:cancel-order");
  const statusOrderId = bytes16Id("gas:status-order");
  const tradeId = bytes16Id("gas:trade");
  const residualOfferId = bytes16Id("gas:residual-offer");
  const now = Math.floor(Date.now() / 1000);
  const timeSlot = Math.floor(now / 900) * 900 + 900;

  await recordTx(
    "Mutating calls",
    "registerActor(bytes16,address)",
    "ActorRegistry",
    actorRegistryRegistrar.registerActor(buyerActor, deployerAddress),
  );
  await recordTx(
    "Mutating calls",
    "registerActor(bytes16,address) second actor",
    "ActorRegistry",
    actorRegistryRegistrar.registerActor(sellerActor, deployerAddress),
  );
  await recordTx(
    "Mutating calls",
    "setActorWallet(bytes16,address,true)",
    "ActorRegistry",
    actorRegistryRegistrar.setActorWallet(revokedActor, deployerAddress, true),
  );
  await recordTx(
    "Mutating calls",
    "setActorWallet(bytes16,address,false)",
    "ActorRegistry",
    actorRegistryRegistrar.setActorWallet(revokedActor, deployerAddress, false),
  );
  await recordTx(
    "Mutating calls",
    "setProxy(bytes16,address,true)",
    "ActorRegistry",
    actorRegistryContract.setProxy(buyerActor, benchmarkDelegateAddress, true),
    "Uses a benchmark delegate distinct from the order signer.",
  );
  await recordTx(
    "Mutating calls",
    "setProxy(bytes16,address,false)",
    "ActorRegistry",
    actorRegistryContract.setProxy(buyerActor, benchmarkDelegateAddress, false),
    "Uses a benchmark delegate distinct from the order signer.",
  );

  await recordTx(
    "Mutating calls",
    "setMarketStatus(bytes16,true)",
    "MarketController",
    marketControllerContract.setMarketStatus(marketId, true),
  );
  await recordTx(
    "Mutating calls",
    "setMarketStatus(bytes16,false)",
    "MarketController",
    marketControllerContract.setMarketStatus(marketId, false),
  );
  await recordTx(
    "Mutating calls",
    "setMarketStatus(bytes16,true) reopen",
    "MarketController",
    marketControllerContract.setMarketStatus(marketId, true),
  );

  const bidOrder = {
    orderId: bidId,
    createdBy: buyerActor,
    marketId,
    timeSlot,
    creationTime: now,
    energy: 100_000,
    energyRate: 15_000,
    isBid: true,
  };
  const offerOrder = {
    orderId: offerId,
    createdBy: sellerActor,
    marketId,
    timeSlot,
    creationTime: now + 1,
    energy: 150_000,
    energyRate: 10_000,
    isBid: false,
  };
  const cancelOrder = {
    ...bidOrder,
    orderId: cancelOrderId,
    creationTime: now + 2,
  };
  const statusOrder = {
    ...bidOrder,
    orderId: statusOrderId,
    creationTime: now + 3,
  };

  await recordTx(
    "Mutating calls",
    "placeOrder(OrderParams) bid",
    "OrderRegistry",
    orderRegistryContract.placeOrder(bidOrder),
  );
  await recordTx(
    "Mutating calls",
    "placeOrder(OrderParams) offer",
    "OrderRegistry",
    orderRegistryContract.placeOrder(offerOrder),
  );
  await recordTx(
    "Mutating calls",
    "placeOrder(OrderParams) cancellable order",
    "OrderRegistry",
    orderRegistryContract.placeOrder(cancelOrder),
  );
  await recordTx(
    "Mutating calls",
    "cancelOrder(OrderParams)",
    "OrderRegistry",
    orderRegistryContract.cancelOrder(cancelOrder),
  );
  await recordTx(
    "Mutating calls",
    "placeOrder(OrderParams) status benchmark order",
    "OrderRegistry",
    orderRegistryContract.placeOrder(statusOrder),
  );
  await recordTx(
    "Mutating calls",
    "updateStatus(bytes16,OrderStatus)",
    "OrderRegistry",
    orderRegistryContract.updateStatus(statusOrderId, 2),
    "Measured with benchmark-only settlement role granted to deployer.",
  );

  const match = {
    tradeId,
    bid: {
      orderId: bidOrder.orderId,
      createdBy: bidOrder.createdBy,
      marketId: bidOrder.marketId,
      timeSlot: bidOrder.timeSlot,
      creationTime: bidOrder.creationTime,
      energy: bidOrder.energy,
      energyRate: bidOrder.energyRate,
    },
    offer: {
      orderId: offerOrder.orderId,
      createdBy: offerOrder.createdBy,
      marketId: offerOrder.marketId,
      timeSlot: offerOrder.timeSlot,
      creationTime: offerOrder.creationTime,
      energy: offerOrder.energy,
      energyRate: offerOrder.energyRate,
    },
    residualBidId: ethers.ZeroHash.slice(0, 34),
    residualOfferId,
    selectedEnergy: 100_000,
    clearingPrice: 12_000,
  };

  await recordTx(
    "Mutating calls",
    "settleBatch(Match[1])",
    "TradeSettlement",
    tradeSettlementContract.settleBatch([match]),
  );
  await recordTx(
    "Mutating calls",
    "submitPenalties(TradePenalty[1])",
    "TradeSettlement",
    tradeSettlementContract.submitPenalties([
      {
        penalizedActorId: sellerActor,
        marketId,
        tradeId,
        penaltyEnergy: 12_345,
      },
    ]),
  );

  await recordEstimate(
    "View estimates",
    "isAuthorized(bytes16,address)",
    "ActorRegistry",
    actorRegistryContract.isAuthorized.estimateGas(buyerActor, deployerAddress),
  );
  await recordEstimate(
    "View estimates",
    "isProxy(bytes16,address)",
    "ActorRegistry",
    actorRegistryContract.isProxy.estimateGas(
      buyerActor,
      benchmarkDelegateAddress,
    ),
  );
  await recordEstimate(
    "View estimates",
    "isMarketOpen(bytes16)",
    "MarketController",
    marketControllerContract.isMarketOpen.estimateGas(marketId),
  );
  await recordEstimate(
    "View estimates",
    "getStatus(bytes16)",
    "OrderRegistry",
    orderRegistryContract.getStatus.estimateGas(bidId),
  );
  await recordEstimate(
    "View estimates",
    "getOrder(bytes16)",
    "OrderRegistry",
    orderRegistryContract.getOrder.estimateGas(bidId),
  );
  await recordEstimate(
    "View estimates",
    "penaltyEnergyByTrade(bytes16)",
    "TradeSettlement",
    tradeSettlementContract.penaltyEnergyByTrade.estimateGas(tradeId),
  );
  await recordEstimate(
    "View estimates",
    "penaltyEnergyByActor(bytes16)",
    "TradeSettlement",
    tradeSettlementContract.penaltyEnergyByActor.estimateGas(sellerActor),
  );

  const reportPath = process.env.GAS_REPORT_PATH ?? "/contracts/gas-report.md";
  const jsonPath =
    process.env.GAS_REPORT_JSON_PATH ?? "/contracts/gas-report.json";
  writeReports(
    reportPath,
    jsonPath,
    {
      generatedAt: new Date().toISOString(),
      network: `${network.name} (${network.chainId})`,
      deployer: deployerAddress,
      nativeSymbol: symbol,
      actorRegistryProxy: actorRegistry.proxyAddress,
      marketControllerProxy: marketController.proxyAddress,
      orderRegistryProxy: orderRegistry.proxyAddress,
      tradeSettlementProxy: tradeSettlement.proxyAddress,
    },
    symbol,
  );

  console.log(`Gas report written to ${reportPath}`);
  console.log(`Gas report JSON written to ${jsonPath}`);
}

main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
