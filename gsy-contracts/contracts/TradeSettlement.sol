// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import "@openzeppelin/contracts/access/AccessControl.sol";
import "./OrderRegistry.sol";
import "./GsyVault.sol";

/**
 * @title TradeSettlement
 * @notice Validates matches and settles trades financially.
 */
contract TradeSettlement is AccessControl {
    bytes32 public constant OPERATOR_ROLE = keccak256("OPERATOR_ROLE");
    bytes32 public constant EXECUTION_ENGINE_ROLE =
        keccak256("EXECUTION_ENGINE_ROLE");

    OrderRegistry public registry;
    GsyVault public vault;

    event TradeSettled(
        bytes32 indexed tradeId,
        bytes32 indexed bidHash,
        bytes32 indexed askHash,
        uint256 energy,
        uint256 price
    );

    event PenaltyRecorded(
        address indexed penalizedAccount,
        bytes32 indexed marketId,
        bytes32 indexed tradeId,
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

    // Structs to reconstruct the Hash on-chain for validation
    struct OrderData {
        address owner;
        uint64 nonce;
        bytes32 areaUuid;
        bytes32 marketId;
        uint64 timeSlot;
        uint64 creationTime;
        uint64 energy;
        uint64 energyRate;
    }

    struct Match {
        OrderData bid;
        OrderData ask;
        uint256 selectedEnergy;
        uint256 clearingPrice;
    }

    struct TradePenalty {
        address penalizedAccount;
        bytes32 marketId;
        bytes32 tradeId;
        uint64 penaltyEnergy;
    }

    mapping(bytes32 => uint256) public penaltyEnergyByTrade;
    mapping(address => uint256) public penaltyEnergyByAccount;

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
                penalty.penalizedAccount == address(0) ||
                penalty.tradeId == bytes32(0) ||
                penalty.penaltyEnergy == 0
            ) {
                revert InvalidPenalty();
            }

            penaltyEnergyByTrade[penalty.tradeId] += penalty.penaltyEnergy;
            penaltyEnergyByAccount[penalty.penalizedAccount] += penalty
                .penaltyEnergy;

            emit PenaltyRecorded(
                penalty.penalizedAccount,
                penalty.marketId,
                penalty.tradeId,
                penalty.penaltyEnergy
            );
        }

        emit PenaltiesSubmitted(penalties.length);
    }

    function _settleTrade(Match calldata trade) internal {
        // 1. Reconstruct Hashes to verify these orders actually exist in Registry
        bytes32 bidHash = keccak256(
            abi.encode(
                trade.bid.owner,
                trade.bid.nonce,
                trade.bid.areaUuid,
                trade.bid.marketId,
                trade.bid.timeSlot,
                trade.bid.creationTime,
                trade.bid.energy,
                trade.bid.energyRate,
                true // isBid = true
            )
        );

        bytes32 askHash = keccak256(
            abi.encode(
                trade.ask.owner,
                trade.ask.nonce,
                trade.ask.areaUuid,
                trade.ask.marketId,
                trade.ask.timeSlot,
                trade.ask.creationTime,
                trade.ask.energy,
                trade.ask.energyRate,
                false // isBid = false
            )
        );

        // 2. Validate Registry Status
        // Both orders must be Open.
        if (
            registry.getStatus(bidHash) != OrderRegistry.OrderStatus.Open ||
            registry.getStatus(askHash) != OrderRegistry.OrderStatus.Open
        ) {
            revert OrderNotOpen();
        }

        // 3. Validate Matching Logic
        // Bid Price must be >= Clearing Price
        // Ask Price must be <= Clearing Price
        if (
            trade.bid.energyRate < trade.clearingPrice ||
            trade.ask.energyRate > trade.clearingPrice
        ) {
            revert PriceMismatch();
        }

        // Selected Energy must not exceed available energy
        if (
            trade.selectedEnergy > trade.bid.energy ||
            trade.selectedEnergy > trade.ask.energy
        ) {
            revert EnergyMismatch();
        }

        // 4. Execute Financial Transfer
        // Calculation: Cost = Energy * Price / ScalingFactor (if needed).
        // Assuming inputs are scaled similarly to Substrate (e.g. 10000).
        // Note: Solidity math requires care with scaling.
        // For V1, we assume direct multiplication if units match EWT wei, or simple logic.
        // Let's assume Price is per unit of energy.
        uint256 totalCost = trade.selectedEnergy * trade.clearingPrice;

        vault.transferBySettlement(trade.bid.owner, trade.ask.owner, totalCost);

        // 5. Update Registry
        // Mark orders as Executed.
        // NOTE: In this simplified architecture, we mark the whole order hash as Executed.
        // Residuals would technically generate a NEW Order Hash (new nonce or new amount)
        // pushed by the Matching Engine off-chain, just like the Rust node did.
        registry.updateStatus(bidHash, OrderRegistry.OrderStatus.Executed);
        registry.updateStatus(askHash, OrderRegistry.OrderStatus.Executed);

        // 6. Emit Event
        bytes32 tradeId = keccak256(
            abi.encodePacked(bidHash, askHash, block.timestamp)
        );
        emit TradeSettled(
            tradeId,
            bidHash,
            askHash,
            trade.selectedEnergy,
            trade.clearingPrice
        );
    }
}
