import { loadFixture } from "@nomicfoundation/hardhat-toolbox/network-helpers";
import { expect } from "chai";
import { ethers } from "hardhat";
import { hashOrder, ORDER_TYPE_BID, ORDER_TYPE_ASK } from "./utils";

describe("TradeSettlement", function () {
  async function deploySettlementFixture() {
    const [admin, buyer, seller, operator] = await ethers.getSigners();

    // 1. Deploy dependencies
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

    const TradeSettlement = await ethers.getContractFactory("TradeSettlement");
    const settlement = await TradeSettlement.deploy(
      await registry.getAddress(),
      await vault.getAddress(),
    );

    // 2. Setup Permissions
    const ORCHESTRATOR_ROLE = await controller.ORCHESTRATOR_ROLE();
    await controller.grantRole(ORCHESTRATOR_ROLE, admin.address);

    const SETTLEMENT_ROLE_VAULT = await vault.SETTLEMENT_ROLE();
    await vault.grantRole(SETTLEMENT_ROLE_VAULT, await settlement.getAddress());

    const SETTLEMENT_ROLE_REGISTRY = await registry.SETTLEMENT_ROLE();
    await registry.grantRole(
      SETTLEMENT_ROLE_REGISTRY,
      await settlement.getAddress(),
    );

    const OPERATOR_ROLE = await settlement.OPERATOR_ROLE();
    await settlement.grantRole(OPERATOR_ROLE, operator.address);

    // 3. Setup Market
    const marketId = ethers.keccak256(ethers.toUtf8Bytes("market-1"));
    await controller.setMarketStatus(marketId, true);

    // 4. Setup Balances
    const depositAmount = 10000;
    await vault.connect(buyer).deposit({ value: depositAmount });

    // 5. Define Orders
    const bid = {
      owner: buyer.address,
      nonce: 1,
      areaUuid: ethers.keccak256(ethers.toUtf8Bytes("area-b")),
      marketId: marketId,
      timeSlot: 1000,
      creationTime: 900,
      energy: 100,
      energyRate: 50,
      isBid: ORDER_TYPE_BID,
    };

    const ask = {
      owner: seller.address,
      nonce: 1,
      areaUuid: ethers.keccak256(ethers.toUtf8Bytes("area-s")),
      marketId: marketId,
      timeSlot: 1000,
      creationTime: 900,
      energy: 100,
      energyRate: 40, // Ask <= Bid
      isBid: ORDER_TYPE_ASK,
    };

    return {
      settlement,
      registry,
      vault,
      buyer,
      seller,
      operator,
      bid,
      ask,
      depositAmount,
    };
  }

  it("Should settle a valid trade", async function () {
    const { settlement, registry, vault, buyer, seller, operator, bid, ask } =
      await loadFixture(deploySettlementFixture);

    // Place orders first
    await registry.connect(buyer).placeOrder(bid);
    await registry.connect(seller).placeOrder(ask);

    const matchData = {
      bid: bid,
      ask: ask,
      selectedEnergy: 100,
      clearingPrice: 45, // Between 40 and 50
    };

    const totalCost = matchData.selectedEnergy * matchData.clearingPrice;

    // Check balances before
    expect(await vault.balances(buyer.address)).to.equal(10000);
    expect(await vault.balances(seller.address)).to.equal(0);

    // Execute Settlement
    await expect(settlement.connect(operator).settleBatch([matchData])).to.emit(
      settlement,
      "TradeSettled",
    );

    // Check Balances after
    expect(await vault.balances(buyer.address)).to.equal(10000 - totalCost);
    expect(await vault.balances(seller.address)).to.equal(totalCost);

    // Check Order Status
    const bidHash = await hashOrder(bid);
    expect(await registry.getStatus(bidHash)).to.equal(2); // Executed
  });

  it("Should fail if orders are not open", async function () {
    const { settlement, operator, bid, ask } = await loadFixture(
      deploySettlementFixture,
    );
    // Not placing orders in registry

    const matchData = { bid, ask, selectedEnergy: 100, clearingPrice: 45 };

    await expect(
      settlement.connect(operator).settleBatch([matchData]),
    ).to.be.revertedWithCustomError(settlement, "OrderNotOpen");
  });

  it("Should fail on price mismatch (Ask > Bid)", async function () {
    const { settlement, registry, buyer, seller, operator, bid, ask } =
      await loadFixture(deploySettlementFixture);

    const highAsk = { ...ask, energyRate: 60 }; // 60 > 50 (Bid)
    await registry.connect(buyer).placeOrder(bid);
    await registry.connect(seller).placeOrder(highAsk);

    const matchData = {
      bid,
      ask: highAsk,
      selectedEnergy: 100,
      clearingPrice: 55,
    };

    // Contract validates: bid.price >= clearing >= ask.price
    // Here: 50 < 55 (Fail bid check) or 60 > 55 (Fail ask check)
    await expect(
      settlement.connect(operator).settleBatch([matchData]),
    ).to.be.revertedWithCustomError(settlement, "PriceMismatch");
  });
});
