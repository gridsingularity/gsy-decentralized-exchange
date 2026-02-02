import { loadFixture } from "@nomicfoundation/hardhat-toolbox/network-helpers";
import { expect } from "chai";
import { ethers } from "hardhat";

describe("MarketController", function () {
  async function deployControllerFixture() {
    const [admin, orchestrator, user] = await ethers.getSigners();
    const MarketController =
      await ethers.getContractFactory("MarketController");
    const controller = await MarketController.deploy();

    const ORCHESTRATOR_ROLE = await controller.ORCHESTRATOR_ROLE();
    await controller.grantRole(ORCHESTRATOR_ROLE, orchestrator.address);

    return { controller, admin, orchestrator, user };
  }

  it("Should allow orchestrator to open/close markets", async function () {
    const { controller, orchestrator } = await loadFixture(
      deployControllerFixture,
    );
    const marketId = ethers.keccak256(ethers.toUtf8Bytes("market-1"));

    await expect(
      controller.connect(orchestrator).setMarketStatus(marketId, true),
    )
      .to.emit(controller, "MarketStatusUpdated")
      .withArgs(marketId, true);

    expect(await controller.isMarketOpen(marketId)).to.be.true;
  });

  it("Should prevent unauthorized users from changing status", async function () {
    const { controller, user } = await loadFixture(deployControllerFixture);
    const marketId = ethers.keccak256(ethers.toUtf8Bytes("market-1"));

    await expect(
      controller.connect(user).setMarketStatus(marketId, true),
    ).to.be.revertedWithCustomError(
      controller,
      "AccessControlUnauthorizedAccount",
    );
  });
});
