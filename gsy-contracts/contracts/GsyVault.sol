// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import "@openzeppelin/contracts/access/AccessControl.sol";
import "@openzeppelin/contracts/utils/ReentrancyGuard.sol";

/**
 * @title GsyVault
 * @notice Holds actor collateral (Native Currency) and actor wallet authorizations.
 */
contract GsyVault is AccessControl, ReentrancyGuard {
    bytes32 public constant ACTOR_REGISTRAR_ROLE =
        keccak256("ACTOR_REGISTRAR_ROLE");

    // Actor UUID (bytes16) => Balance (scaled, usually wei)
    mapping(bytes16 => uint256) public balances;

    // Actor UUID (bytes16) => Wallet/Delegate => isApproved
    mapping(bytes16 => mapping(address => bool)) public authorizedWallets;

    // Events
    event ActorWalletUpdated(
        bytes16 indexed actorId,
        address indexed wallet,
        bool isAuthorized
    );
    event Deposited(
        bytes16 indexed actorId,
        address indexed wallet,
        uint256 amount
    );
    event Withdrawn(
        bytes16 indexed actorId,
        address indexed wallet,
        uint256 amount
    );
    event ProxyUpdated(
        bytes16 indexed actorId,
        address indexed delegate,
        bool isApproved
    );

    error InsufficientBalance(uint256 available, uint256 required);
    error InvalidActorId();
    error InvalidWallet();
    error TransferFailed();
    error UnauthorizedActorWallet(bytes16 actorId, address wallet);

    constructor() {
        _grantRole(DEFAULT_ADMIN_ROLE, msg.sender);
        _grantRole(ACTOR_REGISTRAR_ROLE, msg.sender);
    }

    modifier onlyActorWallet(bytes16 actorId) {
        if (!authorizedWallets[actorId][msg.sender]) {
            revert UnauthorizedActorWallet(actorId, msg.sender);
        }
        _;
    }

    /**
     * @notice Register a wallet as authorized for an actor UUID.
     */
    function registerActor(
        bytes16 actorId,
        address wallet
    ) external onlyRole(ACTOR_REGISTRAR_ROLE) {
        _setActorWallet(actorId, wallet, true);
    }

    /**
     * @notice Add or remove a wallet authorized for an actor UUID.
     */
    function setActorWallet(
        bytes16 actorId,
        address wallet,
        bool status
    ) external onlyRole(ACTOR_REGISTRAR_ROLE) {
        _setActorWallet(actorId, wallet, status);
    }

    /**
     * @notice Deposit native currency (EWT) into the vault.
     */
    function deposit(
        bytes16 actorId
    ) external payable nonReentrant onlyActorWallet(actorId) {
        balances[actorId] += msg.value;
        emit Deposited(actorId, msg.sender, msg.value);
    }

    /**
     * @notice Withdraw native currency.
     */
    function withdraw(
        bytes16 actorId,
        uint256 amount
    ) external nonReentrant onlyActorWallet(actorId) {
        if (balances[actorId] < amount)
            revert InsufficientBalance(balances[actorId], amount);

        balances[actorId] -= amount;

        (bool success, ) = payable(msg.sender).call{value: amount}("");
        if (!success) revert TransferFailed();

        emit Withdrawn(actorId, msg.sender, amount);
    }

    /**
     * @notice Add or remove a proxy (delegate) for an actor.
     */
    function setProxy(
        bytes16 actorId,
        address delegate,
        bool status
    ) external onlyActorWallet(actorId) {
        authorizedWallets[actorId][delegate] = status;
        emit ProxyUpdated(actorId, delegate, status);
    }

    /**
     * @notice View function to check if a delegate is authorized.
     */
    function isProxy(
        bytes16 actorId,
        address delegate
    ) external view returns (bool) {
        return authorizedWallets[actorId][delegate];
    }

    /**
     * @notice View function to check if a wallet may act for an actor UUID.
     */
    function isAuthorized(
        bytes16 actorId,
        address wallet
    ) external view returns (bool) {
        return authorizedWallets[actorId][wallet];
    }

    function _setActorWallet(
        bytes16 actorId,
        address wallet,
        bool status
    ) private {
        if (actorId == bytes16(0)) revert InvalidActorId();
        if (wallet == address(0)) revert InvalidWallet();

        authorizedWallets[actorId][wallet] = status;
        emit ActorWalletUpdated(actorId, wallet, status);
    }
}
