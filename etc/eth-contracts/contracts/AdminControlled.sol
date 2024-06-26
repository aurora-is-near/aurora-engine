// SPDX-License-Identifier: CC0-1.0
pragma solidity ^0.8.0;


contract AdminControlled {
    // slither-disable-next-line immutable-states
    address public admin;
    uint public paused;

    constructor(address _admin, uint flags) {
        // slither-disable-next-line missing-zero-check
        admin = _admin;

        // Add the possibility to set pause flags on the initialization
        paused = flags;
    }

    modifier onlyAdmin {
        require(msg.sender == admin);
        _;
    }

    modifier pausable(uint flag) {
        require((paused & flag) == 0 || msg.sender == admin);
        _;
    }

    function adminPause(uint flags) public onlyAdmin {
        paused = flags;
    }

    function adminSstore(uint key, uint value) public onlyAdmin {
        assembly {
            sstore(key, value)
        }
    }

    function adminSendEth(address payable destination, uint amount) public onlyAdmin {
        // slither-disable-next-line missing-zero-check
        destination.transfer(amount);
    }

    function adminReceiveEth() public payable onlyAdmin {}

    function adminDelegatecall(address target, bytes memory data) public payable onlyAdmin returns (bytes memory) {
        // slither-disable-next-line controlled-delegatecall,low-level-calls,missing-zero-check
        (bool success, bytes memory rdata) = target.delegatecall(data);
        require(success);
        return rdata;
    }
}
