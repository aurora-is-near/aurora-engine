// SPDX-License-Identifier: GPL-3.0-or-later
pragma solidity ^0.8;

contract TestAsync {
    constructor() public {}

    function simpleCall(
        string memory accountId,
        string memory method,
        uint128 arg,
        string memory gas
    ) public returns (string memory) {
        string memory args = string(
            abi.encodePacked('{"arg":', toString(arg), '}'
            )
        );

        return
            string(
                abi.encodePacked(
                    "promises:",
                    accountId, "#",
                    method, "#",
                    args, "#",
                    gas
                )
            );
    }

    function thenCall(
        string memory accountId,
        string memory method1,
        string memory method2,
        uint128 arg,
        string memory gas
    ) public returns (string memory) {
        string memory args = string(
            abi.encodePacked('{"arg":', toString(arg), '}'
            )
        );

        return
            string(
                abi.encodePacked(
                    "promises:",
                    accountId, "#",
                    method1, "#",
                    args,"#",
                    gas,
                    "##",
                    accountId,"#",
                    method2,"#",
                    args,"#",
                    gas, "#",
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
        uint128 arg,
        string memory gas
    ) public returns (string memory) {
        string memory args = string(
            abi.encodePacked('{"arg":', toString(arg), '}'
            )
        );

        string memory p1 = string(abi.encodePacked("promises:",
                    accountId, "#",
                    method1, "#",
                    args, "#",
                    gas,
                    "##"));

        string memory p2 = string(abi.encodePacked(
                    accountId, "#",
                    method2, "#",
                    args, "#",
                    gas, "#",
                    "&", "#",
                    "0",
                    "##"));

        string memory p3 = string(abi.encodePacked(
                    accountId, "#",
                    method3, "#",
                    args, "#",
                    gas, "#",
                    "->", "#",
                    "1",
                    "##"));

        string memory p4 = string(abi.encodePacked(
                    accountId, "#",
                    method4, "#",
                    args, "#",
                    gas, "#",
                    "&", "#",
                    "0"));       
        return
            string(
                abi.encodePacked(p1, p2, p3, p4)
            );
    }

    function toString(uint256 value) internal pure returns (string memory) {
        if (value == 0) {
            return "0";
        }
        uint256 temp = value;
        uint256 digits;
        while (temp != 0) {
            digits++;
            temp /= 10;
        }
        bytes memory buffer = new bytes(digits);
        while (value != 0) {
            digits -= 1;
            buffer[digits] = bytes1(uint8(48 + uint256(value % 10)));
            value /= 10;
        }
        return string(buffer);
    }
}