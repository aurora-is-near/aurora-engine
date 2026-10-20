// SPDX-License-Identifier: CC0-1.0
pragma solidity ^0.8.0;

contract P256Verify {
    // Forwards 160 bytes (hash, r, s, qx, qy) to the EIP-7951 P256VERIFY precompile at 0x100.
    function verifyRaw(bytes memory input) external view returns (bool valid) {
        assembly {
            if iszero(eq(mload(input), 160)) {
                revert(0, 0)
            }

            let ptr := mload(0x40)
            let success := staticcall(gas(), 0x100, add(input, 0x20), 160, ptr, 32)

            valid := and(success, and(eq(returndatasize(), 32), eq(mload(ptr), 1)))
        }
    }
}
