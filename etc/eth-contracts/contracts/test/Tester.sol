// SPDX-License-Identifier: CC0-1.0
pragma solidity ^0.8.0;

import "../IExit.sol";

contract Tester {
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
}
