// SPDX-License-Identifier: CC0-1.0
pragma solidity ^0.8.0;

contract Random {
    function randomSeed() public returns (uint256) {
        bytes32[1] memory value;

        assembly {
            let ret := call(gas(), 0xc104f4840573bed437190daf5d2898c2bdf928ac, 0, 0, 0, value, 32)
        }

        return uint256(value[0]);
    }
}
