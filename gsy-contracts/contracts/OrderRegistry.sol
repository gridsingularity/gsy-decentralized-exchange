// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import "@openzeppelin/contracts/access/AccessControl.sol";
import "./MarketController.sol";
import "./GsyVault.sol";

/**
 * @title OrderRegistry
 * @notice Stores order commitments and validities.
 */
contract OrderRegistry is AccessControl {
    bytes32 public constant SETTLEMENT_ROLE = keccak256("SETTLEMENT_ROLE");

    enum OrderStatus {
        None,
        Open,
        Executed,
        Cancelled
    }

    // External Contract References
    MarketController public marketController;
    GsyVault public vault;

    // OrderHash => Status
    mapping(bytes32 => OrderStatus) public orderStatus;

    // Events to replace Offchain Worker logic
    event OrderPlaced(
        bytes32 indexed orderHash,
        address indexed owner,
        bytes32 indexed marketId,
        bytes32 areaUuid,
        uint64 nonce,
        uint64 timeSlot,
        uint64 creationTime,
        uint64 energy,
        uint64 energyRate,
        bool isBid // true = Bid, false = Ask
    );

    event OrderCancelled(bytes32 indexed orderHash);
    event OrderStatusUpdated(bytes32 indexed orderHash, OrderStatus status);

    error MarketClosed();
    error Unauthorized();
    error OrderNotOpen();
    error OrderAlreadyExists();

    constructor(address _marketController, address _vault) {
        _grantRole(DEFAULT_ADMIN_ROLE, msg.sender);
        marketController = MarketController(_marketController);
        vault = GsyVault(_vault);
    }

    struct OrderParams {
        address owner;
        uint64 nonce;
        bytes32 areaUuid;
        bytes32 marketId;
        uint64 timeSlot;
        uint64 creationTime;
        uint64 energy;
        uint64 energyRate;
        bool isBid;
    }

    /**
     * @notice Place an order.
     * @dev Validates market status and caller authority (direct or proxy).
     */
    function placeOrder(OrderParams calldata params) external {
        // 1. Check if Market is Open
        if (!marketController.isMarketOpen(params.marketId)) {
            revert MarketClosed();
        }

        // 2. Validate Sender (Direct or Proxy)
        if (msg.sender != params.owner) {
            bool isAuthorized = vault.isProxy(params.owner, msg.sender);
            if (!isAuthorized) revert Unauthorized();
        }

        // 3. Calculate Hash (Commitment)
        bytes32 orderHash = _hashOrder(params);

        if (orderStatus[orderHash] != OrderStatus.None) {
            revert OrderAlreadyExists();
        }

        // 4. Update State
        orderStatus[orderHash] = OrderStatus.Open;

        // 5. Emit Event for Off-Chain Relayer
        emit OrderPlaced(
            orderHash,
            params.owner,
            params.marketId,
            params.areaUuid,
            params.nonce,
            params.timeSlot,
            params.creationTime,
            params.energy,
            params.energyRate,
            params.isBid
        );
    }

    /**
     * @notice Cancel an order.
     * @dev Requires the original params to reconstruct the hash and verify ownership
     *      without on-chain storage overhead.
     */
    function cancelOrder(OrderParams calldata params) external {
        // 1. Reconstruct the hash
        bytes32 orderHash = _hashOrder(params);

        // 2. Verify the order is currently Open
        if (orderStatus[orderHash] != OrderStatus.Open) {
            revert OrderNotOpen();
        }

        // 3. Verify Authorization (Owner or Proxy)
        if (msg.sender != params.owner) {
            bool isAuthorized = vault.isProxy(params.owner, msg.sender);
            if (!isAuthorized) revert Unauthorized();
        }

        // 4. Update State
        orderStatus[orderHash] = OrderStatus.Cancelled;

        // 5. Emit Event
        emit OrderCancelled(orderHash);
    }

    /**
     * @notice Update status (called by TradeSettlement).
     */
    function updateStatus(
        bytes32 orderHash,
        OrderStatus status
    ) external onlyRole(SETTLEMENT_ROLE) {
        orderStatus[orderHash] = status;
        emit OrderStatusUpdated(orderHash, status);
    }

    /**
     * @notice Helper to check status
     */
    function getStatus(bytes32 orderHash) external view returns (OrderStatus) {
        return orderStatus[orderHash];
    }

    /**
     * @dev Internal helper to ensure consistent hashing across Place and Cancel
     */
    function _hashOrder(
        OrderParams calldata params
    ) internal pure returns (bytes32) {
        return
            keccak256(
                abi.encode(
                    params.owner,
                    params.nonce,
                    params.areaUuid,
                    params.marketId,
                    params.timeSlot,
                    params.creationTime,
                    params.energy,
                    params.energyRate,
                    params.isBid
                )
            );
    }
}
