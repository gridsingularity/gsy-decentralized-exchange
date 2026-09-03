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
        _setMarketStatus(marketId, isOpen);
    }

    /**
     * @notice Open or close multiple market slots in one transaction.
     * @param marketIds Intelligent market UUIDs encoded as bytes16.
     * @param isOpen True to open, false to close.
     */
    function setMarketStatuses(
        bytes16[] calldata marketIds,
        bool isOpen
    ) external onlyRole(ORCHESTRATOR_ROLE) {
        uint256 marketCount = marketIds.length;
        for (uint256 index = 0; index < marketCount; ) {
            _setMarketStatus(marketIds[index], isOpen);
            unchecked {
                ++index;
            }
        }
    }

    /**
     * @notice Check if a market is open for trading.
     */
    function isMarketOpen(bytes16 marketId) external view returns (bool) {
        return marketStatus[marketId];
    }

    function _setMarketStatus(bytes16 marketId, bool isOpen) private {
        marketStatus[marketId] = isOpen;
        emit MarketStatusUpdated(marketId, isOpen);
    }
}
