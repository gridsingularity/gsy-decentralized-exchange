// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import "@openzeppelin/contracts/access/AccessControl.sol";
import "./OrderRegistry.sol";
import "./GsyVault.sol";

/**
 * @title TradeSettlement
 * @notice Validates matches and settles trades financially by Actor UUID.
 */
contract TradeSettlement is AccessControl {
    bytes32 public constant OPERATOR_ROLE = keccak256("OPERATOR_ROLE");
    bytes32 public constant EXECUTION_ENGINE_ROLE =
        keccak256("EXECUTION_ENGINE_ROLE");

    OrderRegistry public registry;
    GsyVault public vault;

    event TradeSettled(
        bytes16 indexed tradeId,
        bytes16 indexed bidId,
        bytes16 indexed askId,
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

    constructor(address _registry, address _vault) {
        _grantRole(DEFAULT_ADMIN_ROLE, msg.sender);
        registry = OrderRegistry(_registry);
        vault = GsyVault(_vault);
    }

    struct OrderData {
        bytes16 orderId;
        bytes16 createdBy;
        bytes16 marketId;
        uint64 timeSlot;
        uint64 creationTime;
        uint64 energy;
        uint64 energyRate;
    }

    struct Match {
        bytes16 tradeId;
        OrderData bid;
        OrderData ask;
        uint256 selectedEnergy;
        uint256 clearingPrice;
    }

    struct TradePenalty {
        bytes16 penalizedActorId;
        bytes16 marketId;
        bytes16 tradeId;
        uint64 penaltyEnergy;
    }

    mapping(bytes16 => uint256) public penaltyEnergyByTrade;
    mapping(bytes16 => uint256) public penaltyEnergyByActor;

    /**
     * @notice Settle a batch of matched trades.
     * @dev Only callable by the Matching Engine (Operator).
     */
    function settleBatch(
        Match[] calldata matches
    ) external onlyRole(OPERATOR_ROLE) {
        for (uint256 i = 0; i < matches.length; i++) {
            _settleTrade(matches[i]);
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
            trade.ask.orderId == bytes16(0) ||
            trade.bid.createdBy == bytes16(0) ||
            trade.ask.createdBy == bytes16(0) ||
            trade.bid.marketId != trade.ask.marketId
        ) {
            revert InvalidOrderParams();
        }

        if (
            registry.getStatus(trade.bid.orderId) != OrderRegistry.OrderStatus.Open ||
            registry.getStatus(trade.ask.orderId) != OrderRegistry.OrderStatus.Open
        ) {
            revert OrderNotOpen();
        }

        _validateOrderData(trade.bid, registry.getOrder(trade.bid.orderId), true);
        _validateOrderData(trade.ask, registry.getOrder(trade.ask.orderId), false);

        if (
            trade.bid.energyRate < trade.clearingPrice ||
            trade.ask.energyRate > trade.clearingPrice
        ) {
            revert PriceMismatch();
        }

        if (
            trade.selectedEnergy > trade.bid.energy ||
            trade.selectedEnergy > trade.ask.energy
        ) {
            revert EnergyMismatch();
        }

        uint256 totalCost = trade.selectedEnergy * trade.clearingPrice;

        vault.transferBySettlement(
            trade.bid.createdBy,
            trade.ask.createdBy,
            totalCost
        );

        registry.updateStatus(trade.bid.orderId, OrderRegistry.OrderStatus.Executed);
        registry.updateStatus(trade.ask.orderId, OrderRegistry.OrderStatus.Executed);

        emit TradeSettled(
            trade.tradeId,
            trade.bid.orderId,
            trade.ask.orderId,
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
            stored.isBid != expectedBid
        ) {
            revert InvalidOrderParams();
        }
    }
}
