import { loadFixture, time } from "@nomicfoundation/hardhat-toolbox/network-helpers";
import { expect } from "chai";
import { ethers } from "hardhat";
import {
  bytes16Id,
  deployUpgradeableContract,
  MARKET_TYPE_FLEX,
  MARKET_TYPE_SETTLEMENT,
  MATCHING_ALGORITHM_AMM,
  MATCHING_ALGORITHM_PAY_AS_CLEAR,
  newMarket,
  ZERO_BYTES16,
} from "./utils";

describe("MarketController", function () {
  async function deployControllerFixture() {
    const [admin, orchestrator, user] = await ethers.getSigners();
    const controller = await deployUpgradeableContract("MarketController", [
      admin.address,
    ]);

    const ORCHESTRATOR_ROLE = await controller.ORCHESTRATOR_ROLE();
    await controller.grantRole(ORCHESTRATOR_ROLE, orchestrator.address);

    return { controller, admin, orchestrator, user };
  }

  function eventArgs(market: any, createdAt: bigint | number) {
    return [
      market.marketId,
      market.communityId,
      market.openingTime,
      market.closingTime,
      market.deliveryStartTime,
      market.deliveryEndTime,
      market.marketType,
      market.matchingAlgorithm,
      createdAt,
    ];
  }

  async function expectStored(
    controller: any,
    market: any,
    createdAt: bigint | number,
  ) {
    const stored = await controller.getMarket(market.marketId);
    expect(stored.communityId).to.equal(market.communityId);
    expect(stored.openingTime).to.equal(market.openingTime);
    expect(stored.closingTime).to.equal(market.closingTime);
    expect(stored.deliveryStartTime).to.equal(market.deliveryStartTime);
    expect(stored.deliveryEndTime).to.equal(market.deliveryEndTime);
    expect(stored.createdAt).to.equal(createdAt);
    expect(stored.marketType).to.equal(market.marketType);
    expect(stored.matchingAlgorithm).to.equal(market.matchingAlgorithm);
    expect(await controller.marketExists(market.marketId)).to.be.true;
  }

  async function nextBlockTimestamp() {
    const timestamp = (await time.latest()) + 1;
    await time.setNextBlockTimestamp(timestamp);
    return timestamp;
  }

  describe("createMarkets", function () {
    it("Should store a new market and emit the full record", async function () {
      const { controller, orchestrator } = await loadFixture(
        deployControllerFixture,
      );
      const market = newMarket(bytes16Id("market-1"), {
        marketType: MARKET_TYPE_FLEX,
        matchingAlgorithm: MATCHING_ALGORITHM_PAY_AS_CLEAR,
      });
      const createdAt = await nextBlockTimestamp();

      await expect(controller.connect(orchestrator).createMarkets([market]))
        .to.emit(controller, "NewMarketCreated")
        .withArgs(...eventArgs(market, createdAt));
      await expectStored(controller, market, createdAt);
    });

    it("Should create multiple markets in one transaction", async function () {
      const { controller, orchestrator } = await loadFixture(
        deployControllerFixture,
      );
      const markets = [
        newMarket(bytes16Id("market-1")),
        newMarket(bytes16Id("market-2"), { marketType: MARKET_TYPE_FLEX }),
        newMarket(bytes16Id("market-3"), {
          marketType: MARKET_TYPE_SETTLEMENT,
          matchingAlgorithm: MATCHING_ALGORITHM_AMM,
        }),
      ];
      const createdAt = await nextBlockTimestamp();

      const createMarkets = controller
        .connect(orchestrator)
        .createMarkets(markets);
      for (const market of markets) {
        await expect(createMarkets)
          .to.emit(controller, "NewMarketCreated")
          .withArgs(...eventArgs(market, createdAt));
      }
      for (const market of markets) {
        await expectStored(controller, market, createdAt);
      }
    });

    it("Should skip a market that already exists", async function () {
      const { controller, orchestrator } = await loadFixture(
        deployControllerFixture,
      );
      const existing = newMarket(bytes16Id("market-1"));
      const fresh = newMarket(bytes16Id("market-2"));
      const createdAt = await nextBlockTimestamp();
      await controller.connect(orchestrator).createMarkets([existing]);
      const freshCreatedAt = await nextBlockTimestamp();

      const createMarkets = controller
        .connect(orchestrator)
        .createMarkets([
          newMarket(existing.marketId, { closingTime: 1500 }),
          fresh,
        ]);
      await expect(createMarkets)
        .to.emit(controller, "NewMarketCreated")
        .withArgs(...eventArgs(fresh, freshCreatedAt));
      const receipt = await (await createMarkets).wait();
      expect(receipt!.logs.length).to.equal(1);
      await expectStored(controller, existing, createdAt);
      await expectStored(controller, fresh, freshCreatedAt);
    });

    it("Should create a market only once if it appears twice in one batch", async function () {
      const { controller, orchestrator } = await loadFixture(
        deployControllerFixture,
      );
      const market = newMarket(bytes16Id("market-1"));
      const createdAt = await nextBlockTimestamp();

      const createMarkets = controller
        .connect(orchestrator)
        .createMarkets([
          market,
          newMarket(market.marketId, { closingTime: 1500 }),
        ]);
      const receipt = await (await createMarkets).wait();
      expect(receipt!.logs.length).to.equal(1);
      await expectStored(controller, market, createdAt);
    });

    const invalidMarkets: [string, string, any][] = [
      ["a zero market id", ZERO_BYTES16, {}],
      ["a zero community id", "", { communityId: ZERO_BYTES16 }],
      ["closing time equal to opening time", "", { closingTime: 1000 }],
      ["closing time before opening time", "", { closingTime: 999 }],
      ["delivery end equal to delivery start", "", { deliveryEndTime: 2000 }],
      ["delivery end before delivery start", "", { deliveryEndTime: 1999 }],
    ];
    for (const [name, marketIdOverride, overrides] of invalidMarkets) {
      it(`Should reject ${name}`, async function () {
        const { controller, orchestrator } = await loadFixture(
          deployControllerFixture,
        );
        const marketId = marketIdOverride || bytes16Id("market-1");

        await expect(
          controller
            .connect(orchestrator)
            .createMarkets([newMarket(marketId, overrides)]),
        )
          .to.be.revertedWithCustomError(controller, "InvalidMarket")
          .withArgs(marketId);
      });
    }

    it("Should revert the whole batch if one market is invalid", async function () {
      const { controller, orchestrator } = await loadFixture(
        deployControllerFixture,
      );
      const validMarketId = bytes16Id("market-1");
      const invalidMarketId = bytes16Id("market-2");

      await expect(
        controller
          .connect(orchestrator)
          .createMarkets([
            newMarket(validMarketId),
            newMarket(invalidMarketId, { closingTime: 0 }),
          ]),
      )
        .to.be.revertedWithCustomError(controller, "InvalidMarket")
        .withArgs(invalidMarketId);
      expect(await controller.marketExists(validMarketId)).to.be.false;
    });

    it("Should prevent unauthorized users from creating markets", async function () {
      const { controller, user } = await loadFixture(deployControllerFixture);

      await expect(
        controller.connect(user).createMarkets([newMarket(bytes16Id("m"))]),
      ).to.be.revertedWithCustomError(
        controller,
        "AccessControlUnauthorizedAccount",
      );
    });

    it("Should allow an empty batch", async function () {
      const { controller, orchestrator } = await loadFixture(
        deployControllerFixture,
      );

      await expect(
        controller.connect(orchestrator).createMarkets([]),
      ).not.to.emit(controller, "NewMarketCreated");
    });
  });

  describe("isMarketOpen", function () {
    it("Should follow the opening and closing time", async function () {
      const { controller, orchestrator } = await loadFixture(
        deployControllerFixture,
      );
      const latest = await time.latest();
      const market = newMarket(bytes16Id("market-1"), {
        openingTime: latest + 100,
        closingTime: latest + 200,
        deliveryStartTime: latest + 200,
        deliveryEndTime: latest + 300,
      });
      await controller.connect(orchestrator).createMarkets([market]);

      expect(await controller.isMarketOpen(market.marketId)).to.be.false;

      await time.increaseTo(market.openingTime - 1);
      expect(await controller.isMarketOpen(market.marketId)).to.be.false;

      await time.increaseTo(market.openingTime);
      expect(await controller.isMarketOpen(market.marketId)).to.be.true;

      await time.increaseTo(market.closingTime - 1);
      expect(await controller.isMarketOpen(market.marketId)).to.be.true;

      await time.increaseTo(market.closingTime);
      expect(await controller.isMarketOpen(market.marketId)).to.be.false;
    });

    it("Should be false for an unknown market", async function () {
      const { controller } = await loadFixture(deployControllerFixture);

      expect(await controller.isMarketOpen(bytes16Id("unknown-market"))).to.be
        .false;
    });
  });

  it("Should return an empty record for an unknown market", async function () {
    const { controller } = await loadFixture(deployControllerFixture);
    const marketId = bytes16Id("unknown-market");

    expect(await controller.marketExists(marketId)).to.be.false;
    expect((await controller.getMarket(marketId)).createdAt).to.equal(0);
  });
});
