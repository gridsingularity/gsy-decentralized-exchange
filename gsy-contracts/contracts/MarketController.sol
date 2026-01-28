// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import "@openzeppelin/contracts/access/AccessControl.sol";

/**
 * @title MarketController
 * @notice Manages the open/closed state of market time slots.
 */
contract MarketController is AccessControl {
    bytes32 public constant ORCHESTRATOR_ROLE = keccak256("ORCHESTRATOR_ROLE");

    // MarketId (hash) => isOpen
    mapping(bytes32 => bool) public marketStatus;

    event MarketStatusUpdated(bytes32 indexed marketId, bool isOpen);

    constructor() {
        _grantRole(DEFAULT_ADMIN_ROLE, msg.sender);
    }

    /**
     * @notice Open or Close a specific market slot.
     * @param marketId Hash of (MarketType + Timestamp)
     * @param isOpen True to open, False to close
     */
    function setMarketStatus(
        bytes32 marketId,
        bool isOpen
    ) external onlyRole(ORCHESTRATOR_ROLE) {
        marketStatus[marketId] = isOpen;
        emit MarketStatusUpdated(marketId, isOpen);
    }

    /**
     * @notice Check if a market is open for trading.
     */
    function isMarketOpen(bytes32 marketId) external view returns (bool) {
        return marketStatus[marketId];
    }
}
