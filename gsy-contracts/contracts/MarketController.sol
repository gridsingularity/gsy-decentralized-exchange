// SPDX-License-Identifier: MIT
pragma solidity ^0.8.22;

import "@openzeppelin/contracts-upgradeable/access/AccessControlUpgradeable.sol";
import "@openzeppelin/contracts-upgradeable/proxy/utils/Initializable.sol";

/**
 * @title MarketController
 * @notice Manages the open/closed state of market time slots.
 */
contract MarketController is Initializable, AccessControlUpgradeable {
    bytes32 public constant ORCHESTRATOR_ROLE = keccak256("ORCHESTRATOR_ROLE");

    struct MarketInfo {
        bytes16 communityId;
        uint64 openingTime;
        uint64 closingTime;
        uint64 deliveryStartTime;
        uint64 deliveryEndTime;
        uint64 createdAt;
        uint8 matchingAlgorithm;
        uint8 marketType;
        bool isOpen;
    }
    mapping(bytes16 => MarketInfo) public marketRegistry;

    event MarketInfoUpdated(
        bytes16 indexed marketId,
        bytes16 indexed communityId,
        uint64 openingTime,
        uint64 closingTime,
        uint64 deliveryStartTime,
        uint64 deliveryEndTime,
        uint64 createdAt,
        uint8 matchingAlgorithm,
        uint8 marketType,
        bool isOpen
    );

    constructor() {
        _disableInitializers();
    }

    function initialize(address admin) external initializer {
        __AccessControl_init();
        _grantRole(DEFAULT_ADMIN_ROLE, admin);
    }

    /**
     * @notice Open or Close a specific market slot.
     * @param marketId Intelligent market UUID encoded as bytes16.
     * @param isOpen True to open, False to close
     */
    function setMarketStatus(
        bytes16 marketId,
        bool isOpen
    ) external onlyRole(ORCHESTRATOR_ROLE) {
        marketRegistry[marketId].isOpen = isOpen;
        MarketInfo storage market_info = marketRegistry[marketId];
        emit MarketInfoUpdated(
            marketId,
            market_info.communityId,
            market_info.openingTime,
            market_info.closingTime,
            market_info.deliveryStartTime,
            market_info.deliveryEndTime,
            market_info.createdAt,
            market_info.matchingAlgorithm,
            market_info.marketType,
            market_info.isOpen
        );
    }
    /**
     * @notice Create or overwrite a market slot in the registry.
     */
    function createMarket(
        bytes16 marketId,
        bytes16 communityId,
        uint64 openingTime,
        uint64 closingTime,
        uint64 deliveryStartTime,
        uint64 deliveryEndTime,
        uint64 createdAt,
        uint8 matchingAlgorithm,
        uint8 marketType,
        bool isOpen
    ) external onlyRole(ORCHESTRATOR_ROLE) {
        MarketInfo memory mInfo = MarketInfo({
            communityId: communityId,
            openingTime: openingTime,
            closingTime: closingTime,
            deliveryStartTime: deliveryStartTime,
            deliveryEndTime: deliveryEndTime,
            createdAt: createdAt,
            matchingAlgorithm: matchingAlgorithm,
            marketType: marketType,
            isOpen: isOpen
        });
        marketRegistry[marketId] = mInfo;
        emit MarketInfoUpdated(
            marketId,
            mInfo.communityId,
            mInfo.openingTime,
            mInfo.closingTime,
            mInfo.deliveryStartTime,
            mInfo.deliveryEndTime,
            mInfo.createdAt,
            mInfo.matchingAlgorithm,
            mInfo.marketType,
            mInfo.isOpen
        );
    }
    /**
     * @notice Check if a market is open for trading.
     */
    function isMarketOpen(bytes16 marketId) external view returns (bool) {
        return marketRegistry[marketId].isOpen;
    }
}
