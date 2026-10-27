// SPDX-License-Identifier: MIT
pragma solidity ^0.8.22;

import "@openzeppelin/contracts-upgradeable/access/AccessControlUpgradeable.sol";
import "@openzeppelin/contracts-upgradeable/proxy/utils/Initializable.sol";
import "./MarketController.sol";

/**
 * @title OrderRegistry
 * @notice Stores order commitments and validities using Intelligent UUID identities.
 */
contract OrderRegistry is Initializable, AccessControlUpgradeable {
    bytes32 public constant SETTLEMENT_ROLE = keccak256("SETTLEMENT_ROLE");
    bytes32 public constant COMMUNITY_CLIENT_ROLE = keccak256("COMMUNITY_CLIENT_ROLE");

    enum OrderStatus {
        None,
        Open,
        Executed,
        Cancelled,
        Expired,
        PartiallyFilled,
        Filled
    }

    MarketController public marketController;

    uint8 public constant ENERGY_TYPE_NONE = 0;
    uint8 public constant ENERGY_TYPE_GREEN = 1;
    uint8 public constant ENERGY_TYPE_PV = 2;
    uint8 public constant ENERGY_TYPE_HYDRO = 3;
    uint8 public constant ENERGY_TYPE_BIOMASS = 4;
    uint8 public constant ENERGY_TYPE_BATTERY = 5;

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
        bytes16 preferredTradingPartner;
        uint64 preferredEnergyRate;
    }

    // Intelligent Order UUID (bytes16) => Status
    mapping(bytes16 => OrderStatus) private storedOrderStatus;
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
        bool isBid,
        bytes16 preferredTradingPartner,
        uint64 preferredEnergyRate
    );
    event OrderCancelled(bytes16 indexed orderId);
    event OrderStatusUpdated(bytes16 indexed orderId, OrderStatus status);

    error MarketClosed();
    error Unauthorized();
    error InvalidOrderParams();
    error OrderNotOpen();
    error OrderAlreadyExists();
    error OrderNotFilled();

    constructor() {
        _disableInitializers();
    }

    function initialize(
        address admin,
        address _marketController
    ) external initializer {
        __AccessControl_init();
        _grantRole(DEFAULT_ADMIN_ROLE, admin);
        marketController = MarketController(_marketController);
    }

    /**
     * @notice Place an order.
     * @dev The authorized service validates facility/community eligibility off-chain.
     */
    function placeOrder(OrderParams calldata params) external {
        if (
            params.orderId == bytes16(0) ||
            params.createdBy == bytes16(0) ||
            params.marketId == bytes16(0) ||
            params.energySourcePreference > ENERGY_TYPE_BATTERY ||
            params.energyType > ENERGY_TYPE_BATTERY
        ) {
            revert InvalidOrderParams();
        }

        if (!marketController.isMarketOpen(params.marketId)) {
            revert MarketClosed();
        }

        if (!hasRole(COMMUNITY_CLIENT_ROLE, msg.sender)) {
            revert Unauthorized();
        }

        _storeOrder(params);
    }

    function _storeOrder(OrderParams memory params) internal {
        if (storedOrderStatus[params.orderId] != OrderStatus.None) {
            revert OrderAlreadyExists();
        }

        storedOrderStatus[params.orderId] = OrderStatus.Open;
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
            params.isBid,
            params.preferredTradingPartner,
            params.preferredEnergyRate
        );
    }

    /**
     * @notice Consume an open order and register its remaining quantity atomically.
     * @dev Residual metadata is inherited from the stored order, not supplied by the operator.
     */
    function settleOrder(
        bytes16 orderId,
        uint64 selectedEnergy,
        bytes16 residualOrderId
    ) external onlyRole(SETTLEMENT_ROLE) {
        if (orderStatus(orderId) != OrderStatus.Open) {
            revert OrderNotOpen();
        }

        OrderParams memory residual = orders[orderId];
        if (selectedEnergy == 0 || selectedEnergy > residual.energy) {
            revert InvalidOrderParams();
        }
        residual.energy -= selectedEnergy;
        if ((residual.energy == 0) != (residualOrderId == bytes16(0))) {
            revert InvalidOrderParams();
        }

        OrderStatus status = residual.energy == 0
            ? OrderStatus.Filled
            : OrderStatus.PartiallyFilled;
        storedOrderStatus[orderId] = status;
        emit OrderStatusUpdated(orderId, status);

        if (residual.energy > 0) {
            residual.orderId = residualOrderId;
            _storeOrder(residual);
        }
    }

    /// @notice Complete only the consumed quantity; residual order IDs remain independent.
    function markExecuted(bytes16 orderId) external onlyRole(SETTLEMENT_ROLE) {
        OrderStatus status = storedOrderStatus[orderId];
        if (status != OrderStatus.Filled && status != OrderStatus.PartiallyFilled) {
            revert OrderNotFilled();
        }
        storedOrderStatus[orderId] = OrderStatus.Executed;
        emit OrderStatusUpdated(orderId, OrderStatus.Executed);
    }

    /**
     * @notice Cancel an order.
     * @dev Requires an authorized service and the original facility identity.
     */
    function cancelOrder(OrderParams calldata params) external {
        if (orderStatus(params.orderId) != OrderStatus.Open) {
            revert OrderNotOpen();
        }

        OrderParams storage storedOrder = orders[params.orderId];
        if (storedOrder.createdBy != params.createdBy) {
            revert Unauthorized();
        }

        if (!hasRole(COMMUNITY_CLIENT_ROLE, msg.sender)) {
            revert Unauthorized();
        }

        storedOrderStatus[params.orderId] = OrderStatus.Cancelled;
        emit OrderCancelled(params.orderId);
    }

    /**
     * @notice Update status (called by TradeSettlement).
     */
    function updateStatus(
        bytes16 orderId,
        OrderStatus status
    ) external onlyRole(SETTLEMENT_ROLE) {
        storedOrderStatus[orderId] = status;
        emit OrderStatusUpdated(orderId, status);
    }

    /**
     * @notice Helper to check status.
     */
    function getStatus(bytes16 orderId) external view returns (OrderStatus) {
        return orderStatus(orderId);
    }

    /**
     * @notice Return the effective status, including expiry after market closure.
     * @dev Preserves the original mapping getter ABI without per-order expiry writes.
     */
    function orderStatus(bytes16 orderId) public view returns (OrderStatus) {
        OrderStatus status = storedOrderStatus[orderId];
        if (status == OrderStatus.Open) {
            bytes16 marketId = orders[orderId].marketId;
            if (!marketController.isMarketOpen(marketId)) {
                return OrderStatus.Expired;
            }
        }
        return status;
    }

    function getOrder(bytes16 orderId) external view returns (OrderParams memory) {
        return orders[orderId];
    }
}
