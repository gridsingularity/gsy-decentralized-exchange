import { ethers } from "hardhat";

export const ORDER_TYPE_BID = true;
export const ORDER_TYPE_ASK = false;
export const ENERGY_TYPE_NONE = 0;
export const ENERGY_TYPE_GREEN = 1;
export const ZERO_BYTES16 = "0x00000000000000000000000000000000";
export const ERC1967_ADMIN_SLOT =
  "0xb53127684a568b3173ae13b9f8a6016e243e63b6e8ee1178d6a717850b5d6103";

export function bytes16Id(seed: string) {
  return ethers.dataSlice(ethers.keccak256(ethers.toUtf8Bytes(seed)), 0, 16);
}

export const SCALING_FACTOR = 10000n;

// Same order as MarketController.MarketType / MatchingAlgorithm.
export const MARKET_TYPE_SPOT = 0;
export const MARKET_TYPE_FLEX = 1;
export const MARKET_TYPE_SETTLEMENT = 2;
export const MATCHING_ALGORITHM_PAY_AS_BID = 0;
export const MATCHING_ALGORITHM_PAY_AS_CLEAR = 1;
export const MATCHING_ALGORITHM_AMM = 2;

export function newMarket(marketId: string, overrides: any = {}) {
  return {
    marketId,
    communityId: bytes16Id("community-1"),
    openingTime: 1000,
    closingTime: 1900,
    deliveryStartTime: 2000,
    deliveryEndTime: 2900,
    marketType: MARKET_TYPE_SPOT,
    matchingAlgorithm: MATCHING_ALGORITHM_PAY_AS_BID,
    ...overrides,
  };
}

export const OPEN_MARKET_DURATION = 3600;

// Creates a market that is open from the latest block for OPEN_MARKET_DURATION.
// The caller of the controller must hold ORCHESTRATOR_ROLE.
export async function createOpenMarket(controller: any, marketId: string) {
  const latest = (await ethers.provider.getBlock("latest"))!.timestamp;
  await controller.createMarkets([
    newMarket(marketId, {
      openingTime: latest,
      closingTime: latest + OPEN_MARKET_DURATION,
      deliveryStartTime: latest + OPEN_MARKET_DURATION,
      deliveryEndTime: latest + OPEN_MARKET_DURATION + 900,
    }),
  ]);
}

export async function getProxyAdminAddress(proxyAddress: string) {
  const storageValue = await ethers.provider.getStorage(
    proxyAddress,
    ERC1967_ADMIN_SLOT,
  );
  return ethers.getAddress(`0x${storageValue.slice(-40)}`);
}

export async function deployUpgradeableContract(
  contractName: string,
  initializerArgs: any[] = [],
  proxyAdminOwner?: string,
) {
  const [defaultOwner] = await ethers.getSigners();
  const factory = await ethers.getContractFactory(contractName);
  const implementation = await factory.deploy();
  await implementation.waitForDeployment();

  const initData = factory.interface.encodeFunctionData(
    "initialize",
    initializerArgs,
  );
  const proxyFactory = await ethers.getContractFactory(
    "TransparentUpgradeableProxy",
  );
  const proxy = await proxyFactory.deploy(
    await implementation.getAddress(),
    proxyAdminOwner ?? defaultOwner.address,
    initData,
  );
  await proxy.waitForDeployment();

  return factory.attach(await proxy.getAddress());
}
