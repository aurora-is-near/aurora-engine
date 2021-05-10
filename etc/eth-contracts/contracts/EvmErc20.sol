pragma solidity ^0.8.0;

import "@openzeppelin/contracts/utils/Context.sol";
import "@openzeppelin/contracts/token/ERC20/ERC20.sol";
import "./AdminControlled.sol";


/**
 * @title SimpleToken
 * @dev Very simple ERC20 Token example, where all tokens are pre-assigned to the creator.
 * Note they can later distribute these tokens as they wish using `transfer` and other
 * `ERC20` functions.
 */
contract EvmErc20 is Context, ERC20, AdminControlled {
    uint8 private _decimals;

    constructor (string memory name, string memory symbol, uint8 decimal, address admin)  ERC20(name, symbol) AdminControlled(admin, 0) {
        _decimals = decimal;
    }

    function decimals() public view override returns (uint8) {
        return _decimals;
    }

    function mint(address account, uint256 amount) public onlyAdmin {
        _mint(account, amount);
    }

    function withdrawToNear(bytes memory recipient, uint256 amount) public {
        _burn(msg.sender, amount);

        // TODO(#51): How to concatenate bytes in solidity?
        bytes32 amount_b = bytes32(amount);
        uint input_size = 32 + recipient.length;
        bytes memory input = new bytes(32 + recipient.length);
        for (uint i = 0; i < 32; i++) {
            input[i] = amount_b[i];
        }
        for (uint i = 0; i < recipient.length; i++) {
            input[i + 32] = recipient[i];
        }

        assembly {
            let res := staticcall(gas(), 11421322804619973199, add(input, 32), input_size, 0, 32)
        }
    }

    function withdrawToEthereum(address recipient, uint256 amount) public {
        _burn(msg.sender, amount);

        // TODO(#51): How to concatenate bytes in solidity?
        bytes32 amount_b = bytes32(amount);

        bytes memory input = new bytes(32 + 20);
        for (uint i = 0; i < 32; i++) {
            input[i] = amount_b[i];
        }
        bytes20 recipient_b = bytes20(recipient);
        for (uint i = 0; i < 20; i++) {
            input[i + 32] = recipient_b[i];
        }

        assembly {
            let res := staticcall(gas(), 17176159495920586411, input, 52, 0, 32)
        }
    }
}