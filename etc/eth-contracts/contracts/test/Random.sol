// SPDX-License-Identifier: CC0-1.0
pragma solidity ^0.8.0;

contract Random {
    function randomSeed() public returns (uint256) {
        bytes32[1] memory value;

        assembly {
            let ret := call(gas(), 0xf861511815955326b953fa97b6955a2f8020a4e9, 0, 0, 0, value, 32)
        }

        return uint256(value[0]);
    }
}
