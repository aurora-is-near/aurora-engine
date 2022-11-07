// SPDX-License-Identifier: CC0-1.0
pragma solidity ^0.8.0;

interface ICallTooDeep {
    function test() external;
}

contract CallTooDeep {
    function test() external {
        ICallTooDeep(address(this)).test();
    }
}
