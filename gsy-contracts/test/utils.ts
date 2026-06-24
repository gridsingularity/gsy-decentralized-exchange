import { ethers } from "hardhat";

export const ORDER_TYPE_BID = true;
export const ORDER_TYPE_ASK = false;
export const ENERGY_TYPE_UNSPECIFIED = 0;
export const ENERGY_TYPE_GREEN = 1;
export const ZERO_BYTES16 = "0x00000000000000000000000000000000";

export function bytes16Id(seed: string) {
  return ethers.dataSlice(ethers.keccak256(ethers.toUtf8Bytes(seed)), 0, 16);
}

export const SCALING_FACTOR = 10000n;
