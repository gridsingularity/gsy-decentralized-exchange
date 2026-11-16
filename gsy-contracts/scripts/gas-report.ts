import { ethers } from "hardhat";
import type { Signer } from "ethers";
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

function parseSettleBatchSizes(): number[] {
  const raw = process.env.GAS_REPORT_SETTLE_BATCH_SIZES ?? "1,2,5";
  const sizes = raw.split(",").map((value) => {
    const parsed = Number(value.trim());
    if (!Number.isSafeInteger(parsed) || parsed <= 0) {
      throw new Error(
        `Invalid GAS_REPORT_SETTLE_BATCH_SIZES value '${value}'. Expected comma-separated positive integers.`,
      );
    }
    return parsed;
  });

  return [...new Set(sizes)].sort((left, right) => left - right);
}

// MarketController.MarketType.Spot and MatchingAlgorithm.PayAsBid.
const MARKET_TYPE_SPOT = 0;
const MATCHING_ALGORITHM_PAY_AS_BID = 0;

// Matches primitives::db_api_schema::trades::ClearingStatus::Final.to_evm().
const CLEARING_STATUS_FINAL = 1;

function buildMatch(
  tradeId: string,
  bid: any,
  offer: any,
  residualOfferId: string,
  matchType = 0,
) {
  return {
    tradeId,
    bid,
    offer,
    residualBidId: ethers.ZeroHash.slice(0, 34),
    residualOfferId,
    selectedEnergy: 100_000,
    clearingPrice: 12_000,
    matchType,
  };
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
  initializerArgs: any[],
  signer: Signer,
): Promise<DeploymentResult> {
  console.log(`Deploying ${contractName} implementation...`);
  const factory = await ethers.getContractFactory(contractName, signer);
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
    signer,
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
    "- `settleBatch(Match[N])` rows are measured for `GAS_REPORT_SETTLE_BATCH_SIZES` values; prerequisite dummy order placements are reported as separate mutating calls.",
    "- Standard (matchType=0) and preferred (matchType=1: buyer-only, seller-only, reciprocal) batches each fill 100000 of a 100000 bid and a 150000 offer per match, at price 12000. Each match includes on-chain creation of a 50000 residual offer.",
    "- Preferred benchmarks set both effective rates to 12000. The counterpart without a preferred partner uses its normal rate when no preferred rate is supplied. Differences include calldata and storage access costs, not just validation instructions.",
    "- Direct `settleOrder` rows use a benchmark-only settlement role. Production `settleBatch` rows already include these internal calls; do not add their gas again when estimating a trade.",
    "- Totals include all benchmark fixtures and alternative cases, not the cost of a single production deployment/trading workflow. Local fees are illustrative, not remote-network price estimates.",
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

export async function main() {
  entries.length = 0;
  const signers = await ethers.getSigners();
  if (!signers[0]) {
    throw new Error("No deployer signer configured");
  }

  // Keep one nonce sequence across deployments and calls, including the remote
  // single-account case where the deployer also acts as the registrar.
  const deployer = new ethers.NonceManager(signers[0]);
  const actorRegistrar = signers[1]
    ? new ethers.NonceManager(signers[1])
    : deployer;

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
    deployer,
  );
  const marketController = await deployUpgradeableContract(
    "MarketController",
    deployerAddress,
    [deployerAddress],
    deployer,
  );
  const orderRegistry = await deployUpgradeableContract(
    "OrderRegistry",
    deployerAddress,
    [deployerAddress, marketController.proxyAddress],
    deployer,
  );
  const tradeSettlement = await deployUpgradeableContract(
    "TradeSettlement",
    deployerAddress,
    [deployerAddress, orderRegistry.proxyAddress],
    deployer,
  );

  const actorRegistryContract = actorRegistry.contract as any;
  const marketControllerContract = marketController.contract as any;
  const orderRegistryContract = orderRegistry.contract as any;
  const tradeSettlementContract = tradeSettlement.contract as any;

  const ORCHESTRATOR_ROLE = ethers.id("ORCHESTRATOR_ROLE");
  const SETTLEMENT_ROLE = ethers.id("SETTLEMENT_ROLE");
  const COMMUNITY_CLIENT_ROLE = ethers.id("COMMUNITY_CLIENT_ROLE");
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
    "grantRole(COMMUNITY_CLIENT_ROLE)",
    "OrderRegistry",
    orderRegistryContract.grantRole(COMMUNITY_CLIENT_ROLE, deployerAddress),
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
    "Benchmark-only grant used to measure updateStatus and settleOrder directly.",
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
  const settleBatchSizes = parseSettleBatchSizes();
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
  const block = await withTimeout(
    "Benchmark timestamp fetch",
    ethers.provider.getBlock("latest"),
    RPC_CALL_TIMEOUT_MS,
  );
  if (!block) throw new Error("Missing latest block for benchmark timing");
  const now = block.timestamp;
  // A remote run sends many transactions; do not close at the next quarter hour.
  const timeSlot = Math.floor(now / 900) * 900 + 24 * 3600;

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

  // Open now: the order placements below need an open market.
  const newMarket = (id: string, marketType: number) => ({
    marketId: id,
    communityId: bytes16Id("gas:community"),
    openingTime: now,
    closingTime: timeSlot,
    deliveryStartTime: timeSlot,
    deliveryEndTime: timeSlot + 900,
    marketType,
    matchingAlgorithm: MATCHING_ALGORITHM_PAY_AS_BID,
  });
  const batchMarketIds = [0, 1, 2].map((index) =>
    bytes16Id(`gas:batch-market-${index}`),
  );
  await recordTx(
    "Mutating calls",
    "createMarkets(1 market)",
    "MarketController",
    marketControllerContract.createMarkets([
      newMarket(marketId, MARKET_TYPE_SPOT),
    ]),
  );
  await recordTx(
    "Mutating calls",
    "createMarkets(3 markets)",
    "MarketController",
    marketControllerContract.createMarkets(
      batchMarketIds.map((id, marketType) => newMarket(id, marketType)),
    ),
    "One community's spot, flex and settlement markets for one delivery slot.",
  );
  await recordTx(
    "Mutating calls",
    "createMarkets(3 markets, all existing)",
    "MarketController",
    marketControllerContract.createMarkets(
      batchMarketIds.map((id, marketType) => newMarket(id, marketType)),
    ),
    "Existing markets are skipped; the cost of resending a window.",
  );
  const resendMarketIds = Array.from({ length: 50 }, (_, index) =>
    bytes16Id(`gas:resend-market-${index}`),
  );
  await recordTx(
    "Mutating calls",
    "createMarkets(50 markets)",
    "MarketController",
    marketControllerContract.createMarkets(
      resendMarketIds.map((id) => newMarket(id, MARKET_TYPE_SPOT)),
    ),
  );
  await recordTx(
    "Mutating calls",
    "createMarkets(50 markets, all existing)",
    "MarketController",
    marketControllerContract.createMarkets(
      resendMarketIds.map((id) => newMarket(id, MARKET_TYPE_SPOT)),
    ),
    "Existing markets are skipped; the cost of resending a window.",
  );

  const bidOrder = {
    orderId: bidId,
    createdBy: buyerActor,
    marketId,
    timeSlot,
    creationTime: now,
    energy: 100_000,
    energyRate: 15_000,
    energySourcePreference: 0,
    energyType: 0,
    isBid: true,
    preferredTradingPartner: ethers.ZeroHash.slice(0, 34),
    preferredEnergyRate: 0,
  };
  const offerOrder = {
    orderId: offerId,
    createdBy: sellerActor,
    marketId,
    timeSlot,
    creationTime: now + 1,
    energy: 150_000,
    energyRate: 10_000,
    energySourcePreference: 0,
    energyType: 0,
    isBid: false,
    preferredTradingPartner: ethers.ZeroHash.slice(0, 34),
    preferredEnergyRate: 0,
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

  const directOrder = { ...bidOrder, orderId: bytes16Id("gas:direct-settle") };
  const directResidualId = bytes16Id("gas:direct-residual");
  await recordTx(
    "Mutating calls",
    "placeOrder(OrderParams) settleOrder benchmark",
    "OrderRegistry",
    orderRegistryContract.placeOrder(directOrder),
  );
  await recordTx(
    "Mutating calls",
    "settleOrder(bytes16,uint64,bytes16) partial fill",
    "OrderRegistry",
    orderRegistryContract.settleOrder(directOrder.orderId, 50_000, directResidualId),
    "Benchmark-only direct call; consumes parent and registers a 50000 residual bid with inherited metadata.",
  );
  await recordTx(
    "Mutating calls",
    "settleOrder(bytes16,uint64,bytes16) consume residual",
    "OrderRegistry",
    orderRegistryContract.settleOrder(directResidualId, 50_000, ethers.ZeroHash.slice(0, 34)),
    "Benchmark-only direct call; fully consumes the previously registered residual bid.",
  );

  const settleBatchMatches = new Map<number, any[]>();
  settleBatchMatches.set(1, [
    buildMatch(tradeId, bidOrder, offerOrder, residualOfferId),
  ]);

  for (const batchSize of settleBatchSizes.filter((size) => size !== 1)) {
    const matches = [];
    for (let index = 0; index < batchSize; index++) {
      const bid = {
        ...bidOrder,
        orderId: bytes16Id(`gas:settle-batch-${batchSize}:bid:${index}`),
        creationTime: now + 10 + batchSize * 100 + index * 2,
      };
      const offer = {
        ...offerOrder,
        orderId: bytes16Id(`gas:settle-batch-${batchSize}:offer:${index}`),
        creationTime: now + 11 + batchSize * 100 + index * 2,
      };

      await recordTx(
        "Mutating calls",
        `placeOrder(OrderParams) settleBatch[${batchSize}] bid ${index + 1}`,
        "OrderRegistry",
        orderRegistryContract.placeOrder(bid),
        "Prepares unique open bid for settleBatch gas benchmark.",
      );
      await recordTx(
        "Mutating calls",
        `placeOrder(OrderParams) settleBatch[${batchSize}] offer ${index + 1}`,
        "OrderRegistry",
        orderRegistryContract.placeOrder(offer),
        "Prepares unique open offer for settleBatch gas benchmark.",
      );

      matches.push(
        buildMatch(
          bytes16Id(`gas:settle-batch-${batchSize}:trade:${index}`),
          bid,
          offer,
          bytes16Id(`gas:settle-batch-${batchSize}:residual-offer:${index}`),
        ),
      );
    }
    settleBatchMatches.set(batchSize, matches);
  }

  const asMarketBatch = (matches: any[]) => [{
    matches,
    clearingResult: {
      marketId,
      clearingStatus: CLEARING_STATUS_FINAL,
      clearingPrice: 12_000,
      totalSupply: 0,
      totalDemand: 0,
      tradedQuantity: matches.reduce((total, match) => total + BigInt(match.selectedEnergy), 0n),
      numTrades: matches.length,
    },
  }];

  for (const batchSize of settleBatchSizes) {
    const matches = settleBatchMatches.get(batchSize);
    if (!matches) {
      throw new Error(`Missing settleBatch matches for batch size ${batchSize}`);
    }

    await recordTx(
      "Mutating calls",
      `settleBatch(Match[${batchSize}])`,
      "TradeSettlement",
      tradeSettlementContract.settleBatch(asMarketBatch(matches)),
      "Standard matchType=0; includes residual offer registration for each match. Order placement gas is separate.",
    );
  }

  for (const preference of ["buyer-only", "seller-only", "reciprocal"]) {
    for (const batchSize of settleBatchSizes) {
      const matches = [];
      for (let index = 0; index < batchSize; index++) {
        const seed = `gas:preferred:${preference}:${batchSize}:${index}`;
        const buyerPrefers = preference !== "seller-only";
        const sellerPrefers = preference !== "buyer-only";
        const bid = {
          ...bidOrder,
          orderId: bytes16Id(`${seed}:bid`),
          energyRate: buyerPrefers ? bidOrder.energyRate : 12_000,
          preferredTradingPartner: buyerPrefers ? sellerActor : ethers.ZeroHash.slice(0, 34),
          preferredEnergyRate: buyerPrefers ? 12_000 : 0,
        };
        const offer = {
          ...offerOrder,
          orderId: bytes16Id(`${seed}:offer`),
          energyRate: sellerPrefers ? offerOrder.energyRate : 12_000,
          preferredTradingPartner: sellerPrefers ? buyerActor : ethers.ZeroHash.slice(0, 34),
          preferredEnergyRate: sellerPrefers ? 12_000 : 0,
        };
        for (const [side, order] of [["bid", bid], ["offer", offer]] as const) {
          await recordTx(
            "Mutating calls",
            `placeOrder(OrderParams) preferred ${preference} [${batchSize}] ${side} ${index + 1}`,
            "OrderRegistry",
            orderRegistryContract.placeOrder(order),
            "Prepares a unique open order for the preferred settlement benchmark.",
          );
        }
        matches.push(buildMatch(
          bytes16Id(`${seed}:trade`), bid, offer, bytes16Id(`${seed}:residual-offer`), 1,
        ));
      }
      await recordTx(
        "Mutating calls",
        `settleBatch(Match[${batchSize}]) preferred ${preference}`,
        "TradeSettlement",
        tradeSettlementContract.settleBatch(asMarketBatch(matches)),
        "Preferred matchType=1; includes residual offer registration for each match. Order placement gas is separate.",
      );
    }
  }

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

  // Use fresh trades so verification measures real state changes, not retries.
  for (const sellerPenaltyEnergy of [0, 12_345]) {
    const label = sellerPenaltyEnergy === 0 ? "zero penalties" : "seller penalty";
    const verificationTradeId = bytes16Id(`gas:verification:${label}:trade`);
    const bid = {
      ...bidOrder,
      orderId: bytes16Id(`gas:verification:${label}:bid`),
    };
    const offer = {
      ...offerOrder,
      orderId: bytes16Id(`gas:verification:${label}:offer`),
    };
    for (const order of [bid, offer]) {
      await recordTx(
        "Mutating calls",
        `placeOrder(OrderParams) verification ${label} ${order.isBid ? "bid" : "offer"}`,
        "OrderRegistry",
        orderRegistryContract.placeOrder(order),
        "Prerequisite for delivery verification gas benchmark.",
      );
    }
    const match = buildMatch(
      verificationTradeId,
      bid,
      offer,
      bytes16Id(`gas:verification:${label}:residual-offer`),
    );
    await recordTx(
      "Mutating calls",
      `settleBatch(Match[1]) verification ${label}`,
      "TradeSettlement",
      tradeSettlementContract.settleBatch([
        {
          matches: [match],
          clearingResult: {
            marketId,
            clearingStatus: CLEARING_STATUS_FINAL,
            clearingPrice: match.clearingPrice,
            totalSupply: 0,
            totalDemand: 0,
            tradedQuantity: match.selectedEnergy,
            numTrades: 1,
          },
        },
      ]),
      "Prerequisite for delivery verification gas benchmark.",
    );
    await recordTx(
      "Mutating calls",
      `completeDeliveryVerification(DeliveryVerification[1]) ${label}`,
      "TradeSettlement",
      tradeSettlementContract.completeDeliveryVerification([
        {
          tradeId: verificationTradeId,
          buyerPenaltyEnergy: 0,
          sellerPenaltyEnergy,
        },
      ]),
      "Fresh settled trade with no existing trade penalties; prerequisite gas is reported separately.",
    );
  }

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
    "marketsExist(3 markets)",
    "MarketController",
    marketControllerContract.marketsExist.estimateGas(batchMarketIds),
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
      settleBatchSizes: settleBatchSizes.join(","),
      matchTypes: "Standard=0,Preferred=1",
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

if (require.main === module) {
  main().catch((error) => {
    console.error(error);
    process.exitCode = 1;
  });
}
