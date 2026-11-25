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

  function marketKey(market: any) {
    return {
      communityId: market.communityId,
      marketType: market.marketType,
      deliveryStartTime: market.deliveryStartTime,
    };
  }

  async function expectExist(controller: any, markets: any[]) {
    expect(
      await controller.marketsExist(markets.map((market) => market.marketId)),
    ).to.deep.equal(markets.map(() => true));
    expect(await controller.getMarketIds(markets.map(marketKey))).to.deep.equal(
      markets.map((market) => market.marketId),
    );
  }

  /** A market that opens 100 s and closes 200 s after the latest block. */
  async function upcomingMarket(marketId: string, overrides: any = {}) {
    const latest = await time.latest();
    return newMarket(marketId, {
      openingTime: latest + 100,
      closingTime: latest + 200,
      deliveryStartTime: latest + 200,
      deliveryEndTime: latest + 300,
      ...overrides,
    });
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
      await expectExist(controller, [market]);
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
      await expectExist(controller, markets);
    });

    it("Should skip a market that already exists", async function () {
      const { controller, orchestrator } = await loadFixture(
        deployControllerFixture,
      );
      const existing = await upcomingMarket(bytes16Id("market-1"));
      const fresh = newMarket(bytes16Id("market-2"));
      await controller.connect(orchestrator).createMarkets([existing]);
      const freshCreatedAt = await nextBlockTimestamp();

      const createMarkets = controller.connect(orchestrator).createMarkets([
        { ...existing, closingTime: existing.openingTime + 50 },
        fresh,
      ]);
      await expect(createMarkets)
        .to.emit(controller, "NewMarketCreated")
        .withArgs(...eventArgs(fresh, freshCreatedAt));
      const receipt = await (await createMarkets).wait();
      expect(receipt!.logs.length).to.equal(1);
      await expectExist(controller, [existing, fresh]);

      // The stored closing time is still the original one.
      await time.increaseTo(existing.openingTime + 60);
      expect(await controller.isMarketOpen(existing.marketId)).to.be.true;
    });

    it("Should create a market only once if it appears twice in one batch", async function () {
      const { controller, orchestrator } = await loadFixture(
        deployControllerFixture,
      );
      const market = await upcomingMarket(bytes16Id("market-1"));

      const createMarkets = controller.connect(orchestrator).createMarkets([
        market,
        { ...market, closingTime: market.openingTime + 50 },
      ]);
      const receipt = await (await createMarkets).wait();
      expect(receipt!.logs.length).to.equal(1);
      await expectExist(controller, [market]);

      // The first entry won: the market is still open after the second one's closing time.
      await time.increaseTo(market.openingTime + 60);
      expect(await controller.isMarketOpen(market.marketId)).to.be.true;
    });

    it("Should keep the first ID for a duplicate tuple in one batch", async function () {
      const { controller, orchestrator } = await loadFixture(
        deployControllerFixture,
      );
      const first = await upcomingMarket(bytes16Id("first-proposal"));
      const duplicate = {
        ...first,
        marketId: bytes16Id("second-proposal"),
        openingTime: first.openingTime + 10,
        closingTime: first.openingTime + 50,
        deliveryEndTime: first.deliveryEndTime + 100,
        matchingAlgorithm: MATCHING_ALGORITHM_AMM,
      };
      const fresh = newMarket(bytes16Id("fresh-market"));
      const createdAt = await nextBlockTimestamp();
      const createMarkets = controller
        .connect(orchestrator)
        .createMarkets([first, duplicate, fresh]);

      for (const market of [first, fresh]) {
        await expect(createMarkets)
          .to.emit(controller, "NewMarketCreated")
          .withArgs(...eventArgs(market, createdAt));
      }
      expect((await (await createMarkets).wait())!.logs.length).to.equal(2);
      await expectExist(controller, [first, fresh]);
      expect(await controller.getMarketIds([marketKey(duplicate)])).to.deep.equal([
        first.marketId,
      ]);
      expect(await controller.marketsExist([duplicate.marketId])).to.deep.equal([
        false,
      ]);
      await time.increaseTo(first.openingTime);
      expect(await controller.isMarketOpen(first.marketId)).to.be.true;
      await time.increaseTo(first.openingTime + 60);
      expect(await controller.isMarketOpen(first.marketId)).to.be.true;
      expect(await controller.isMarketOpen(duplicate.marketId)).to.be.false;

      // A skipped proposal did not reserve its ID for the winning tuple.
      const reused = { ...duplicate, marketType: MARKET_TYPE_FLEX };
      await controller.connect(orchestrator).createMarkets([reused]);
      await expectExist(controller, [first, reused]);
    });

    it("Should preserve the winner when two orchestrators propose different IDs sequentially", async function () {
      const { controller, orchestrator, user } = await loadFixture(
        deployControllerFixture,
      );
      await controller.grantRole(
        await controller.ORCHESTRATOR_ROLE(),
        user.address,
      );
      const first = await upcomingMarket(bytes16Id("orchestrator-1-proposal"));
      const second = {
        ...first,
        marketId: bytes16Id("orchestrator-2-proposal"),
        closingTime: first.openingTime + 50,
      };

      // Both orchestrators observed an absent tuple before either submitted.
      for (const signer of [orchestrator, user]) {
        expect(
          await controller.connect(signer).getMarketIds([marketKey(first)]),
        ).to.deep.equal([ZERO_BYTES16]);
      }
      await controller.connect(orchestrator).createMarkets([first]);
      await expect(controller.connect(user).createMarkets([second])).not.to.emit(
        controller,
        "NewMarketCreated",
      );
      await expect(controller.connect(user).createMarkets([second])).not.to.emit(
        controller,
        "NewMarketCreated",
      );
      await expectExist(controller, [first]);
      expect(await controller.marketsExist([second.marketId])).to.deep.equal([
        false,
      ]);
      await time.increaseTo(first.openingTime + 60);
      expect(await controller.isMarketOpen(first.marketId)).to.be.true;
    });

    const differentKeys: [string, any][] = [
      ["community", { communityId: bytes16Id("community-2") }],
      ["type", { marketType: MARKET_TYPE_FLEX }],
      ["delivery slot", { deliveryStartTime: 2100 }],
    ];
    for (const [name, overrides] of differentKeys) {
      it(`Should create distinct markets when only the ${name} changes`, async function () {
        const { controller, orchestrator } = await loadFixture(
          deployControllerFixture,
        );
        const markets = [
          newMarket(bytes16Id("market-1")),
          newMarket(bytes16Id("market-2"), overrides),
        ];

        await controller.connect(orchestrator).createMarkets(markets);
        await expectExist(controller, markets);
      });

      for (const sameBatch of [true, false]) {
        it(`Should reject the same ID with a different ${name} in ${sameBatch ? "one batch" : "a later batch"}`, async function () {
          const { controller, orchestrator } = await loadFixture(
            deployControllerFixture,
          );
          const first = newMarket(bytes16Id("market-1"));
          const collision = { ...first, ...overrides };
          if (!sameBatch) {
            await controller.connect(orchestrator).createMarkets([first]);
          }

          await expect(
            controller
              .connect(orchestrator)
              .createMarkets(sameBatch ? [first, collision] : [collision]),
          )
            .to.be.revertedWithCustomError(controller, "InvalidMarket")
            .withArgs(first.marketId);
          expect(
            await controller.getMarketIds([
              marketKey(first),
              marketKey(collision),
            ]),
          ).to.deep.equal([
            sameBatch ? ZERO_BYTES16 : first.marketId,
            ZERO_BYTES16,
          ]);
          expect(await controller.marketsExist([first.marketId])).to.deep.equal([
            !sameBatch,
          ]);
        });
      }
    }

    it("Should reject an ID collision even when the proposed tuple already exists", async function () {
      const { controller, orchestrator } = await loadFixture(
        deployControllerFixture,
      );
      const first = newMarket(bytes16Id("market-1"));
      const second = newMarket(bytes16Id("market-2"), {
        marketType: MARKET_TYPE_FLEX,
      });
      await controller.connect(orchestrator).createMarkets([first, second]);
      const fresh = newMarket(bytes16Id("market-3"), {
        marketType: MARKET_TYPE_SETTLEMENT,
      });

      await expect(
        controller
          .connect(orchestrator)
          .createMarkets([fresh, { ...second, marketId: first.marketId }]),
      )
        .to.be.revertedWithCustomError(controller, "InvalidMarket")
        .withArgs(first.marketId);
      await expectExist(controller, [first, second]);
      expect(await controller.getMarketIds([marketKey(fresh)])).to.deep.equal([
        ZERO_BYTES16,
      ]);
      expect(await controller.marketsExist([fresh.marketId])).to.deep.equal([
        false,
      ]);
    });

    it("Should accept a nonzero bytes16 ID without UUIDv4 constraints", async function () {
      const { controller, orchestrator } = await loadFixture(
        deployControllerFixture,
      );
      const market = newMarket("0x00000000000000000000000000000001");

      await controller.connect(orchestrator).createMarkets([market]);
      await expectExist(controller, [market]);
    });

    it("Should reject a zero ID even for an existing tuple", async function () {
      const { controller, orchestrator } = await loadFixture(
        deployControllerFixture,
      );
      const market = newMarket(bytes16Id("market-1"));
      await controller.connect(orchestrator).createMarkets([market]);

      await expect(
        controller
          .connect(orchestrator)
          .createMarkets([{ ...market, marketId: ZERO_BYTES16 }]),
      )
        .to.be.revertedWithCustomError(controller, "InvalidMarket")
        .withArgs(ZERO_BYTES16);
      await expectExist(controller, [market]);
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
            newMarket(invalidMarketId, {
              marketType: MARKET_TYPE_FLEX,
              closingTime: 0,
            }),
          ]),
      )
        .to.be.revertedWithCustomError(controller, "InvalidMarket")
        .withArgs(invalidMarketId);
      expect(await controller.marketsExist([validMarketId])).to.deep.equal([
        false,
      ]);
      expect(
        await controller.getMarketIds([marketKey(newMarket(validMarketId))]),
      ).to.deep.equal([ZERO_BYTES16]);
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

  describe("getMarketIds", function () {
    it("Should return IDs and zeros in request order, including repeated keys", async function () {
      const { controller, orchestrator, user } = await loadFixture(
        deployControllerFixture,
      );
      const first = newMarket(bytes16Id("market-1"));
      const second = newMarket(bytes16Id("market-2"), {
        marketType: MARKET_TYPE_SETTLEMENT,
        deliveryStartTime: (1n << 64n) - 2n,
        deliveryEndTime: (1n << 64n) - 1n,
      });
      await controller.connect(orchestrator).createMarkets([first, second]);
      const unknown = {
        ...marketKey(first),
        communityId: bytes16Id("unknown-community"),
      };

      expect(
        await controller.connect(user).getMarketIds([
          unknown,
          marketKey(second),
          marketKey(first),
          unknown,
          marketKey(second),
        ]),
      ).to.deep.equal([
        ZERO_BYTES16,
        second.marketId,
        first.marketId,
        ZERO_BYTES16,
        second.marketId,
      ]);
    });

    it("Should return zero for unknown communities, types and delivery slots", async function () {
      const { controller, orchestrator } = await loadFixture(
        deployControllerFixture,
      );
      const market = newMarket(bytes16Id("market-1"));
      await controller.connect(orchestrator).createMarkets([market]);
      const key = marketKey(market);

      expect(
        await controller.getMarketIds([
          { ...key, communityId: bytes16Id("unknown-community") },
          { ...key, marketType: MARKET_TYPE_FLEX },
          { ...key, deliveryStartTime: market.deliveryStartTime + 1 },
          { ...key, communityId: ZERO_BYTES16 },
        ]),
      ).to.deep.equal([ZERO_BYTES16, ZERO_BYTES16, ZERO_BYTES16, ZERO_BYTES16]);
    });

    it("Should return an empty list for no keys", async function () {
      const { controller } = await loadFixture(deployControllerFixture);

      expect(await controller.getMarketIds([])).to.deep.equal([]);
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

  describe("marketsExist", function () {
    it("Should report existence per market in request order", async function () {
      const { controller, orchestrator } = await loadFixture(
        deployControllerFixture,
      );
      const created = [bytes16Id("market-1"), bytes16Id("market-2")];
      const unknown = bytes16Id("unknown-market");
      await controller
        .connect(orchestrator)
        .createMarkets(
          created.map((marketId, marketType) =>
            newMarket(marketId, { marketType }),
          ),
        );

      expect(
        await controller.marketsExist([unknown, created[1], unknown, created[0]]),
      ).to.deep.equal([false, true, false, true]);
    });

    it("Should return an empty list for no markets", async function () {
      const { controller } = await loadFixture(deployControllerFixture);

      expect(await controller.marketsExist([])).to.deep.equal([]);
    });
  });
});
