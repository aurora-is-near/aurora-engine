// SPDX-License-Identifier: CC0-1.0
pragma solidity ^0.8.0;

import "@openzeppelin/contracts/token/ERC20/IERC20.sol";
import "@openzeppelin/contracts/token/ERC20/utils/SafeERC20.sol";
import "@openzeppelin/contracts/utils/math/SafeMath.sol";
import "./AdminControlled.sol";

contract EvmErc20Locker is AdminControlled {
    using SafeMath for uint256;
    using SafeERC20 for IERC20;

    event Locked (
        address indexed token,
        address indexed sender,
        uint256 amount,
        string accountId
    );

    event Unlocked (
        uint256 amount,
        address recipient
    );

    uint constant UNPAUSED_ALL = 0;
    uint constant PAUSED_LOCK = 1 << 0;
    uint constant PAUSED_UNLOCK = 1 << 1;

    // ERC20Locker is linked to the bridge token factory on NEAR side.
    constructor(address _admin)
        AdminControlled(_admin, 0)
        public
    {
    }

    function lockToken(address ethToken, uint256 amount, bytes memory recipient)
        public
        pausable (PAUSED_LOCK)
    {
        require(IERC20(ethToken).balanceOf(address(this)).add(amount) <= ((uint256(1) << 128) - 1), "Maximum tokens locked exceeded (< 2^128 - 1)");
        IERC20(ethToken).safeTransferFrom(msg.sender, address(this), amount);
        bytes32 ethToken_b = bytes32(uint256(uint160(ethToken)));
        bytes32 amount_b = bytes32(amount);
        bytes memory input = abi.encodePacked("\x02", ethToken_b, amount_b, recipient);
        uint input_size = 1 + 32 + recipient.length;

        assembly {
            let res := call(gas(), 0xe9217bc70b7ed1f598ddd3199e80b093fa71124f, 0, add(input, 32), input_size, 0, 32)
        }

        emit Locked(address(ethToken), msg.sender, amount, string(recipient));
    }

    function unlockToken(address token, uint256 amount, address recipient)
        public
        onlyAdmin
        pausable (PAUSED_UNLOCK)
    {
        IERC20(token).safeTransfer(recipient, amount);
        emit Unlocked(amount, recipient);
    }

    // tokenFallback implements the ContractReceiver interface from ERC223-token-standard.
    // This allows to support ERC223 tokens with no extra cost.
    // The function always passes: we don't need to make any decision and the contract always
    // accept token transfers transfer.
    function tokenFallback(address _from, uint _value, bytes memory _data) public pure {}

    function adminTransfer(IERC20 token, address destination, uint amount)
        public
        onlyAdmin
    {
        token.safeTransfer(destination, amount);
    }
}
