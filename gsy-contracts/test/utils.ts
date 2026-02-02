import { ethers } from "hardhat";

export const ORDER_TYPE_BID = true;
export const ORDER_TYPE_ASK = false;

export async function hashOrder(order: any) {
  const abiCoder = new ethers.AbiCoder();
  const encoded = abiCoder.encode(
    [
      "address",
      "uint64",
      "bytes32",
      "bytes32",
      "uint64",
      "uint64",
      "uint64",
      "uint64",
      "bool",
    ],
    [
      order.owner,
      order.nonce,
      order.areaUuid,
      order.marketId,
      order.timeSlot,
      order.creationTime,
      order.energy,
      order.energyRate,
      order.isBid,
    ],
  );
  return ethers.keccak256(encoded);
}

export const SCALING_FACTOR = 10000n;
