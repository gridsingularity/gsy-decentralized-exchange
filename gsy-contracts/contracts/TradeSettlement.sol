// SPDX-License-Identifier: MIT
pragma solidity ^0.8.22;

import "@openzeppelin/contracts-upgradeable/access/AccessControlUpgradeable.sol";
import "@openzeppelin/contracts-upgradeable/proxy/utils/Initializable.sol";
import "./OrderRegistry.sol";

/**
 * @title TradeSettlement
 * @notice Validates matches and emits settlement records by Actor UUID.
 */
contract TradeSettlement is Initializable, AccessControlUpgradeable {
    bytes32 public constant OPERATOR_ROLE = keccak256("OPERATOR_ROLE");
    bytes32 public constant EXECUTION_ENGINE_ROLE =
        keccak256("EXECUTION_ENGINE_ROLE");

    OrderRegistry public registry;

    event TradeSettled(
        bytes16 indexed tradeId,
        bytes16 indexed bidId,
        bytes16 indexed offerId,
        bytes16 buyerId,
        bytes16 sellerId,
        bytes16 marketId,
        uint64 timeSlot,
        bytes16 residualBidId,
        bytes16 residualOfferId,
        uint256 energy,
        uint256 price
    );

    event PenaltyRecorded(
        bytes16 indexed penalizedActorId,
        bytes16 indexed marketId,
        bytes16 indexed tradeId,
        uint64 penaltyEnergy
    );
    event PenaltiesSubmitted(uint256 count);
    event DeliveryVerified(bytes16 indexed tradeId, bytes16 indexed bidId, bytes16 indexed offerId);

    error InvalidOrderParams();
    error OrderNotOpen();
    error PriceMismatch();
    error InvalidPreferredPartner();
    error EnergyMismatch();
    error InvalidPenalty();
    error TradedQuantityMismatch();
    error TradeAlreadySettled();
    error UnknownTrade();

    constructor() {
        _disableInitializers();
    }

    function initialize(address admin, address _registry) external initializer {
        __AccessControl_init();
        _grantRole(DEFAULT_ADMIN_ROLE, admin);
        registry = OrderRegistry(_registry);
    }

    enum MatchType {
        Standard,
        Preferred
    }

    struct Match {
        bytes16 tradeId;
        OrderRegistry.OrderParams bid;
        OrderRegistry.OrderParams offer;
        bytes16 residualBidId;
        bytes16 residualOfferId;
        uint256 selectedEnergy;
        uint256 clearingPrice;
        MatchType matchType;
    }

    struct TradePenalty {
        bytes16 penalizedActorId;
        bytes16 marketId;
        bytes16 tradeId;
        uint64 penaltyEnergy;
    }

    struct ClearingResult {
        bytes16 marketId;
        uint8 clearingStatus;
        uint256 clearingPrice;
        uint256 totalSupply;
        uint256 totalDemand;
        uint256 tradedQuantity;
        uint32 numTrades;
    }

    event MarketClearing(
        bytes16 indexed marketId,
        uint8 clearingStatus,
        uint256 clearingPrice,
        uint256 totalSupply,
        uint256 totalDemand,
        uint256 tradedQuantity,
        uint32 numTrades
    );
    struct MarketSettlement {
        Match[] matches;
        ClearingResult clearingResult;
    }

    mapping(bytes16 => uint256) public penaltyEnergyByTrade;
    mapping(bytes16 => uint256) public penaltyEnergyByActor;

    struct SettledTrade {
        bytes16 bidId;
        bytes16 offerId;
        bool verified;
    }

    struct DeliveryVerification {
        bytes16 tradeId;
        uint64 buyerPenaltyEnergy;
        uint64 sellerPenaltyEnergy;
    }

    // Appended storage: existing proxy slots remain unchanged.
    mapping(bytes16 => SettledTrade) public settledTrades;
    mapping(bytes16 => mapping(bytes16 => uint256)) public penaltyEnergyByTradeAndActor;

    /**
     * @notice Settle a batch of matched trades.
     * @dev Only callable by the Matching Engine (Operator).
     */
    function settleBatch(
        MarketSettlement[] calldata settlements
    ) external onlyRole(OPERATOR_ROLE) {
        for (uint256 m = 0; m < settlements.length; m++) {
            MarketSettlement calldata settlement = settlements[m];

            if (_sumSelectedEnergy(settlement.matches) != settlement.clearingResult.tradedQuantity) {
                revert TradedQuantityMismatch();
            }

            for (uint256 i = 0; i < settlement.matches.length; i++) {
                _settleTrade(settlement.matches[i]);
            }

            emit MarketClearing(
                settlement.clearingResult.marketId,
                settlement.clearingResult.clearingStatus,
                settlement.clearingResult.clearingPrice,
                settlement.clearingResult.totalSupply,
                settlement.clearingResult.totalDemand,
                settlement.clearingResult.tradedQuantity,
                settlement.clearingResult.numTrades
            );
        }
    }

    function _sumSelectedEnergy(
        Match[] calldata matches
    ) internal pure returns (uint256 total) {
        for (uint256 i = 0; i < matches.length; i++) {
            total += matches[i].selectedEnergy;
        }
    }

    /**
     * @notice Submit penalties computed by the execution engine.
     * @dev Stores aggregate values and emits events for off-chain indexing.
     */
    function submitPenalties(
        TradePenalty[] calldata penalties
    ) external onlyRole(EXECUTION_ENGINE_ROLE) {
        for (uint256 i = 0; i < penalties.length; i++) {
            TradePenalty calldata penalty = penalties[i];

            if (
                penalty.penalizedActorId == bytes16(0) ||
                penalty.tradeId == bytes16(0) ||
                penalty.penaltyEnergy == 0
            ) {
                revert InvalidPenalty();
            }

            if (settledTrades[penalty.tradeId].verified) revert InvalidPenalty();
            _recordPenalty(penalty.penalizedActorId, penalty.marketId, penalty.tradeId, penalty.penaltyEnergy);
        }

        emit PenaltiesSubmitted(penalties.length);
    }

    /**
     * @notice Record the execution engine's delivery verdict and any penalties atomically.
     * @dev Meter verification is off-chain. Repeated verdicts have no effect; residuals
     * are separate orders and are not completed with their consumed parent.
     */
    function completeDeliveryVerification(
        DeliveryVerification[] calldata verifications
    ) external onlyRole(EXECUTION_ENGINE_ROLE) {
        for (uint256 i = 0; i < verifications.length; i++) {
            DeliveryVerification calldata verification = verifications[i];
            SettledTrade storage trade = settledTrades[verification.tradeId];
            if (trade.bidId == bytes16(0)) revert UnknownTrade();
            if (trade.verified) continue;

            OrderRegistry.OrderParams memory bid = registry.getOrder(trade.bidId);
            OrderRegistry.OrderParams memory offer = registry.getOrder(trade.offerId);
            if (bid.createdBy == offer.createdBy) {
                _ensurePenaltyRecorded(bid.createdBy, bid.marketId, verification.tradeId,
                    verification.buyerPenaltyEnergy + verification.sellerPenaltyEnergy);
            } else {
                _ensurePenaltyRecorded(bid.createdBy, bid.marketId, verification.tradeId, verification.buyerPenaltyEnergy);
                _ensurePenaltyRecorded(offer.createdBy, offer.marketId, verification.tradeId, verification.sellerPenaltyEnergy);
            }
            // Widen before addition: two uint64 penalties can exceed uint64.
            if (penaltyEnergyByTrade[verification.tradeId] !=
                uint256(verification.buyerPenaltyEnergy) + verification.sellerPenaltyEnergy) {
                revert InvalidPenalty();
            }

            registry.markExecuted(trade.bidId);
            registry.markExecuted(trade.offerId);
            trade.verified = true;
            emit DeliveryVerified(verification.tradeId, trade.bidId, trade.offerId);
        }
    }

    function _ensurePenaltyRecorded(bytes16 actorId, bytes16 marketId, bytes16 tradeId, uint64 energy) internal {
        uint256 recorded = penaltyEnergyByTradeAndActor[tradeId][actorId];
        if (recorded == 0 && energy > 0) {
            _recordPenalty(actorId, marketId, tradeId, energy);
        } else if (recorded != energy) {
            revert InvalidPenalty();
        }
    }

    function _recordPenalty(bytes16 actorId, bytes16 marketId, bytes16 tradeId, uint64 energy) internal {
        penaltyEnergyByTrade[tradeId] += energy;
        penaltyEnergyByActor[actorId] += energy;
        penaltyEnergyByTradeAndActor[tradeId][actorId] += energy;
        emit PenaltyRecorded(actorId, marketId, tradeId, energy);
    }

    function _settleTrade(Match calldata trade) internal {
        if (
            trade.tradeId == bytes16(0) ||
            trade.bid.orderId == bytes16(0) ||
            trade.offer.orderId == bytes16(0) ||
            trade.bid.createdBy == bytes16(0) ||
            trade.offer.createdBy == bytes16(0) ||
            trade.bid.createdBy == trade.offer.createdBy ||
            trade.bid.marketId != trade.offer.marketId ||
            trade.bid.timeSlot != trade.offer.timeSlot
        ) {
            revert InvalidOrderParams();
        }

        if (
            registry.getStatus(trade.bid.orderId) != OrderRegistry.OrderStatus.Open ||
            registry.getStatus(trade.offer.orderId) != OrderRegistry.OrderStatus.Open
        ) {
            revert OrderNotOpen();
        }

        _validateOrderParams(trade.bid, registry.getOrder(trade.bid.orderId), true);
        _validateOrderParams(
            trade.offer,
            registry.getOrder(trade.offer.orderId),
            false
        );

        if (settledTrades[trade.tradeId].bidId != bytes16(0)) revert TradeAlreadySettled();

        _validateMatch(trade);

        if (
            trade.selectedEnergy == 0 ||
            trade.selectedEnergy > trade.bid.energy ||
            trade.selectedEnergy > trade.offer.energy
        ) {
            revert EnergyMismatch();
        }

        registry.settleOrder(trade.bid.orderId, uint64(trade.selectedEnergy), trade.residualBidId);
        registry.settleOrder(trade.offer.orderId, uint64(trade.selectedEnergy), trade.residualOfferId);

        settledTrades[trade.tradeId] = SettledTrade(trade.bid.orderId, trade.offer.orderId, false);

        emit TradeSettled(
            trade.tradeId,
            trade.bid.orderId,
            trade.offer.orderId,
            trade.bid.createdBy,
            trade.offer.createdBy,
            trade.bid.marketId,
            trade.bid.timeSlot,
            trade.residualBidId,
            trade.residualOfferId,
            trade.selectedEnergy,
            trade.clearingPrice
        );
    }

    function _validateMatch(Match calldata trade) internal pure {
        if (trade.matchType == MatchType.Standard) {
            if (
                trade.bid.energyRate < trade.clearingPrice ||
                trade.offer.energyRate > trade.clearingPrice
            ) {
                revert PriceMismatch();
            }
            return;
        }

        bytes16 bidPartner = trade.bid.preferredTradingPartner;
        bytes16 offerPartner = trade.offer.preferredTradingPartner;
        if (
            (bidPartner == bytes16(0) && offerPartner == bytes16(0)) ||
            (bidPartner != bytes16(0) && bidPartner != trade.offer.createdBy) ||
            (offerPartner != bytes16(0) && offerPartner != trade.bid.createdBy)
        ) {
            revert InvalidPreferredPartner();
        }

        uint64 bidRate = trade.bid.preferredEnergyRate == 0
            ? trade.bid.energyRate
            : trade.bid.preferredEnergyRate;
        uint64 offerRate = trade.offer.preferredEnergyRate == 0
            ? trade.offer.energyRate
            : trade.offer.preferredEnergyRate;
        if (
            bidRate < offerRate ||
            trade.clearingPrice > bidRate ||
            trade.clearingPrice < offerRate
        ) {
            revert PriceMismatch();
        }
    }

    function _validateOrderParams(
        OrderRegistry.OrderParams calldata provided,
        OrderRegistry.OrderParams memory stored,
        bool expectedBid
    ) internal pure {
        if (
            stored.orderId != provided.orderId ||
            stored.createdBy != provided.createdBy ||
            stored.marketId != provided.marketId ||
            stored.timeSlot != provided.timeSlot ||
            stored.creationTime != provided.creationTime ||
            stored.energy != provided.energy ||
            stored.energyRate != provided.energyRate ||
            stored.energySourcePreference != provided.energySourcePreference ||
            stored.energyType != provided.energyType ||
            stored.preferredTradingPartner != provided.preferredTradingPartner ||
            stored.preferredEnergyRate != provided.preferredEnergyRate ||
            stored.isBid != provided.isBid ||
            stored.isBid != expectedBid
        ) {
            revert InvalidOrderParams();
        }
    }
}
