// SPDX-License-Identifier: CC0-1.0
pragma solidity ^0.8.0;

import "../IExit.sol";

contract TesterV2 {
    IExit public erc20Token;

    constructor (IExit _erc20Token) {
        erc20Token = _erc20Token;
    }

    function helloWorld(string memory name) public pure returns(string memory) {
        return string(abi.encodePacked("Hello ", name, "!"));
    }

    // Use flag to indicate if should withdraw to NEAR (true) or to Ethereum (false)
    function withdraw(bool toNear) external {
        if (toNear) {
            erc20Token.withdrawToNear("target.aurora", 1);
        } else {
            erc20Token.withdrawToEthereum(0xE0f5206BBD039e7b0592d8918820024e2a7437b9, 1);
        }
    }

    function withdrawAndFail(bool toNear) external {
        this.withdraw(toNear);
        require(false);
    }

    function tryWithdrawAndAvoidFail(bool toNear) external {
        try this.withdrawAndFail(toNear) {
            require(false);
        } catch {
        }
    }

    function tryWithdrawAndAvoidFailAndSucceed(bool toNear) external {
        this.tryWithdrawAndAvoidFail(toNear);
        this.withdraw(toNear);
    }

    function withdrawEthToNear(bytes memory recipient) external payable {
        address sender = msg.sender;
        bytes memory input = abi.encodePacked("\x00", sender, recipient);
        uint input_size = 1 + 20 + recipient.length;
        uint256 amount = msg.value;

        assembly {
            let res := call(gas(), 0xe9217bc70b7ed1f598ddd3199e80b093fa71124f, amount, add(input, 32), input_size, 0, 32)
        }
    }

    function withdrawEthToEthereum(address recipient) external payable {
        bytes20 recipient_b = bytes20(recipient);
        bytes memory input = abi.encodePacked("\x00", recipient_b);
        uint input_size = 1 + 20;
        uint256 amount = msg.value;

        assembly {
            let res := call(gas(), 0xb0bd02f6a392af548bdf1cfaee5dfa0eefcc8eab, amount, add(input, 32), input_size, 0, 32)
        }
    }
}
