// SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;

contract Bencher {
    function cpu_ram_soak_test(uint32 loop_limit) public pure {
        uint8[102400] memory buf;
        uint32 len = 102400;
        for (uint32 i=0; i < loop_limit; i++) {
            uint32 j = (i * 7 + len / 2) % len;
            uint32 k = (i * 3) % len;
            uint8 tmp = buf[k];
            buf[k] = buf[j];
            buf[j] = tmp;
        }
    }
}
