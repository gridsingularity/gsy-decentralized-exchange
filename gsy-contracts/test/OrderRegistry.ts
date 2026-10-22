import {
  loadFixture,
  time,
} from "@nomicfoundation/hardhat-toolbox/network-helpers";
import { expect } from "chai";
import { ethers } from "hardhat";
import {
  bytes16Id,
  createOpenMarket,
  deployUpgradeableContract,
  ENERGY_TYPE_GREEN,
  ENERGY_TYPE_NONE,
  newMarket,
  ORDER_TYPE_BID,
  OPEN_MARKET_DURATION,
  ZERO_BYTES16,
} from "./utils";

describe("OrderRegistry", function () {
  async function deployRegistryFixture() {
    const [admin, user, proxy, other] = await ethers.getSigners();

    const controller = await deployUpgradeableContract("MarketController", [
      admin.address,
    ]);
    const actorRegistry = await deployUpgradeableContract("ActorRegistry", [
      admin.address,
    ]);
    const registry = await deployUpgradeableContract("OrderRegistry", [
      admin.address,
      await controller.getAddress(),
      await actorRegistry.getAddress(),
    ]);

    const actorId = bytes16Id("actor:user");
    const marketId = bytes16Id("market-1");
    const ORCHESTRATOR_ROLE = await controller.ORCHESTRATOR_ROLE();
    await controller.grantRole(ORCHESTRATOR_ROLE, admin.address);
    await createOpenMarket(controller, marketId);

    await actorRegistry.registerActor(actorId, user.address);
    await actorRegistry.connect(user).setProxy(actorId, proxy.address, true);

    const baseOrder = {
      orderId: bytes16Id("order-1"),
      createdBy: actorId,
      marketId: marketId,
      timeSlot: 1000,
      creationTime: 900,
      energy: 100,
      energyRate: 50,
      energySourcePreference: ENERGY_TYPE_GREEN,
      energyType: ENERGY_TYPE_NONE,
      isBid: ORDER_TYPE_BID,
      preferredTradingPartner: bytes16Id("actor:preferred-partner"),
      preferredEnergyRate: 45,
    };

    return {
      registry,
      controller,
      user,
      proxy,
      other,
      baseOrder,
      marketId,
    };
  }

  it("Should place order successfully", async function () {
    const { registry, user, baseOrder } = await loadFixture(
      deployRegistryFixture,
    );

    await expect(registry.connect(user).placeOrder(baseOrder))
      .to.emit(registry, "OrderPlaced")
      .withArgs(
        baseOrder.orderId,
        baseOrder.createdBy,
        baseOrder.marketId,
        baseOrder.timeSlot,
        baseOrder.creationTime,
        baseOrder.energy,
        baseOrder.energyRate,
        baseOrder.energySourcePreference,
        baseOrder.energyType,
        baseOrder.isBid,
        baseOrder.preferredTradingPartner,
        baseOrder.preferredEnergyRate,
      );

    expect(await registry.getStatus(baseOrder.orderId)).to.equal(1); // Open
    const storedOrder = await registry.getOrder(baseOrder.orderId);
    expect(storedOrder.preferredTradingPartner).to.equal(
      baseOrder.preferredTradingPartner,
    );
    expect(storedOrder.preferredEnergyRate).to.equal(
      baseOrder.preferredEnergyRate,
    );
  });

  it("Should revert if market is closed", async function () {
    const { registry, controller, user, baseOrder, marketId } =
      await loadFixture(deployRegistryFixture);
    await time.increase(OPEN_MARKET_DURATION);

    await expect(
      registry.connect(user).placeOrder(baseOrder),
    ).to.be.revertedWithCustomError(registry, "MarketClosed");
  });

  it("Should revert if market has not opened yet", async function () {
    const { registry, controller, user, baseOrder } =
      await loadFixture(deployRegistryFixture);
    const futureMarketId = bytes16Id("market-future");
    const latest = await time.latest();
    await controller.createMarkets([
      newMarket(futureMarketId, {
        openingTime: latest + 600,
        closingTime: latest + 1200,
      }),
    ]);

    await expect(
      registry
        .connect(user)
        .placeOrder({ ...baseOrder, marketId: futureMarketId }),
    ).to.be.revertedWithCustomError(registry, "MarketClosed");
  });

  it("Should revert if market does not exist", async function () {
    const { registry, user, baseOrder } = await loadFixture(
      deployRegistryFixture,
    );

    await expect(
      registry
        .connect(user)
        .placeOrder({ ...baseOrder, marketId: bytes16Id("unknown-market") }),
    ).to.be.revertedWithCustomError(registry, "MarketClosed");
  });

  it("Should allow proxy to place order", async function () {
    const { registry, proxy, baseOrder } = await loadFixture(
      deployRegistryFixture,
    );

    await expect(registry.connect(proxy).placeOrder(baseOrder)).to.emit(
      registry,
      "OrderPlaced",
    );
  });

  it("Should expire only open orders in the closed market, not at delivery time", async function () {
    const { registry, controller, user, baseOrder, marketId } =
      await loadFixture(deployRegistryFixture);
    const otherOrder = { ...baseOrder, orderId: bytes16Id("other-order"), marketId: bytes16Id("other-market") };
    await controller.createMarkets([newMarket(otherOrder.marketId, { openingTime: await time.latest(), closingTime: (await time.latest()) + 2 * OPEN_MARKET_DURATION })]);
    await registry.connect(user).placeOrder(baseOrder);
    await registry.connect(user).placeOrder(otherOrder);
    expect((await ethers.provider.getBlock("latest"))!.timestamp).to.be.greaterThan(baseOrder.timeSlot);
    expect(await registry.getStatus(baseOrder.orderId)).to.equal(1); // Open

    await time.increase(OPEN_MARKET_DURATION);
    expect(await registry.getStatus(baseOrder.orderId)).to.equal(4); // Expired
    expect(await registry.orderStatus(baseOrder.orderId)).to.equal(4);
    expect(await registry.getStatus(otherOrder.orderId)).to.equal(1);
    expect(await registry.getStatus(bytes16Id("unknown"))).to.equal(0);
    await expect(registry.connect(user).cancelOrder(baseOrder))
      .to.be.revertedWithCustomError(registry, "OrderNotOpen");
  });

  it("Should not revive expired orders by recreating the same market", async function () {
    const { registry, controller, user, baseOrder, marketId } =
      await loadFixture(deployRegistryFixture);
    await registry.connect(user).placeOrder(baseOrder);
    await time.increase(OPEN_MARKET_DURATION);
    await createOpenMarket(controller, marketId);
    expect(await controller.isMarketOpen(marketId)).to.equal(false);
    expect(await registry.getStatus(baseOrder.orderId)).to.equal(4);
    expect(await registry.orderStatus(baseOrder.orderId)).to.equal(4);

    await registry.grantRole(await registry.SETTLEMENT_ROLE(), user.address);
    await expect(registry.connect(user).settleOrder(baseOrder.orderId, baseOrder.energy, ZERO_BYTES16))
      .to.be.revertedWithCustomError(registry, "OrderNotOpen");
    const newOrder = { ...baseOrder, orderId: bytes16Id("new-order") };
    await expect(registry.connect(user).placeOrder(newOrder))
      .to.be.revertedWithCustomError(registry, "MarketClosed");
    const newMarketId = bytes16Id("new-market");
    await createOpenMarket(controller, newMarketId);
    await registry.connect(user).placeOrder({ ...newOrder, marketId: newMarketId });
    expect(await registry.getStatus(newOrder.orderId)).to.equal(1);
  });

  for (const isBid of [true, false]) {
    it(`Should expire a residual ${isBid ? "bid" : "offer"} without changing consumed or cancelled orders`, async function () {
      const { registry, controller, user, baseOrder, marketId } =
        await loadFixture(deployRegistryFixture);
      await registry.grantRole(await registry.SETTLEMENT_ROLE(), user.address);
      const partial = { ...baseOrder, isBid };
      const filled = { ...baseOrder, orderId: bytes16Id("filled") };
      const cancelled = { ...baseOrder, orderId: bytes16Id("cancelled") };
      for (const order of [partial, filled, cancelled]) {
        await registry.connect(user).placeOrder(order);
      }
      const residualId = bytes16Id("residual");
      await registry.connect(user).settleOrder(partial.orderId, 40, residualId);
      await registry.connect(user).settleOrder(filled.orderId, 100, ZERO_BYTES16);
      await registry.connect(user).cancelOrder(cancelled);
      await time.increase(OPEN_MARKET_DURATION);
      expect(await registry.getStatus(partial.orderId)).to.equal(5); // PartiallyFilled
      expect(await registry.getStatus(filled.orderId)).to.equal(6); // Filled
      expect(await registry.getStatus(cancelled.orderId)).to.equal(3); // Cancelled
      expect(await registry.getStatus(residualId)).to.equal(4);
    });
  }

  it("Should revert unauthorized proxy", async function () {
    const { registry, other, baseOrder } = await loadFixture(
      deployRegistryFixture,
    );
    await expect(
      registry.connect(other).placeOrder(baseOrder),
    ).to.be.revertedWithCustomError(registry, "Unauthorized");
  });

  it("Should reject unsupported energy metadata values", async function () {
    const { registry, user, baseOrder } = await loadFixture(
      deployRegistryFixture,
    );

    await expect(
      registry
        .connect(user)
        .placeOrder({ ...baseOrder, energySourcePreference: 99 }),
    ).to.be.revertedWithCustomError(registry, "InvalidOrderParams");

    await expect(
      registry.connect(user).placeOrder({ ...baseOrder, energyType: 99 }),
    ).to.be.revertedWithCustomError(registry, "InvalidOrderParams");
  });

  it("Should cancel an open order", async function () {
    const { registry, user, baseOrder } = await loadFixture(
      deployRegistryFixture,
    );
    await registry.connect(user).placeOrder(baseOrder);

    await expect(registry.connect(user).cancelOrder(baseOrder))
      .to.emit(registry, "OrderCancelled")
      .withArgs(baseOrder.orderId);

    expect(await registry.getStatus(baseOrder.orderId)).to.equal(3); // Cancelled
  });

  it("Should reject cancellation with mismatched actor details", async function () {
    const { registry, user, other, baseOrder } = await loadFixture(
      deployRegistryFixture,
    );
    await registry.connect(user).placeOrder(baseOrder);

    const tamperedOrder = {
      ...baseOrder,
      createdBy: bytes16Id("actor:other"),
    };

    await expect(
      registry.connect(other).cancelOrder(tamperedOrder),
    ).to.be.revertedWithCustomError(registry, "Unauthorized");
  });
});
