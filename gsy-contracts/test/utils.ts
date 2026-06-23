import { ethers } from "hardhat";

export const ORDER_TYPE_BID = true;
export const ORDER_TYPE_ASK = false;
export const ZERO_BYTES16 = "0x00000000000000000000000000000000";
export const ERC1967_ADMIN_SLOT =
  "0xb53127684a568b3173ae13b9f8a6016e243e63b6e8ee1178d6a717850b5d6103";

export function bytes16Id(seed: string) {
  return ethers.dataSlice(ethers.keccak256(ethers.toUtf8Bytes(seed)), 0, 16);
}

export const SCALING_FACTOR = 10000n;

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
