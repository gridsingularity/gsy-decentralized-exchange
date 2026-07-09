import { loadFixture } from "@nomicfoundation/hardhat-toolbox/network-helpers";
import { expect } from "chai";
import { ethers } from "hardhat";
import {
  bytes16Id,
  deployUpgradeableContract,
  ORDER_TYPE_ASK,
  ORDER_TYPE_BID,
  ZERO_BYTES16,
} from "./utils";

describe("TradeSettlement", function () {
  async function deploySettlementFixture() {
    const [admin, buyer, seller, operator, executionEngine] =
      await ethers.getSigners();

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
    const settlement = await deployUpgradeableContract("TradeSettlement", [
      admin.address,
      await registry.getAddress(),
    ]);

    const ORCHESTRATOR_ROLE = await controller.ORCHESTRATOR_ROLE();
    await controller.grantRole(ORCHESTRATOR_ROLE, admin.address);

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

    await actorRegistry.registerActor(buyerActorId, buyer.address);
    await actorRegistry.registerActor(sellerActorId, seller.address);

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

    const offer = {
      orderId: bytes16Id("offer-1"),
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
      buyer,
      seller,
      operator,
      executionEngine,
      bid,
      offer,
      buyerActorId,
      sellerActorId,
      marketId,
    };
  }

  it("Should settle a valid trade", async function () {
    const {
      settlement,
      registry,
      buyer,
      seller,
      operator,
      bid,
      offer,
      buyerActorId,
      sellerActorId,
      marketId,
    } =
      await loadFixture(deploySettlementFixture);

    await registry.connect(buyer).placeOrder(bid);
    await registry.connect(seller).placeOrder(offer);

    const matchData = {
      tradeId: bytes16Id("trade-1"),
      bid,
      offer,
      residualBidId: ZERO_BYTES16,
      residualOfferId: ZERO_BYTES16,
      selectedEnergy: 100,
      clearingPrice: 45,
    };

    await expect(settlement.connect(operator).settleBatch([matchData]))
      .to.emit(settlement, "TradeSettled")
      .withArgs(
        matchData.tradeId,
        bid.orderId,
        offer.orderId,
        buyerActorId,
        sellerActorId,
        marketId,
        bid.timeSlot,
        ZERO_BYTES16,
        ZERO_BYTES16,
        matchData.selectedEnergy,
        matchData.clearingPrice,
      );

    expect(await registry.getStatus(bid.orderId)).to.equal(2); // Executed
    expect(await registry.getStatus(offer.orderId)).to.equal(2); // Executed
  });

  it("Should emit residual order ids for partially filled orders", async function () {
    const {
      settlement,
      registry,
      buyer,
      seller,
      operator,
      bid,
      offer,
      buyerActorId,
      sellerActorId,
      marketId,
    } = await loadFixture(deploySettlementFixture);

    const partialOffer = { ...offer, energy: 150 };
    const residualOfferId = bytes16Id("residual-offer-1");

    await registry.connect(buyer).placeOrder(bid);
    await registry.connect(seller).placeOrder(partialOffer);

    const matchData = {
      tradeId: bytes16Id("trade-partial-1"),
      bid,
      offer: partialOffer,
      residualBidId: ZERO_BYTES16,
      residualOfferId,
      selectedEnergy: 100,
      clearingPrice: 45,
    };

    await expect(settlement.connect(operator).settleBatch([matchData]))
      .to.emit(settlement, "TradeSettled")
      .withArgs(
        matchData.tradeId,
        bid.orderId,
        partialOffer.orderId,
        buyerActorId,
        sellerActorId,
        marketId,
        bid.timeSlot,
        ZERO_BYTES16,
        residualOfferId,
        matchData.selectedEnergy,
        matchData.clearingPrice,
      );
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
    const { settlement, operator, bid, offer } = await loadFixture(
      deploySettlementFixture,
    );

    const matchData = {
      tradeId: bytes16Id("trade-1"),
      bid,
      offer,
      residualBidId: ZERO_BYTES16,
      residualOfferId: ZERO_BYTES16,
      selectedEnergy: 100,
      clearingPrice: 45,
    };

    await expect(
      settlement.connect(operator).settleBatch([matchData]),
    ).to.be.revertedWithCustomError(settlement, "OrderNotOpen");
  });

  it("Should fail if match order details do not match stored orders", async function () {
    const { settlement, registry, buyer, seller, operator, bid, offer } =
      await loadFixture(deploySettlementFixture);

    await registry.connect(buyer).placeOrder(bid);
    await registry.connect(seller).placeOrder(offer);

    const tamperedBid = { ...bid, energyRate: bid.energyRate + 1 };
    const matchData = {
      tradeId: bytes16Id("trade-1"),
      bid: tamperedBid,
      offer,
      residualBidId: ZERO_BYTES16,
      residualOfferId: ZERO_BYTES16,
      selectedEnergy: 100,
      clearingPrice: 45,
    };

    await expect(
      settlement.connect(operator).settleBatch([matchData]),
    ).to.be.revertedWithCustomError(settlement, "InvalidOrderParams");
  });

  it("Should fail on price mismatch (Offer > Bid)", async function () {
    const { settlement, registry, buyer, seller, operator, bid, offer } =
      await loadFixture(deploySettlementFixture);

    const highOffer = { ...offer, energyRate: 60 };
    await registry.connect(buyer).placeOrder(bid);
    await registry.connect(seller).placeOrder(highOffer);

    const matchData = {
      tradeId: bytes16Id("trade-1"),
      bid,
      offer: highOffer,
      residualBidId: ZERO_BYTES16,
      residualOfferId: ZERO_BYTES16,
      selectedEnergy: 100,
      clearingPrice: 55,
    };

    await expect(
      settlement.connect(operator).settleBatch([matchData]),
    ).to.be.revertedWithCustomError(settlement, "PriceMismatch");
  });
});
