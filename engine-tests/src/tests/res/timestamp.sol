// SPDX-License-Identifier: LGPL-3.0-only
pragma solidity ^0.8.6;

contract Timestamp {
    function getCurrentBlockTimestamp() public view returns (uint256 timestamp) {
        timestamp = block.timestamp;
    }
}

