import { loadFixture } from "@nomicfoundation/hardhat-toolbox/network-helpers";
import { expect } from "chai";
import { ethers } from "hardhat";
import {
  bytes16Id,
  deployUpgradeableContract,
  ENERGY_TYPE_GREEN,
  ENERGY_TYPE_NONE,
  ORDER_TYPE_BID,
  ORDER_TYPE_ASK,
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
      energySourcePreference: ENERGY_TYPE_GREEN,
      energyType: ENERGY_TYPE_NONE,
      isBid: ORDER_TYPE_BID,
      preferredTradingPartner: sellerActorId,
      preferredEnergyRate: 45,
    };

    const offer = {
      orderId: bytes16Id("offer-1"),
      createdBy: sellerActorId,
      marketId: marketId,
      timeSlot: 1000,
      creationTime: 900,
      energy: 100,
      energyRate: 40,
      energySourcePreference: ENERGY_TYPE_NONE,
      energyType: ENERGY_TYPE_GREEN,
      isBid: ORDER_TYPE_ASK,
      preferredTradingPartner: ZERO_BYTES16,
      preferredEnergyRate: 0,
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

  it("Should expose the match type in settlement calldata", async function () {
    const factory = await ethers.getContractFactory("TradeSettlement");
    const fields = factory.interface.getFunction("settleBatch")!
      .inputs[0].arrayChildren!.components!;
    expect(fields.map((field) => field.name)).to.deep.equal([
      "tradeId", "bid", "offer", "residualBidId", "residualOfferId",
      "selectedEnergy", "clearingPrice", "matchType",
    ]);
    expect(fields[7].type).to.equal("uint8");
  });

  for (const [bidPartner, offerPartner, eligible] of [
    ["none", "none", false],
    ["counterparty", "none", true],
    ["none", "counterparty", true],
    ["counterparty", "counterparty", true],
    ["other", "none", false],
    ["none", "other", false],
    ["counterparty", "other", false],
    ["other", "counterparty", false],
    ["order-id", "counterparty", false],
    ["counterparty", "order-id", false],
  ] as const) {
    it(`Should validate preferred counterparties: ${bidPartner}/${offerPartner}`, async function () {
      const { settlement, registry, buyer, seller, operator, bid: originalBid, offer: originalOffer } =
        await loadFixture(deploySettlementFixture);
      const bid = { ...originalBid };
      const offer = { ...originalOffer };
      const partner = (kind: string, counterparty: typeof bid) =>
        kind === "none" ? ZERO_BYTES16
          : kind === "counterparty" ? counterparty.createdBy
          : kind === "order-id" ? counterparty.orderId
          : bytes16Id("unavailable-partner");
      bid.preferredTradingPartner = partner(bidPartner, offer);
      offer.preferredTradingPartner = partner(offerPartner, bid);
      bid.preferredEnergyRate = offer.preferredEnergyRate = 45;
      await registry.connect(buyer).placeOrder(bid);
      await registry.connect(seller).placeOrder(offer);
      const matchData = {
        tradeId: bytes16Id("partner-validation"),
        bid, offer,
        residualBidId: ZERO_BYTES16, residualOfferId: ZERO_BYTES16,
        selectedEnergy: 100, clearingPrice: 45, matchType: 1,
      };
      if (eligible) {
        await expect(settlement.connect(operator).settleBatch([matchData]))
          .to.emit(settlement, "TradeSettled");
      } else {
        await expect(settlement.connect(operator).settleBatch([matchData]))
          .to.be.revertedWithCustomError(settlement, "InvalidPreferredPartner");
        expect(await registry.getStatus(bid.orderId)).to.equal(1);
        expect(await registry.getStatus(offer.orderId)).to.equal(1);
        // Preferences are not exclusive restrictions on standard matching.
        await expect(settlement.connect(operator).settleBatch([{ ...matchData, matchType: 0 }]))
          .to.emit(settlement, "TradeSettled");
      }
    });
  }

  for (const [name, bidRate, offerRate, bidPreferred, offerPreferred, price, valid] of [
    ["equal preferred rates", 50, 40, 45, 45, 45, true],
    ["absent offer preferred rate", 50, 45, 45, 0, 45, true],
    ["absent bid preferred rate", 45, 40, 0, 45, 45, true],
    ["both preferred rates absent", 45, 45, 0, 0, 45, true],
    ["below normal offer limit", 50, 40, 30, 30, 30, true],
    ["above normal bid limit", 50, 40, 60, 60, 60, true],
    ["unequal preferred rates at bid quote", 50, 40, 45, 42, 45, false],
    ["unequal preferred rates at offer quote", 50, 40, 45, 42, 42, false],
    ["unequal fallback rates", 50, 40, 0, 0, 45, false],
    ["unequal offer fallback rate", 50, 40, 45, 0, 45, false],
    ["unequal bid fallback rate", 50, 40, 0, 45, 45, false],
    ["incorrect settlement price", 50, 40, 45, 45, 44, false],
  ] as const) {
    it(`Should validate preferred prices: ${name}`, async function () {
      const { settlement, registry, buyer, seller, operator, bid: originalBid, offer: originalOffer } =
        await loadFixture(deploySettlementFixture);
      const bid = { ...originalBid };
      const offer = { ...originalOffer };
      bid.energyRate = bidRate;
      offer.energyRate = offerRate;
      bid.preferredEnergyRate = bidPreferred;
      offer.preferredEnergyRate = offerPreferred;
      offer.preferredTradingPartner = bid.createdBy;
      await registry.connect(buyer).placeOrder(bid);
      await registry.connect(seller).placeOrder(offer);
      const matchData = {
        tradeId: bytes16Id("price-validation"),
        bid, offer,
        residualBidId: ZERO_BYTES16, residualOfferId: ZERO_BYTES16,
        selectedEnergy: 100, clearingPrice: price, matchType: 1,
      };
      if (price < offerRate || price > bidRate) {
        await expect(settlement.connect(operator).settleBatch([{ ...matchData, matchType: 0 }]))
          .to.be.revertedWithCustomError(settlement, "PriceMismatch");
      }
      if (valid) {
        await expect(settlement.connect(operator).settleBatch([matchData]))
          .to.emit(settlement, "TradeSettled");
      } else {
        await expect(settlement.connect(operator).settleBatch([matchData]))
          .to.be.revertedWithCustomError(settlement, "PriceMismatch");
        expect(await registry.getStatus(bid.orderId)).to.equal(1);
        expect(await registry.getStatus(offer.orderId)).to.equal(1);
        await expect(settlement.connect(operator).settleBatch([{ ...matchData, matchType: 0 }]))
          .to.emit(settlement, "TradeSettled");
      }
    });
  }

  it("Should reject an unknown match type", async function () {
    const { settlement, registry, buyer, seller, operator, bid, offer } =
      await loadFixture(deploySettlementFixture);
    await registry.connect(buyer).placeOrder(bid);
    await registry.connect(seller).placeOrder(offer);
    await expect(settlement.connect(operator).settleBatch([{
      tradeId: bytes16Id("invalid-match-type"), bid, offer,
      residualBidId: ZERO_BYTES16, residualOfferId: ZERO_BYTES16,
      selectedEnergy: 100, clearingPrice: 45, matchType: 2,
    }])).to.be.reverted;
    expect(await registry.getStatus(bid.orderId)).to.equal(1);
    expect(await registry.getStatus(offer.orderId)).to.equal(1);
  });

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
      matchType: 0,
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
      matchType: 0,
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

  for (const side of ["bid", "offer"] as const) {
    for (const sameBatch of [true, false]) {
      it(`Should register and settle a residual ${side} in ${sameBatch ? "the same batch" : "a later transaction"}`, async function () {
        const { settlement, registry, buyer, seller, operator, bid, offer } =
          await loadFixture(deploySettlementFixture);
        const orders = { bid, offer };
        orders[side] = { ...orders[side], energy: 150 };
        const otherSide = side === "bid" ? "offer" : "bid";
        const counterpart = {
          ...orders[otherSide],
          orderId: bytes16Id("second-counterpart"),
          energy: 50,
        };
        const residual = {
          ...orders[side],
          orderId: bytes16Id("residual"),
          energy: 50,
        };
        await registry.connect(buyer).placeOrder(orders.bid);
        await registry.connect(seller).placeOrder(orders.offer);
        await registry.connect(side === "bid" ? seller : buyer).placeOrder(counterpart);

        const first = {
          tradeId: bytes16Id("first-trade"),
          ...orders,
          residualBidId: side === "bid" ? residual.orderId : ZERO_BYTES16,
          residualOfferId: side === "offer" ? residual.orderId : ZERO_BYTES16,
          selectedEnergy: 100,
          clearingPrice: 45,
          matchType: 0,
        };
        const second = {
          ...first,
          tradeId: bytes16Id("second-trade"),
          [side]: residual,
          [otherSide]: counterpart,
          selectedEnergy: 50,
          residualBidId: ZERO_BYTES16,
          residualOfferId: ZERO_BYTES16,
        };

        const firstTx = settlement.connect(operator).settleBatch(sameBatch ? [first, second] : [first]);
        await expect(firstTx).to.emit(registry, "OrderPlaced").withArgs(
          residual.orderId, residual.createdBy, residual.marketId, residual.timeSlot,
          residual.creationTime, residual.energy, residual.energyRate,
          residual.energySourcePreference, residual.energyType, residual.isBid,
          residual.preferredTradingPartner, residual.preferredEnergyRate,
        );
        const stored = await registry.getOrder(residual.orderId);
        for (const [field, value] of Object.entries(residual)) {
          expect(stored[field], field).to.equal(value);
        }
        expect(await registry.getStatus(orders[side].orderId)).to.equal(2);
        if (!sameBatch) {
          expect(await registry.getStatus(residual.orderId)).to.equal(1);
          await settlement.connect(operator).settleBatch([second]);
        }
        expect(await registry.getStatus(residual.orderId)).to.equal(2);
        expect(await registry.getStatus(counterpart.orderId)).to.equal(2);
        await expect(settlement.connect(operator).settleBatch([first]))
          .to.be.revertedWithCustomError(settlement, "OrderNotOpen");
        await expect(settlement.connect(operator).settleBatch([second]))
          .to.be.revertedWithCustomError(settlement, "OrderNotOpen");
      });
    }
  }

  for (const invalid of ["missing", "unexpected", "parent", "counterpart", "duplicate", "zero-energy", "excess-energy"]) {
    it(`Should reject ${invalid} residual settlement atomically`, async function () {
      const { settlement, registry, buyer, seller, operator, bid, offer } =
        await loadFixture(deploySettlementFixture);
      await registry.connect(buyer).placeOrder(bid);
      await registry.connect(seller).placeOrder(offer);
      const residualBidId = bytes16Id("residual-bid");
      const matchData = {
        tradeId: bytes16Id("invalid-trade"),
        bid,
        offer,
        residualBidId,
        residualOfferId: bytes16Id("residual-offer"),
        selectedEnergy: 50,
        clearingPrice: 45,
        matchType: 0,
      };
      if (invalid === "missing") matchData.residualOfferId = ZERO_BYTES16;
      if (invalid === "unexpected") matchData.selectedEnergy = 100;
      if (invalid === "parent") matchData.residualOfferId = offer.orderId;
      if (invalid === "counterpart") matchData.residualOfferId = bid.orderId;
      if (invalid === "duplicate") matchData.residualOfferId = residualBidId;
      if (invalid === "zero-energy") matchData.selectedEnergy = 0;
      if (invalid === "excess-energy") matchData.selectedEnergy = 101;

      const errorContract = invalid.endsWith("energy") ? settlement : registry;
      const errorName = invalid.endsWith("energy") ? "EnergyMismatch"
        : ["parent", "counterpart", "duplicate"].includes(invalid) ? "OrderAlreadyExists"
        : "InvalidOrderParams";
      await expect(settlement.connect(operator).settleBatch([matchData]))
        .to.be.revertedWithCustomError(errorContract, errorName);
      expect(await registry.getStatus(bid.orderId)).to.equal(1);
      expect(await registry.getStatus(offer.orderId)).to.equal(1);
      expect(await registry.getStatus(residualBidId)).to.equal(0);
    });
  }

  it("Should restrict residual creation to the settlement role", async function () {
    const { registry, buyer, bid } = await loadFixture(deploySettlementFixture);
    await registry.connect(buyer).placeOrder(bid);
    await expect(registry.connect(buyer).settleOrder(bid.orderId, 50, bytes16Id("residual")))
      .to.be.revertedWithCustomError(registry, "AccessControlUnauthorizedAccount");
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
      matchType: 0,
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
      matchType: 0,
    };

    await expect(
      settlement.connect(operator).settleBatch([matchData]),
    ).to.be.revertedWithCustomError(settlement, "InvalidOrderParams");
  });

  for (const side of ["bid", "offer"] as const) {
    for (const field of [
      "isBid",
      "preferredTradingPartner",
      "preferredEnergyRate",
    ] as const) {
      it(`Should reject a changed ${field} on the ${side}`, async function () {
        const { settlement, registry, buyer, seller, operator, bid, offer } =
          await loadFixture(deploySettlementFixture);

        await registry.connect(buyer).placeOrder(bid);
        await registry.connect(seller).placeOrder(offer);

        const order = side === "bid" ? bid : offer;
        const changedValue = field === "isBid"
          ? !order.isBid
          : field === "preferredEnergyRate"
            ? order.preferredEnergyRate + 1
            : bytes16Id("different-partner");
        const matchData = {
          tradeId: bytes16Id("trade-tampered"),
          bid,
          offer,
          [side]: { ...order, [field]: changedValue },
          residualBidId: ZERO_BYTES16,
          residualOfferId: ZERO_BYTES16,
          selectedEnergy: 100,
          clearingPrice: 45,
          matchType: 0,
        };

        await expect(
          settlement.connect(operator).settleBatch([matchData]),
        ).to.be.revertedWithCustomError(settlement, "InvalidOrderParams");
        expect(await registry.getStatus(bid.orderId)).to.equal(1); // Open
        expect(await registry.getStatus(offer.orderId)).to.equal(1); // Open
      });
    }
  }

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
      matchType: 0,
    };

    await expect(
      settlement.connect(operator).settleBatch([matchData]),
    ).to.be.revertedWithCustomError(settlement, "PriceMismatch");
  });
});
