// SPDX-License-Identifier: CC0-1.0
pragma solidity ^0.8.0;


contract SelfDestruct {
    uint256 public counter;

    constructor() { }

    function increase() public {
        counter += 1;
    }

    function finish() public {
        selfdestruct(payable(msg.sender));
    }
}


contract SelfDestructFactory {
    constructor() { }

    function deploy() public returns(address) {
        address addr = address(new SelfDestruct{salt: bytes32(uint256(0x1234))}());
        return addr;
    }
}
