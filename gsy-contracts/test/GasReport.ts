import { expect } from "chai";
import { ethers, network } from "hardhat";
import { mkdtempSync, readFileSync, rmSync } from "fs";
import { tmpdir } from "os";
import { join } from "path";
import { main } from "../scripts/gas-report";

describe("Gas report", function () {
  this.timeout(60_000);

  for (const singleSigner of [true, false]) {
    it(`survives slow blocks and stale nonce reads with ${singleSigner ? "one signer" : "separate signers"}`, async function () {
      const snapshot = await network.provider.send("evm_snapshot");
      const output = mkdtempSync(join(tmpdir(), "gsy-gas-report-"));
      const originalGetSigners = ethers.getSigners;
      const originalSend = ethers.provider.send;
      const originalGetTransactionCount = ethers.provider.getTransactionCount;
      const originalLog = console.log;
      const overrides = {
        GAS_REPORT_PATH: join(output, "report.md"),
        GAS_REPORT_JSON_PATH: join(output, "report.json"),
        GAS_REPORT_SETTLE_BATCH_SIZES: "1",
      };
      const savedEnv = Object.fromEntries(
        Object.keys(overrides).map((key) => [key, process.env[key]]),
      );
      const signers = await originalGetSigners();
      const initialNonces = new Map<string, number>();
      const nextNonces = new Map<string, number>();
      for (const signer of signers.slice(0, singleSigner ? 1 : 2)) {
        const address = (await signer.getAddress()).toLowerCase();
        const nonce = await signer.getNonce("pending");
        initialNonces.set(address, nonce);
        nextNonces.set(address, nonce);
      }

      try {
        const latest = (await ethers.provider.getBlock("latest"))!;
        const start = Math.floor(latest.timestamp / 900) * 900 + 899;
        await network.provider.send("evm_setNextBlockTimestamp", [start]);
        await network.provider.send("evm_mine");
        Object.assign(process.env, overrides);
        console.log = () => {};
        ethers.getSigners = async () => singleSigner ? [signers[0]] : signers;
        ethers.provider.getTransactionCount = async (address, blockTag) => {
          if (blockTag === "pending") {
            // Simulate an RPC returning the same stale pending nonce repeatedly.
            const resolved = await ethers.resolveAddress(address);
            return initialNonces.get(resolved.toLowerCase())!;
          }
          return originalGetTransactionCount.call(ethers.provider, address, blockTag);
        };
        ethers.provider.send = async (method, params) => {
          if (method === "eth_sendTransaction") {
            const tx = params?.[0];
            const address = tx.from.toLowerCase();
            const expected = nextNonces.get(address)!;
            expect(tx.nonce, "every submission must carry the next explicit nonce")
              .to.equal(ethers.toQuantity(expected));
            nextNonces.set(address, expected + 1);
            // Cross a full delivery slot without wall-clock sleeps or real fees.
            await originalSend.call(ethers.provider, "evm_increaseTime", [30]);
          }
          return originalSend.call(ethers.provider, method, params);
        };

        await main();

        const report = JSON.parse(readFileSync(overrides.GAS_REPORT_JSON_PATH, "utf8"));
        const actions = report.entries.map((entry: { action: string }) => entry.action);
        expect(actions).to.include("completeDeliveryVerification(DeliveryVerification[1]) seller penalty");
        expect(actions).to.include("settleBatch(Match[1]) preferred reciprocal");
        const end = (await ethers.provider.getBlock("latest"))!.timestamp;
        expect(end - start).to.be.greaterThan(900);
        const marketController = await ethers.getContractAt(
          "MarketController", report.metadata.marketControllerProxy,
        );
        const marketId = ethers.dataSlice(ethers.id("gas:market"), 0, 16);
        expect(await marketController.isMarketOpen(marketId)).to.equal(true);
      } finally {
        ethers.getSigners = originalGetSigners;
        ethers.provider.send = originalSend;
        ethers.provider.getTransactionCount = originalGetTransactionCount;
        console.log = originalLog;
        for (const [key, value] of Object.entries(savedEnv)) {
          if (value === undefined) delete process.env[key];
          else process.env[key] = value;
        }
        await network.provider.send("evm_revert", [snapshot]);
        rmSync(output, { recursive: true, force: true });
      }
    });
  }
});
