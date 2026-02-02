import { loadFixture } from "@nomicfoundation/hardhat-toolbox/network-helpers";
import { expect } from "chai";
import { ethers } from "hardhat";
import { hashOrder, ORDER_TYPE_BID } from "./utils";

describe("OrderRegistry", function () {
  async function deployRegistryFixture() {
    const [admin, user, proxy, other] = await ethers.getSigners();

    // Deploy Dependencies
    const MarketController =
      await ethers.getContractFactory("MarketController");
    const controller = await MarketController.deploy();
    const GsyVault = await ethers.getContractFactory("GsyVault");
    const vault = await GsyVault.deploy();

    // Deploy Registry
    const OrderRegistry = await ethers.getContractFactory("OrderRegistry");
    const registry = await OrderRegistry.deploy(
      await controller.getAddress(),
      await vault.getAddress(),
    );

    // Setup Market
    const marketId = ethers.keccak256(ethers.toUtf8Bytes("market-1"));
    const ORCHESTRATOR_ROLE = await controller.ORCHESTRATOR_ROLE();
    await controller.grantRole(ORCHESTRATOR_ROLE, admin.address);
    await controller.setMarketStatus(marketId, true);

    // Setup Proxy
    await vault.connect(user).setProxy(proxy.address, true);

    const baseOrder = {
      owner: user.address,
      nonce: 1,
      areaUuid: ethers.keccak256(ethers.toUtf8Bytes("area-1")),
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
    const expectedHash = await hashOrder(baseOrder);

    await expect(registry.connect(user).placeOrder(baseOrder))
      .to.emit(registry, "OrderPlaced")
      .withArgs(
        expectedHash,
        user.address,
        baseOrder.marketId,
        baseOrder.areaUuid,
        baseOrder.nonce,
        baseOrder.timeSlot,
        baseOrder.creationTime,
        baseOrder.energy,
        baseOrder.energyRate,
        baseOrder.isBid,
      );

    expect(await registry.getStatus(expectedHash)).to.equal(1); // Open
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
    const expectedHash = await hashOrder(baseOrder);

    await expect(registry.connect(user).cancelOrder(baseOrder))
      .to.emit(registry, "OrderCancelled")
      .withArgs(expectedHash);

    expect(await registry.getStatus(expectedHash)).to.equal(3); // Cancelled
  });
});
