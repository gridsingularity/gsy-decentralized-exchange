// SPDX-License-Identifier: MIT
pragma solidity ^0.8.22;

import "../ActorRegistry.sol";

contract ActorRegistryV2Mock is ActorRegistry {
    function version() external pure returns (uint256) {
        return 2;
    }
}
