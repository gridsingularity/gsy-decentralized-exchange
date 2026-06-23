// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import "@openzeppelin/contracts/access/AccessControl.sol";

/**
 * @title ActorRegistry
 * @notice Maintains Actor UUID to wallet/delegate authorizations.
 */
contract ActorRegistry is AccessControl {
    bytes32 public constant ACTOR_REGISTRAR_ROLE =
        keccak256("ACTOR_REGISTRAR_ROLE");

    // Actor UUID (bytes16) => Wallet/Delegate => isApproved
    mapping(bytes16 => mapping(address => bool)) public authorizedWallets;

    event ActorWalletUpdated(
        bytes16 indexed actorId,
        address indexed wallet,
        bool isAuthorized
    );
    event ProxyUpdated(
        bytes16 indexed actorId,
        address indexed delegate,
        bool isApproved
    );

    error InvalidActorId();
    error InvalidWallet();
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
