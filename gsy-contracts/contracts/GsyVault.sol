// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import "@openzeppelin/contracts/access/AccessControl.sol";
import "@openzeppelin/contracts/utils/ReentrancyGuard.sol";

/**
 * @title GsyVault
 * @notice Holds user collateral (Native Currency) and handles transfers.
 */
contract GsyVault is AccessControl, ReentrancyGuard {
    bytes32 public constant SETTLEMENT_ROLE = keccak256("SETTLEMENT_ROLE");

    // User Address => Balance (scaled, usually wei)
    mapping(address => uint256) public balances;

    // Delegator => Delegate => isApproved
    mapping(address => mapping(address => bool)) public proxies;

    // Events
    event Deposited(address indexed user, uint256 amount);
    event Withdrawn(address indexed user, uint256 amount);
    event TransferredBySettlement(
        address indexed from,
        address indexed to,
        uint256 amount
    );
    event ProxyUpdated(
        address indexed delegator,
        address indexed delegate,
        bool isApproved
    );

    error InsufficientBalance(uint256 available, uint256 required);
    error TransferFailed();

    constructor() {
        _grantRole(DEFAULT_ADMIN_ROLE, msg.sender);
    }

    /**
     * @notice Deposit native currency (EWT) into the vault.
     */
    function deposit() external payable nonReentrant {
        balances[msg.sender] += msg.value;
        emit Deposited(msg.sender, msg.value);
    }

    /**
     * @notice Withdraw native currency.
     */
    function withdraw(uint256 amount) external nonReentrant {
        if (balances[msg.sender] < amount)
            revert InsufficientBalance(balances[msg.sender], amount);

        balances[msg.sender] -= amount;

        (bool success, ) = payable(msg.sender).call{value: amount}("");
        if (!success) revert TransferFailed();

        emit Withdrawn(msg.sender, amount);
    }

    /**
     * @notice Add or remove a proxy (delegate) for the caller.
     */
    function setProxy(address delegate, bool status) external {
        proxies[msg.sender][delegate] = status;
        emit ProxyUpdated(msg.sender, delegate, status);
    }

    /**
     * @notice Executed by the Settlement Contract to move funds between users.
     */
    function transferBySettlement(
        address from,
        address to,
        uint256 amount
    ) external onlyRole(SETTLEMENT_ROLE) {
        if (balances[from] < amount)
            revert InsufficientBalance(balances[from], amount);

        balances[from] -= amount;
        balances[to] += amount;

        emit TransferredBySettlement(from, to, amount);
    }

    /**
     * @notice View function to check if a delegate is authorized.
     */
    function isProxy(
        address delegator,
        address delegate
    ) external view returns (bool) {
        return proxies[delegator][delegate];
    }
}
