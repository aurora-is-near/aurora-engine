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

    constructor (string memory name, string memory symbol, uint8 decimal)  ERC20(name, symbol) AdminControlled(address(0), 0) {
        _decimals = decimal;
    }

    function decimals() public view override returns (uint8) {
        return _decimals;
    }

    function mint(address account, uint256 amount) public onlyAdmin {
        _mint(account, amount);
    }
}