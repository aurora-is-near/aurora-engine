pragma solidity ^0.8.0;

contract ReverterByDefault {
    uint256 y = 0;
    constructor(uint256 x) public {
        require (x < y, "Revert message");
        y = x;
    }
}
