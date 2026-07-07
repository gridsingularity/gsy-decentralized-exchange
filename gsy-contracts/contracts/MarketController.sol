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

    // Market UUID (bytes16) => isOpen
    mapping(bytes16 => bool) public marketStatus;

    event MarketStatusUpdated(bytes16 indexed marketId, bool isOpen);

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
        marketStatus[marketId] = isOpen;
        emit MarketStatusUpdated(marketId, isOpen);
    }

    /**
     * @notice Check if a market is open for trading.
     */
    function isMarketOpen(bytes16 marketId) external view returns (bool) {
        return marketStatus[marketId];
    }
}
