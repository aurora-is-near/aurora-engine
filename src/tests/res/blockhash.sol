// SPDX-License-Identifier: Unlicense
pragma solidity ^0.8.0;

contract BlockHash {
  constructor() payable {}

  function test() public view {
    require(
      blockhash(0) == hex"a7ac0e4bd5ad1654392b64ecd40a69f983e8ce7c315639a339d19a880902457a", 
      "Bad block hash"
    );
  }
}
