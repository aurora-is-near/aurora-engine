// SPDX-License-Identifier: LGPL-3.0-only
pragma solidity ^0.8.6;

contract Poster {
    event NewPost(address indexed user, string content);
    
    address public sender_address;

    function get() public view returns (address) {
        return sender_address;
    }

    function post(string calldata content) public {
        sender_address = msg.sender;
        emit NewPost(sender_address, content);
    }
}

