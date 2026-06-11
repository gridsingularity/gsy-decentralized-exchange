import { loadFixture } from "@nomicfoundation/hardhat-toolbox/network-helpers";
import { expect } from "chai";
import { ethers } from "hardhat";
import { bytes16Id, ORDER_TYPE_BID, ORDER_TYPE_ASK, ZERO_BYTES16 } from "./utils";

describe("TradeSettlement", function () {
  async function deploySettlementFixture() {
    const [admin, buyer, seller, operator, executionEngine] =
      await ethers.getSigners();

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
    const EXECUTION_ENGINE_ROLE = await settlement.EXECUTION_ENGINE_ROLE();
    await settlement.grantRole(EXECUTION_ENGINE_ROLE, executionEngine.address);

    const buyerActorId = bytes16Id("actor:buyer");
    const sellerActorId = bytes16Id("actor:seller");
    const marketId = bytes16Id("market-1");
    await controller.setMarketStatus(marketId, true);

    await vault.registerActor(buyerActorId, buyer.address);
    await vault.registerActor(sellerActorId, seller.address);

    const depositAmount = 10000;
    await vault.connect(buyer).deposit(buyerActorId, { value: depositAmount });

    const bid = {
      orderId: bytes16Id("bid-1"),
      createdBy: buyerActorId,
      marketId: marketId,
      timeSlot: 1000,
      creationTime: 900,
      energy: 100,
      energyRate: 50,
      isBid: ORDER_TYPE_BID,
    };

    const ask = {
      orderId: bytes16Id("ask-1"),
      createdBy: sellerActorId,
      marketId: marketId,
      timeSlot: 1000,
      creationTime: 900,
      energy: 100,
      energyRate: 40,
      isBid: ORDER_TYPE_ASK,
    };

    return {
      settlement,
      registry,
      vault,
      buyer,
      seller,
      operator,
      executionEngine,
      bid,
      ask,
      buyerActorId,
      sellerActorId,
      marketId,
      depositAmount,
    };
  }

  it("Should settle a valid trade", async function () {
    const { settlement, registry, vault, buyer, seller, operator, bid, ask, buyerActorId, sellerActorId } =
      await loadFixture(deploySettlementFixture);

    await registry.connect(buyer).placeOrder(bid);
    await registry.connect(seller).placeOrder(ask);

    const matchData = {
      tradeId: bytes16Id("trade-1"),
      bid,
      ask,
      selectedEnergy: 100,
      clearingPrice: 45,
    };

    const totalCost = matchData.selectedEnergy * matchData.clearingPrice;

    expect(await vault.balances(buyerActorId)).to.equal(10000);
    expect(await vault.balances(sellerActorId)).to.equal(0);

    await expect(settlement.connect(operator).settleBatch([matchData])).to.emit(
      settlement,
      "TradeSettled",
    );

    expect(await vault.balances(buyerActorId)).to.equal(10000 - totalCost);
    expect(await vault.balances(sellerActorId)).to.equal(totalCost);

    expect(await registry.getStatus(bid.orderId)).to.equal(2); // Executed
  });

  it("Should submit penalties from the execution engine", async function () {
    const { settlement, buyerActorId, executionEngine, marketId } = await loadFixture(
      deploySettlementFixture,
    );

    const tradeId1 = bytes16Id("trade-1");
    const tradeId2 = bytes16Id("trade-2");

    const penalties = [
      {
        penalizedActorId: buyerActorId,
        marketId,
        tradeId: tradeId1,
        penaltyEnergy: 30,
      },
      {
        penalizedActorId: buyerActorId,
        marketId,
        tradeId: tradeId2,
        penaltyEnergy: 70,
      },
    ];

    await expect(settlement.connect(executionEngine).submitPenalties(penalties))
      .to.emit(settlement, "PenaltyRecorded")
      .withArgs(buyerActorId, marketId, tradeId1, 30)
      .and.to.emit(settlement, "PenaltiesSubmitted")
      .withArgs(2);

    expect(await settlement.penaltyEnergyByTrade(tradeId1)).to.equal(30);
    expect(await settlement.penaltyEnergyByTrade(tradeId2)).to.equal(70);
    expect(await settlement.penaltyEnergyByActor(buyerActorId)).to.equal(100);
  });

  it("Should fail penalties submission from unauthorized account", async function () {
    const { settlement, buyerActorId, operator, marketId } = await loadFixture(
      deploySettlementFixture,
    );

    const penalties = [
      {
        penalizedActorId: buyerActorId,
        marketId,
        tradeId: bytes16Id("trade-1"),
        penaltyEnergy: 10,
      },
    ];

    await expect(
      settlement.connect(operator).submitPenalties(penalties),
    ).to.be.revertedWithCustomError(
      settlement,
      "AccessControlUnauthorizedAccount",
    );
  });

  it("Should fail penalties submission with invalid payload", async function () {
    const { settlement, executionEngine, marketId } = await loadFixture(
      deploySettlementFixture,
    );

    const penalties = [
      {
        penalizedActorId: ZERO_BYTES16,
        marketId,
        tradeId: bytes16Id("trade-1"),
        penaltyEnergy: 10,
      },
    ];

    await expect(
      settlement.connect(executionEngine).submitPenalties(penalties),
    ).to.be.revertedWithCustomError(settlement, "InvalidPenalty");
  });

  it("Should fail if orders are not open", async function () {
    const { settlement, operator, bid, ask } = await loadFixture(
      deploySettlementFixture,
    );

    const matchData = { tradeId: bytes16Id("trade-1"), bid, ask, selectedEnergy: 100, clearingPrice: 45 };

    await expect(
      settlement.connect(operator).settleBatch([matchData]),
    ).to.be.revertedWithCustomError(settlement, "OrderNotOpen");
  });

  it("Should fail if match order details do not match stored orders", async function () {
    const { settlement, registry, buyer, seller, operator, bid, ask } =
      await loadFixture(deploySettlementFixture);

    await registry.connect(buyer).placeOrder(bid);
    await registry.connect(seller).placeOrder(ask);

    const tamperedBid = { ...bid, energyRate: bid.energyRate + 1 };
    const matchData = {
      tradeId: bytes16Id("trade-1"),
      bid: tamperedBid,
      ask,
      selectedEnergy: 100,
      clearingPrice: 45,
    };

    await expect(
      settlement.connect(operator).settleBatch([matchData]),
    ).to.be.revertedWithCustomError(settlement, "InvalidOrderParams");
  });

  it("Should fail on price mismatch (Ask > Bid)", async function () {
    const { settlement, registry, buyer, seller, operator, bid, ask } =
      await loadFixture(deploySettlementFixture);

    const highAsk = { ...ask, energyRate: 60 };
    await registry.connect(buyer).placeOrder(bid);
    await registry.connect(seller).placeOrder(highAsk);

    const matchData = {
      tradeId: bytes16Id("trade-1"),
      bid,
      ask: highAsk,
      selectedEnergy: 100,
      clearingPrice: 55,
    };

    await expect(
      settlement.connect(operator).settleBatch([matchData]),
    ).to.be.revertedWithCustomError(settlement, "PriceMismatch");
  });
});
