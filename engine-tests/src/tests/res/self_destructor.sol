// SPDX-License-Identifier: MIT

pragma solidity ^0.8.0;

contract SelfDestruct {
   constructor() payable {}

   function destruct(address benefactor) payable external {
      selfdestruct(payable(benefactor));
   }

}
