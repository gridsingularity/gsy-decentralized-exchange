// SPDX-License-Identifier: MIT
pragma solidity ^0.8.22;

import "@openzeppelin/contracts-upgradeable/access/AccessControlUpgradeable.sol";
import "@openzeppelin/contracts-upgradeable/proxy/utils/Initializable.sol";
import "./MarketController.sol";
import "./ActorRegistry.sol";

/**
 * @title OrderRegistry
 * @notice Stores order commitments and validities using Intelligent UUID identities.
 */
contract OrderRegistry is Initializable, AccessControlUpgradeable {
    bytes32 public constant SETTLEMENT_ROLE = keccak256("SETTLEMENT_ROLE");

    enum OrderStatus {
        None,
        Open,
        Executed,
        Cancelled
    }

    MarketController public marketController;
    ActorRegistry public actorRegistry;

    uint8 public constant ENERGY_TYPE_UNSPECIFIED = 0;
    uint8 public constant ENERGY_TYPE_GREEN = 1;
    uint8 public constant ENERGY_TYPE_PV = 2;
    uint8 public constant ENERGY_TYPE_HYDRO = 3;
    uint8 public constant ENERGY_TYPE_BIOMASS = 4;
    uint8 public constant ENERGY_TYPE_BATTERY = 5;
    uint8 public constant ENERGY_TYPE_GREY = 6;

    struct OrderParams {
        bytes16 orderId;
        bytes16 createdBy;
        bytes16 marketId;
        uint64 timeSlot;
        uint64 creationTime;
        uint64 energy;
        uint64 energyRate;
        uint8 energySourcePreference;
        uint8 energyType;
        bool isBid;
    }

    // Intelligent Order UUID (bytes16) => Status
    mapping(bytes16 => OrderStatus) public orderStatus;
    mapping(bytes16 => OrderParams) private orders;

    event OrderPlaced(
        bytes16 indexed orderId,
        bytes16 indexed createdBy,
        bytes16 indexed marketId,
        uint64 timeSlot,
        uint64 creationTime,
        uint64 energy,
        uint64 energyRate,
        uint8 energySourcePreference,
        uint8 energyType,
        bool isBid
    );
    event OrderCancelled(bytes16 indexed orderId);
    event OrderStatusUpdated(bytes16 indexed orderId, OrderStatus status);

    error MarketClosed();
    error Unauthorized();
    error InvalidOrderParams();
    error OrderNotOpen();
    error OrderAlreadyExists();

    constructor() {
        _disableInitializers();
    }

    function initialize(
        address admin,
        address _marketController,
        address _actorRegistry
    ) external initializer {
        __AccessControl_init();
        _grantRole(DEFAULT_ADMIN_ROLE, admin);
        marketController = MarketController(_marketController);
        actorRegistry = ActorRegistry(_actorRegistry);
    }

    /**
     * @notice Place an order.
     * @dev Validates market status and caller authority for the Actor UUID.
     */
    function placeOrder(OrderParams calldata params) external {
        if (
            params.orderId == bytes16(0) ||
            params.createdBy == bytes16(0) ||
            params.marketId == bytes16(0) ||
            params.energySourcePreference > ENERGY_TYPE_GREY ||
            params.energyType > ENERGY_TYPE_GREY
        ) {
            revert InvalidOrderParams();
        }

        if (!marketController.isMarketOpen(params.marketId)) {
            revert MarketClosed();
        }

        if (!actorRegistry.isAuthorized(params.createdBy, msg.sender)) {
            revert Unauthorized();
        }

        if (orderStatus[params.orderId] != OrderStatus.None) {
            revert OrderAlreadyExists();
        }

        orderStatus[params.orderId] = OrderStatus.Open;
        orders[params.orderId] = params;

        emit OrderPlaced(
            params.orderId,
            params.createdBy,
            params.marketId,
            params.timeSlot,
            params.creationTime,
            params.energy,
            params.energyRate,
            params.energySourcePreference,
            params.energyType,
            params.isBid
        );
    }

    /**
     * @notice Cancel an order.
     * @dev Requires the original params to verify actor authorization.
     */
    function cancelOrder(OrderParams calldata params) external {
        if (orderStatus[params.orderId] != OrderStatus.Open) {
            revert OrderNotOpen();
        }

        OrderParams storage storedOrder = orders[params.orderId];
        if (storedOrder.createdBy != params.createdBy) {
            revert Unauthorized();
        }

        if (!actorRegistry.isAuthorized(storedOrder.createdBy, msg.sender)) {
            revert Unauthorized();
        }

        orderStatus[params.orderId] = OrderStatus.Cancelled;
        emit OrderCancelled(params.orderId);
    }

    /**
     * @notice Update status (called by TradeSettlement).
     */
    function updateStatus(
        bytes16 orderId,
        OrderStatus status
    ) external onlyRole(SETTLEMENT_ROLE) {
        orderStatus[orderId] = status;
        emit OrderStatusUpdated(orderId, status);
    }

    /**
     * @notice Helper to check status.
     */
    function getStatus(bytes16 orderId) external view returns (OrderStatus) {
        return orderStatus[orderId];
    }

    function getOrder(bytes16 orderId) external view returns (OrderParams memory) {
        return orders[orderId];
    }
}
