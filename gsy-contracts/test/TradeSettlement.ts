import { loadFixture } from "@nomicfoundation/hardhat-toolbox/network-helpers";
import { expect } from "chai";
import { ethers } from "hardhat";
import { hashOrder, ORDER_TYPE_BID, ORDER_TYPE_ASK } from "./utils";

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

    const marketId = ethers.keccak256(ethers.toUtf8Bytes("market-1"));
    await controller.setMarketStatus(marketId, true);

    const depositAmount = 10000;
    await vault.connect(buyer).deposit({ value: depositAmount });

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
      executionEngine,
      bid,
      ask,
      depositAmount,
    };
  }

  it("Should settle a valid trade", async function () {
    const { settlement, registry, vault, buyer, seller, operator, bid, ask } =
      await loadFixture(deploySettlementFixture);

    await registry.connect(buyer).placeOrder(bid);
    await registry.connect(seller).placeOrder(ask);

    const matchData = {
      bid: bid,
      ask: ask,
      selectedEnergy: 100,
      clearingPrice: 45, // Between 40 and 50
    };

    const totalCost = matchData.selectedEnergy * matchData.clearingPrice;

    expect(await vault.balances(buyer.address)).to.equal(10000);
    expect(await vault.balances(seller.address)).to.equal(0);

    await expect(settlement.connect(operator).settleBatch([matchData])).to.emit(
      settlement,
      "TradeSettled",
    );

    expect(await vault.balances(buyer.address)).to.equal(10000 - totalCost);
    expect(await vault.balances(seller.address)).to.equal(totalCost);

    const bidHash = await hashOrder(bid);
    expect(await registry.getStatus(bidHash)).to.equal(2); // Executed
  });

  it("Should submit penalties from the execution engine", async function () {
    const { settlement, buyer, executionEngine } = await loadFixture(
      deploySettlementFixture,
    );

    const marketId = ethers.keccak256(ethers.toUtf8Bytes("market-penalty"));
    const tradeId1 = ethers.keccak256(ethers.toUtf8Bytes("trade-1"));
    const tradeId2 = ethers.keccak256(ethers.toUtf8Bytes("trade-2"));

    const penalties = [
      {
        penalizedAccount: buyer.address,
        marketId,
        tradeId: tradeId1,
        penaltyEnergy: 30,
      },
      {
        penalizedAccount: buyer.address,
        marketId,
        tradeId: tradeId2,
        penaltyEnergy: 70,
      },
    ];

    await expect(settlement.connect(executionEngine).submitPenalties(penalties))
      .to.emit(settlement, "PenaltyRecorded")
      .withArgs(buyer.address, marketId, tradeId1, 30)
      .and.to.emit(settlement, "PenaltiesSubmitted")
      .withArgs(2);

    expect(await settlement.penaltyEnergyByTrade(tradeId1)).to.equal(30);
    expect(await settlement.penaltyEnergyByTrade(tradeId2)).to.equal(70);
    expect(await settlement.penaltyEnergyByAccount(buyer.address)).to.equal(
      100,
    );
  });

  it("Should fail penalties submission from unauthorized account", async function () {
    const { settlement, buyer, operator } = await loadFixture(
      deploySettlementFixture,
    );

    const marketId = ethers.keccak256(ethers.toUtf8Bytes("market-penalty"));
    const tradeId = ethers.keccak256(ethers.toUtf8Bytes("trade-1"));
    const penalties = [
      {
        penalizedAccount: buyer.address,
        marketId,
        tradeId,
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
    const { settlement, executionEngine } = await loadFixture(
      deploySettlementFixture,
    );

    const penalties = [
      {
        penalizedAccount: ethers.ZeroAddress,
        marketId: ethers.keccak256(ethers.toUtf8Bytes("market-penalty")),
        tradeId: ethers.keccak256(ethers.toUtf8Bytes("trade-1")),
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
