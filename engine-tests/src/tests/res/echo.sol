 // SPDX-License-Identifier: GPL-3.0

 pragma solidity >=0.7.0 <0.9.0;

 contract Echo {

     function echo(bytes memory payload) public pure {
         assembly {
             let pos := mload(0x40)

             mstore(pos, mload(add(payload, 0x20)))
             mstore(add(pos, 0x20), mload(add(payload, 0x40)))
             
             return(pos, 51)
         }
     }
 }
