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

    error InvalidOrderParams();
    error OrderNotOpen();
    error PriceMismatch();
    error EnergyMismatch();
    error InvalidPenalty();
    error TradedQuantityMismatch();

    constructor() {
        _disableInitializers();
    }

    function initialize(address admin, address _registry) external initializer {
        __AccessControl_init();
        _grantRole(DEFAULT_ADMIN_ROLE, admin);
        registry = OrderRegistry(_registry);
    }

    struct OrderData {
        bytes16 orderId;
        bytes16 createdBy;
        bytes16 marketId;
        uint64 timeSlot;
        uint64 creationTime;
        uint64 energy;
        uint64 energyRate;
        uint8 energySourcePreference;
        uint8 energyType;
    }

    struct Match {
        bytes16 tradeId;
        OrderData bid;
        OrderData offer;
        bytes16 residualBidId;
        bytes16 residualOfferId;
        uint256 selectedEnergy;
        uint256 clearingPrice;
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
        uint64 clearingTime;
    }

    event MarketClearing(
        bytes16 indexed marketId,
        uint8 clearingStatus,
        uint256 clearingPrice,
        uint256 totalSupply,
        uint256 totalDemand,
        uint256 tradedQuantity,
        uint32 numTrades,
        uint64 clearingTime
    );

    mapping(bytes16 => uint256) public penaltyEnergyByTrade;
    mapping(bytes16 => uint256) public penaltyEnergyByActor;

    /**
     * @notice Settle a batch of matched trades.
     * @dev Only callable by the Matching Engine (Operator).
     */

    function settleBatch(
        Match[] calldata matches,
        ClearingResult calldata clearingResult
    ) external onlyRole(OPERATOR_ROLE) {
        if (_sumSelectedEnergy(matches) != clearingResult.tradedQuantity) {
            revert TradedQuantityMismatch();
        }

        for (uint256 i = 0; i < matches.length; i++) {
            _settleTrade(matches[i]);
        }

        emit MarketClearing(
            clearingResult.marketId,
            clearingResult.clearingStatus,
            clearingResult.clearingPrice,
            clearingResult.totalSupply,
            clearingResult.totalDemand,
            clearingResult.tradedQuantity,
            clearingResult.numTrades,
            clearingResult.clearingTime
        );
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

            penaltyEnergyByTrade[penalty.tradeId] += penalty.penaltyEnergy;
            penaltyEnergyByActor[penalty.penalizedActorId] += penalty
                .penaltyEnergy;

            emit PenaltyRecorded(
                penalty.penalizedActorId,
                penalty.marketId,
                penalty.tradeId,
                penalty.penaltyEnergy
            );
        }

        emit PenaltiesSubmitted(penalties.length);
    }

    function _settleTrade(Match calldata trade) internal {
        if (
            trade.tradeId == bytes16(0) ||
            trade.bid.orderId == bytes16(0) ||
            trade.offer.orderId == bytes16(0) ||
            trade.bid.createdBy == bytes16(0) ||
            trade.offer.createdBy == bytes16(0) ||
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

        _validateOrderData(trade.bid, registry.getOrder(trade.bid.orderId), true);
        _validateOrderData(
            trade.offer,
            registry.getOrder(trade.offer.orderId),
            false
        );

        if (
            trade.bid.energyRate < trade.clearingPrice ||
            trade.offer.energyRate > trade.clearingPrice
        ) {
            revert PriceMismatch();
        }

        if (
            trade.selectedEnergy > trade.bid.energy ||
            trade.selectedEnergy > trade.offer.energy
        ) {
            revert EnergyMismatch();
        }

        registry.updateStatus(trade.bid.orderId, OrderRegistry.OrderStatus.Executed);
        registry.updateStatus(trade.offer.orderId, OrderRegistry.OrderStatus.Executed);

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

    function _validateOrderData(
        OrderData calldata provided,
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
            stored.isBid != expectedBid
        ) {
            revert InvalidOrderParams();
        }
    }
}
