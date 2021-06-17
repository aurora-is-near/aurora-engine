// SPDX-License-Identifier: CC0-1.0
pragma solidity ^0.8.0;

interface IExit {
    function withdrawToNear(bytes memory recipient, uint256 amount) external;

    function withdrawToEthereum(address recipient, uint256 amount) external;
}
