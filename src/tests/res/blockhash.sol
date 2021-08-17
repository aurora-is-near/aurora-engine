// SPDX-License-Identifier: Unlicense
pragma solidity ^0.8.0;

contract BlockHash {
  constructor() payable {}

  function test() public view {
    require(
      blockhash(0) == hex"ec035c7409243a343a8fd798077fb0a5f879cc32c9cd31fd07baa2292e4d3d7c",
      "Bad block hash"
    );
  }
}
