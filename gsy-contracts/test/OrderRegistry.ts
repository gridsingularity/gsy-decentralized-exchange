import { loadFixture } from "@nomicfoundation/hardhat-toolbox/network-helpers";
import { expect } from "chai";
import { ethers } from "hardhat";
import { bytes16Id, ORDER_TYPE_BID } from "./utils";

describe("OrderRegistry", function () {
  async function deployRegistryFixture() {
    const [admin, user, proxy, other] = await ethers.getSigners();

    const MarketController =
      await ethers.getContractFactory("MarketController");
    const controller = await MarketController.deploy();
    const GsyVault = await ethers.getContractFactory("GsyVault");
    const vault = await GsyVault.deploy();

    const OrderRegistry = await ethers.getContractFactory("OrderRegistry");
    const registry = await OrderRegistry.deploy(
      await controller.getAddress(),
      await vault.getAddress(),
    );

    const actorId = bytes16Id("actor:user");
    const marketId = bytes16Id("market-1");
    const ORCHESTRATOR_ROLE = await controller.ORCHESTRATOR_ROLE();
    await controller.grantRole(ORCHESTRATOR_ROLE, admin.address);
    await controller.setMarketStatus(marketId, true);

    await vault.registerActor(actorId, user.address);
    await vault.connect(user).setProxy(actorId, proxy.address, true);

    const baseOrder = {
      orderId: bytes16Id("order-1"),
      createdBy: actorId,
      marketId: marketId,
      timeSlot: 1000,
      creationTime: 900,
      energy: 100,
      energyRate: 50,
      isBid: ORDER_TYPE_BID,
    };

    return {
      registry,
      controller,
      vault,
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
        baseOrder.isBid,
      );

    expect(await registry.getStatus(baseOrder.orderId)).to.equal(1); // Open
  });

  it("Should revert if market is closed", async function () {
    const { registry, controller, user, baseOrder, marketId } =
      await loadFixture(deployRegistryFixture);
    await controller.setMarketStatus(marketId, false);

    await expect(
      registry.connect(user).placeOrder(baseOrder),
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

  it("Should revert unauthorized proxy", async function () {
    const { registry, other, baseOrder } = await loadFixture(
      deployRegistryFixture,
    );
    await expect(
      registry.connect(other).placeOrder(baseOrder),
    ).to.be.revertedWithCustomError(registry, "Unauthorized");
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
