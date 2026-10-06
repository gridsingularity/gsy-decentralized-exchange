// SPDX-License-Identifier: MIT
pragma solidity ^0.8.22;

import "@openzeppelin/contracts-upgradeable/access/AccessControlUpgradeable.sol";
import "@openzeppelin/contracts-upgradeable/proxy/utils/Initializable.sol";

/**
 * @title MarketController
 * @notice Stores market records. A market is open while
 *         openingTime <= block.timestamp < closingTime.
 */
contract MarketController is Initializable, AccessControlUpgradeable {
    bytes32 public constant ORCHESTRATOR_ROLE = keccak256("ORCHESTRATOR_ROLE");

    // Same order as primitives::MarketType.
    enum MarketType {
        Spot,
        Flex,
        Settlement
    }

    // Same order as primitives::MatchingAlgorithm.
    enum MatchingAlgorithm {
        PayAsBid,
        PayAsClear,
        Amm
    }

    // Mirrors primitives::MarketSchema; the market ID is the mapping key.
    struct Market {
        bytes16 communityId;
        uint64 openingTime;
        uint64 closingTime;
        uint64 deliveryStartTime;
        uint64 deliveryEndTime;
        uint64 createdAt;
        MarketType marketType;
        MatchingAlgorithm matchingAlgorithm;
    }

    // createdAt is set by the contract.
    struct NewMarket {
        bytes16 marketId;
        bytes16 communityId;
        uint64 openingTime;
        uint64 closingTime;
        uint64 deliveryStartTime;
        uint64 deliveryEndTime;
        MarketType marketType;
        MatchingAlgorithm matchingAlgorithm;
    }

    // Market UUID (bytes16) => market record. createdAt == 0 means missing.
    mapping(bytes16 => Market) private _markets;

    event NewMarketCreated(
        bytes16 indexed marketId,
        bytes16 indexed communityId,
        uint64 openingTime,
        uint64 closingTime,
        uint64 deliveryStartTime,
        uint64 deliveryEndTime,
        uint8 marketType,
        uint8 matchingAlgorithm,
        uint64 createdAt
    );

    error InvalidMarket(bytes16 marketId);

    constructor() {
        _disableInitializers();
    }

    function initialize(address admin) external initializer {
        __AccessControl_init();
        _grantRole(DEFAULT_ADMIN_ROLE, admin);
    }

    /**
     * @notice Create multiple markets in one transaction.
     * @dev Markets that already exist are skipped without an event, so a
     *      resent or concurrently sent market does not revert the batch.
     * @param newMarkets Market records.
     */
    function createMarkets(
        NewMarket[] calldata newMarkets
    ) external onlyRole(ORCHESTRATOR_ROLE) {
        uint256 marketCount = newMarkets.length;
        for (uint256 index = 0; index < marketCount; ) {
            _createMarket(newMarkets[index]);
            unchecked {
                ++index;
            }
        }
    }

    /**
     * @notice Check if a market has been created.
     */
    function marketExists(bytes16 marketId) external view returns (bool) {
        return _markets[marketId].createdAt != 0;
    }

    /**
     * @notice Check if a market is open for trading at the current block time.
     */
    function isMarketOpen(bytes16 marketId) external view returns (bool) {
        Market storage market = _markets[marketId];
        return
            market.createdAt != 0 &&
            market.openingTime <= block.timestamp &&
            block.timestamp < market.closingTime;
    }

    /**
     * @notice Return a market record; createdAt is 0 if it does not exist.
     */
    function getMarket(bytes16 marketId) external view returns (Market memory) {
        return _markets[marketId];
    }

    function _createMarket(NewMarket calldata newMarket) private {
        bytes16 marketId = newMarket.marketId;
        if (_markets[marketId].createdAt != 0) {
            return;
        }
        if (
            marketId == bytes16(0) ||
            newMarket.communityId == bytes16(0) ||
            newMarket.closingTime <= newMarket.openingTime ||
            newMarket.deliveryEndTime <= newMarket.deliveryStartTime
        ) {
            revert InvalidMarket(marketId);
        }

        uint64 createdAt = uint64(block.timestamp);
        _markets[marketId] = Market({
            communityId: newMarket.communityId,
            openingTime: newMarket.openingTime,
            closingTime: newMarket.closingTime,
            deliveryStartTime: newMarket.deliveryStartTime,
            deliveryEndTime: newMarket.deliveryEndTime,
            createdAt: createdAt,
            marketType: newMarket.marketType,
            matchingAlgorithm: newMarket.matchingAlgorithm
        });
        emit NewMarketCreated(
            marketId,
            newMarket.communityId,
            newMarket.openingTime,
            newMarket.closingTime,
            newMarket.deliveryStartTime,
            newMarket.deliveryEndTime,
            uint8(newMarket.marketType),
            uint8(newMarket.matchingAlgorithm),
            createdAt
        );
    }
}
