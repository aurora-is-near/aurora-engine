// SPDX-License-Identifier: CC0-1.0
pragma solidity ^0.8.0;

contract AccountIds {
    function currentAccountId() public returns (string memory) {
        // Near accounts are at most 64 1-byte characters (see https://docs.near.org/docs/concepts/account#account-id-rules)
        bytes32[2] memory value;

        assembly {
            let ret := call(gas(), 0xfefae79e4180eb0284f261205e3f8cea737aff56, 0, 0, 0, value, 64)
        }
        
        return bytes64ToString(value);
    }

    function predecessorAccountId() public returns (string memory) {
        // Near accounts are at most 64 1-byte characters (see https://docs.near.org/docs/concepts/account#account-id-rules)
        bytes32[2] memory value;

        assembly {
            let ret := call(gas(), 0x723ffbaba940e75e7bf5f6d61dcbf8d9a4de0fd7, 0, 0, 0, value, 64)
        }
        
        return bytes64ToString(value);
    }

    function bytes64ToString(bytes32[2] memory value) private pure returns (string memory) {
        uint8 result_len = 0;
        while((result_len < 32 && value[0][result_len] != 0) || (result_len >= 32 && result_len < 64 && value[1][result_len - 32] != 0)) {
            result_len++;
        }
        bytes memory result = new bytes(result_len);
        uint8 i = 0;
        for (i = 0; i < 32 && value[0][i] != 0; i++) {
            result[i] = value[0][i];
        }
        if (result_len > 32) {
            for (i = 0; i < 32 && value[1][i] != 0; i++) {
                result[i + 32] = value[1][i];
            }
        }

        return string(result);
    }
}
