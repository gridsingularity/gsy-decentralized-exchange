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

    // communityId is the community pseudonym, not its raw identifier.
    struct MarketKey {
        bytes16 communityId;
        MarketType marketType;
        uint64 deliveryStartTime;
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

    // keccak256(abi.encode(communityId, marketType, deliveryStartTime)) => market ID.
    mapping(bytes32 => bytes16) private _marketIds;

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
     * @dev Existing tuples are skipped without an event, preserving the first ID
     *      and record. Reusing an ID for a different tuple reverts the batch.
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
     * @notice Look up market IDs by community, type and delivery start time.
     * @return marketIds IDs in request order, or bytes16(0) for missing tuples.
     */
    function getMarketIds(
        MarketKey[] calldata marketKeys
    ) external view returns (bytes16[] memory marketIds) {
        uint256 marketCount = marketKeys.length;
        marketIds = new bytes16[](marketCount);
        for (uint256 index = 0; index < marketCount; ) {
            MarketKey calldata key = marketKeys[index];
            marketIds[index] = _marketIds[
                _marketKey(key.communityId, key.marketType, key.deliveryStartTime)
            ];
            unchecked {
                ++index;
            }
        }
    }

    /**
     * @notice Check which of several markets have been created, in one call.
     * @return exists exists[i] is true if marketIds[i] has been created.
     */
    function marketsExist(
        bytes16[] calldata marketIds
    ) external view returns (bool[] memory exists) {
        uint256 marketCount = marketIds.length;
        exists = new bool[](marketCount);
        for (uint256 index = 0; index < marketCount; ) {
            exists[index] = _markets[marketIds[index]].createdAt != 0;
            unchecked {
                ++index;
            }
        }
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

    function _createMarket(NewMarket calldata newMarket) private {
        bytes16 marketId = newMarket.marketId;
        if (marketId == bytes16(0)) {
            revert InvalidMarket(marketId);
        }

        // Check ID collisions before tuple deduplication, even if both tuples exist.
        Market storage existing = _markets[marketId];
        if (existing.createdAt != 0) {
            if (
                existing.communityId != newMarket.communityId ||
                existing.marketType != newMarket.marketType ||
                existing.deliveryStartTime != newMarket.deliveryStartTime
            ) {
                revert InvalidMarket(marketId);
            }
            return;
        }

        bytes32 key = _marketKey(
            newMarket.communityId,
            newMarket.marketType,
            newMarket.deliveryStartTime
        );
        if (_marketIds[key] != bytes16(0)) {
            return;
        }
        if (
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
        _marketIds[key] = marketId;
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

    function _marketKey(
        bytes16 communityId,
        MarketType marketType,
        uint64 deliveryStartTime
    ) private pure returns (bytes32) {
        return keccak256(abi.encode(communityId, marketType, deliveryStartTime));
    }
}
