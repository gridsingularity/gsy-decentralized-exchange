import { loadFixture } from "@nomicfoundation/hardhat-toolbox/network-helpers";
import { expect } from "chai";
import { ethers } from "hardhat";
import { bytes16Id, deployUpgradeableContract } from "./utils";

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

  it("Should allow orchestrator to open/close markets", async function () {
    const { controller, orchestrator } = await loadFixture(
      deployControllerFixture,
    );
    const marketId = bytes16Id("market-1");

    await expect(
      controller.connect(orchestrator).setMarketStatus(marketId, true),
    )
      .to.emit(controller, "MarketStatusUpdated")
      .withArgs(marketId, true);

    expect(await controller.isMarketOpen(marketId)).to.be.true;
  });

  it("Should prevent unauthorized users from changing status", async function () {
    const { controller, user } = await loadFixture(deployControllerFixture);
    const marketId = bytes16Id("market-1");

    await expect(
      controller.connect(user).setMarketStatus(marketId, true),
    ).to.be.revertedWithCustomError(
      controller,
      "AccessControlUnauthorizedAccount",
    );
  });

  it("Should allow orchestrator to update multiple markets", async function () {
    const { controller, orchestrator } = await loadFixture(
      deployControllerFixture,
    );
    const marketIds = [
      bytes16Id("market-1"),
      bytes16Id("market-2"),
      bytes16Id("market-3"),
    ];

    const openMarkets = controller
      .connect(orchestrator)
      .setMarketStatuses(marketIds, true);
    for (const marketId of marketIds) {
      await expect(openMarkets)
        .to.emit(controller, "MarketStatusUpdated")
        .withArgs(marketId, true);
    }
    for (const marketId of marketIds) {
      expect(await controller.isMarketOpen(marketId)).to.be.true;
    }

    await controller.connect(orchestrator).setMarketStatuses(marketIds, false);
    for (const marketId of marketIds) {
      expect(await controller.isMarketOpen(marketId)).to.be.false;
    }
  });

  it("Should prevent unauthorized users from updating multiple markets", async function () {
    const { controller, user } = await loadFixture(deployControllerFixture);

    await expect(
      controller
        .connect(user)
        .setMarketStatuses([bytes16Id("market-1")], true),
    ).to.be.revertedWithCustomError(
      controller,
      "AccessControlUnauthorizedAccount",
    );
  });

  it("Should allow an empty market batch", async function () {
    const { controller, orchestrator } = await loadFixture(
      deployControllerFixture,
    );

    await expect(
      controller.connect(orchestrator).setMarketStatuses([], true),
    ).not.to.emit(controller, "MarketStatusUpdated");
  });
});
