// SPDX-License-Identifier: GPL-3.0-or-later
pragma solidity ^0.8;

import "@openzeppelin/contracts/token/ERC20/ERC20.sol";
import "@openzeppelin/contracts/token/ERC20/IERC20.sol";
import "@openzeppelin/contracts/token/ERC20/utils/SafeERC20.sol";
import "@openzeppelin/contracts/utils/math/SafeMath.sol";
import "@openzeppelin/contracts/utils/Strings.sol";

contract TestAsync {
    constructor() public {}

    function simpleCall(
        string memory accountId,
        string memory method,
        int128 arg,
        uin256 gas
    ) public returns (string memory) {
        string memory args = string(
            abi.encodePacked(
                '{"arg":"',
                Strings.toString(arg),
                '"}'
            )
        );

        return
            string(
                abi.encodePacked(
                    "promises:",
                    accountId,
                    "#",
                    "deposit",
                    "#",
                    args,
                    "#",
                    Strings.toString(gas)
                )
            );
    }

    function thenCall(
        string memory accountId,
        string memory method1,
        string memory method2,
        int128 arg,
        uin256 gas
    ) public returns (string memory) {
        string memory args = string(
            abi.encodePacked(
                '{"arg":"',
                Strings.toString(arg),
                '"}'
            )
        );

        return
            string(
                abi.encodePacked(
                    "promises:",
                    accountId, "#",
                    method1, "#",
                    args,"#",
                    Strings.toString(gas),
                    "##",
                    accountId,"#",
                    method2,"#",
                    args,"#",
                    Strings.toString(gas), "#",
                    "->", "#",
                    "0"
                )
            );
    }

    function andThenAndCall(
        string memory accountId,
        string memory method1,
        string memory method2,
        string memory method3,
        string memory method4,
        int128 arg,
        uin256 gas
    ) public returns (string memory) {
        string memory args = string(
            abi.encodePacked(
                '{"arg":"',
                Strings.toString(arg),
                '"}'
            )
        );

        return
            string(
                abi.encodePacked(
                    "promises:",
                    accountId, "#",
                    method1, "#",
                    args,"#",
                    Strings.toString(gas),
                    "##",
                    accountId,"#",
                    method2,"#",
                    args,"#",
                    Strings.toString(gas), "#",
                    "&", "#",
                    "0",
                    "##",
                    accountId,"#",
                    method2,"#",
                    args,"#",
                    Strings.toString(gas), "#",
                    "->", "#",
                    "1",
                    "##",
                    accountId,"#",
                    method2,"#",
                    args,"#",
                    Strings.toString(gas), "#",
                    "&", "#",
                    "2"
                )
            );
    }
}
