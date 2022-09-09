// SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;

contract Caller {
    function greet(address to) public {
        to.call(abi.encodeWithSelector(Greeter(to).greet.selector));
    }
}


contract Greeter { // callee contract
    event Logger(address sender);

    function greet() public {
        emit Logger(msg.sender);
    }
}

