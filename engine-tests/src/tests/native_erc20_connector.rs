// Simulation tests for exit to NEAR precompile.
// Note: `AuroraRunner` is not suitable for these tests because
// it does not execute promises; but `near-sdk-sim` does.
mod sim_tests {
    use crate::prelude::{WeiU256, U256};
    use crate::test_utils;
    use crate::test_utils::erc20::{ERC20Constructor, ERC20};
    use crate::tests::state_migration::{deploy_evm, AuroraAccount};
    use aurora_engine::parameters::{
        CallArgs, DeployErc20TokenArgs, FunctionCallArgsV2, SetErc20FactoryAccountArgs,
        SubmitResult,
    };
    use aurora_engine_types::account_id::AccountId;
    use aurora_engine_types::types::Address;
    use borsh::BorshSerialize;
    use near_sdk_sim::UserAccount;
    use serde_json::json;
    use sha3::Digest;
    use ethabi::Token;

    const TOKEN_FACTORY_PATH: &str = "src/tests/res/bridge_aurora_token_factory.wasm";
    const FT_TOTAL_SUPPLY: u128 = 1_000_000;
    const FT_EXIT_AMOUNT: u128 = 100_000;
    const BRIDGE_TOKEN_INIT_BALANCE: u128 = 3_000_000_000_000_000_000_000_000; // 3e24yN, 3N

    #[test]
    fn test_lock_unlock_native_erc20() {
        let TestExitToNearContext {
            erc20_locker,
            erc20,
            nep141,
            ft_owner,
            ft_owner_address,
            aurora,
        } = test_exit_to_near_common();

        mint_erc20_token(&erc20, ft_owner_address, FT_TOTAL_SUPPLY.into(), &aurora);

        assert_eq!(
            erc20_balance(&erc20, ft_owner_address, &aurora),
            (FT_TOTAL_SUPPLY).into()
        );

        approve_erc20_token(
            &erc20,
            &ft_owner,
            &erc20_locker,
            FT_EXIT_AMOUNT.into(),
            &aurora,
        );

        nep141_storage_deposit(ft_owner.account_id.as_str(), &nep141, &aurora);

        // Call lock function on ERC-20; observe ERC-20 locked + NEP-141 minted
        lock_token(
            &erc20_locker,
            &ft_owner,
            ft_owner.account_id.as_str(),
            FT_EXIT_AMOUNT,
            &erc20.0.address,
            &aurora,
        );

        assert_eq!(
            erc20_balance(&erc20, ft_owner_address, &aurora),
            (FT_TOTAL_SUPPLY - FT_EXIT_AMOUNT).into()
        );

        assert_eq!(
            erc20_balance(&erc20, erc20_locker, &aurora),
            FT_EXIT_AMOUNT.into()
        );

        assert_eq!(
            nep_141_balance_of(ft_owner.account_id.as_str(), &nep141, &aurora),
            FT_EXIT_AMOUNT
        );

        // Call withdraw on NEP-141: observe NEP-141 burned + ERC-20 unlocked
        transfer_nep_141_to_erc_20(
            &nep141,
            &ft_owner_address.encode(),
            &ft_owner,
            FT_EXIT_AMOUNT,
        );

        assert_eq!(
            nep_141_balance_of(ft_owner.account_id.as_str(), &nep141, &aurora),
            0
        );

        assert_eq!(erc20_balance(&erc20, erc20_locker, &aurora), 0.into());

        assert_eq!(
            erc20_balance(&erc20, ft_owner_address, &aurora),
            FT_TOTAL_SUPPLY.into()
        );
    }

    fn build_input(str_selector: &str, inputs: &[Token]) -> Vec<u8> {
        let sel = sha3::Keccak256::digest(str_selector.as_bytes()).to_vec()[..4].to_vec();
        let inputs = ethabi::encode(inputs);
        [sel.as_slice(), inputs.as_slice()].concat().to_vec()
    }

    fn test_exit_to_near_common() -> TestExitToNearContext {
        // 1. deploy Aurora
        let aurora = deploy_evm();
        // 2. deploy erc-20 locker
        let erc20_locker = deploy_erc20_locker(&aurora);
        // 3. deploy erc-20 factory
        let erc20_factory = deploy_erc20_factory("factory", &erc20_locker.encode(), &aurora);
        // 4. Deploy ERC-20
        let erc20 = deploy_erc20_token(AccountId::new("tt.root").unwrap(), &aurora);
        // 5. Deploy nep141
        let nep141 = deploy_bridge_token(&erc20.0.address.encode(), &erc20_factory, &aurora);
        // 6. Set erc-20 factory
        set_erc20_factory(&erc20_factory.account_id.to_string(), &aurora);
        // 7. Create account
        let ft_owner = aurora.user.create_user(
            "ft_owner.root".parse().unwrap(),
            near_sdk_sim::STORAGE_AMOUNT,
        );
        let ft_owner_address =
            aurora_engine_sdk::types::near_account_to_evm_address(ft_owner.account_id.as_bytes());

        TestExitToNearContext {
            erc20_locker,
            erc20,
            nep141,
            ft_owner,
            ft_owner_address,
            aurora,
        }
    }

    fn lock_token(
        locker_address: &Address,
        source: &near_sdk_sim::UserAccount,
        recipient: &str,
        amount: u128,
        erc20: &Address,
        aurora: &AuroraAccount,
    ) {
        let input = build_input(
            "lockToken(address,uint256,bytes)",
            &[
                ethabi::Token::Address(erc20.raw()),
                ethabi::Token::Uint(U256::from(amount).into()),
                ethabi::Token::Bytes(recipient.into()),
            ],
        );
        let call_args = CallArgs::V2(FunctionCallArgsV2 {
            contract: locker_address.clone(),
            value: WeiU256::default(),
            input,
        });

        source
            .call(
                aurora.contract.account_id(),
                "call",
                &call_args.try_to_vec().unwrap(),
                near_sdk_sim::DEFAULT_GAS,
                0,
            )
            .assert_success();
    }

    fn mint_erc20_token(erc20: &ERC20, dest: Address, amount: u128, aurora: &AuroraAccount) {
        let mint_tx = erc20.mint(dest, amount.into(), 0.into());
        let call_args = CallArgs::V2(FunctionCallArgsV2 {
            contract: erc20.0.address,
            value: WeiU256::default(),
            input: mint_tx.data,
        });
        aurora
            .contract
            .call(
                aurora.contract.account_id(),
                "call",
                &call_args.try_to_vec().unwrap(),
                near_sdk_sim::DEFAULT_GAS,
                0,
            )
            .assert_success();
    }

    fn approve_erc20_token(
        erc20: &ERC20,
        source: &near_sdk_sim::UserAccount,
        spender: &Address,
        amount: u128,
        aurora: &AuroraAccount,
    ) {
        let tx = erc20.approve(spender.clone(), amount.into(), 0.into());
        let call_args = CallArgs::V2(FunctionCallArgsV2 {
            contract: erc20.0.address,
            value: WeiU256::default(),
            input: tx.data,
        });
        source
            .call(
                aurora.contract.account_id(),
                "call",
                &call_args.try_to_vec().unwrap(),
                near_sdk_sim::DEFAULT_GAS,
                0,
            )
            .assert_success();
    }

    fn transfer_nep_141_to_erc_20(
        nep_141: &near_sdk::AccountId,
        recipient: &str,
        source: &near_sdk_sim::UserAccount,
        amount: u128,
    ) {
        let transfer_args = json!({
            "amount": format!("{:?}", amount),
            "recipient": recipient,
        });
        source
            .call(
                nep_141.clone(),
                "withdraw",
                transfer_args.to_string().as_bytes(),
                near_sdk_sim::DEFAULT_GAS,
                1,
            )
            .assert_success();
    }

    fn erc20_balance(erc20: &ERC20, address: Address, aurora: &AuroraAccount) -> U256 {
        let balance_tx = erc20.balance_of(address, 0.into());
        let call_args = CallArgs::V2(FunctionCallArgsV2 {
            contract: erc20.0.address,
            value: WeiU256::default(),
            input: balance_tx.data,
        });
        let result = aurora.call("call", &call_args.try_to_vec().unwrap());
        let submit_result: SubmitResult = result.unwrap_borsh();
        U256::from_big_endian(&test_utils::unwrap_success(submit_result))
    }

    fn deploy_erc20_token(nep_141: AccountId, aurora: &AuroraAccount) -> ERC20 {
        let args = DeployErc20TokenArgs { nep141: nep_141 };
        let result = aurora.call("deploy_erc20_token", &args.try_to_vec().unwrap());
        let addr_bytes: Vec<u8> = result.unwrap_borsh();
        let address = Address::try_from_slice(&addr_bytes).unwrap();
        let abi = ERC20Constructor::load().0.abi;
        ERC20(crate::test_utils::solidity::DeployedContract { abi, address })
    }

    fn set_erc20_factory(factory: &str, aurora: &AuroraAccount) {
        let args = SetErc20FactoryAccountArgs {
            factory: aurora_engine_types::account_id::AccountId::new(factory).unwrap(),
        };
        aurora
            .contract
            .call(
                aurora.contract.account_id.clone(),
                "set_native_erc20_factory",
                &args.try_to_vec().unwrap(),
                near_sdk_sim::DEFAULT_GAS,
                0,
            )
            .assert_success();
    }

    fn deploy_erc20_locker(aurora: &AuroraAccount) -> Address {
        let result = aurora.contract.call(
            aurora.contract.account_id.clone(),
            "deploy_erc20_locker",
            &[],
            near_sdk_sim::DEFAULT_GAS,
            0,
        );
        let addr_bytes: Vec<u8> = result.unwrap_borsh();
        Address::try_from_slice(&addr_bytes).unwrap()
    }

    fn nep_141_balance_of(
        account_id: &str,
        nep_141: &near_sdk::AccountId,
        aurora: &AuroraAccount,
    ) -> u128 {
        aurora
            .user
            .call(
                nep_141.clone(),
                "ft_balance_of",
                json!({ "account_id": account_id }).to_string().as_bytes(),
                near_sdk_sim::DEFAULT_GAS,
                0,
            )
            .unwrap_json_value()
            .as_str()
            .unwrap()
            .parse()
            .unwrap()
    }

    fn nep141_storage_deposit(
        account_id: &str,
        nep_141: &near_sdk::AccountId,
        aurora: &AuroraAccount,
    ) {
        let args = json!({
            "account_id": account_id,
        })
        .to_string();
        aurora
            .user
            .call(
                nep_141.clone(),
                "storage_deposit",
                args.as_bytes(),
                near_sdk_sim::DEFAULT_GAS,
                near_sdk_sim::STORAGE_AMOUNT,
            )
            .assert_success();
    }

    fn deploy_erc20_factory(
        factory_account_id: &str,
        locker_address: &str,
        aurora: &AuroraAccount,
    ) -> UserAccount {
        let contract_bytes = std::fs::read(TOKEN_FACTORY_PATH).unwrap();

        let contract_account = aurora.user.deploy(
            &contract_bytes,
            factory_account_id.parse().unwrap(),
            5 * near_sdk_sim::STORAGE_AMOUNT,
        );

        let init_args = json!({
            "aurora_account": aurora.contract.account_id().to_string(),
            "locker_address": locker_address,
        })
        .to_string();

        aurora
            .user
            .call(
                contract_account.account_id(),
                "new",
                init_args.as_bytes(),
                near_sdk_sim::DEFAULT_GAS,
                0,
            )
            .assert_success();

        contract_account
    }

    fn deploy_bridge_token(
        token: &str,
        factory: &UserAccount,
        aurora: &AuroraAccount,
    ) -> near_sdk::AccountId {
        let args = json!({
            "address": token,
        });

        aurora
            .user
            .call(
                factory.account_id(),
                "deploy_bridge_token",
                args.to_string().as_bytes(),
                near_sdk_sim::DEFAULT_GAS,
                BRIDGE_TOKEN_INIT_BALANCE * 2,
            )
            .assert_success();

        return format!("{}.{}", token, factory.account_id())
            .try_into()
            .unwrap();
    }

    struct TestExitToNearContext {
        erc20_locker: Address,
        erc20: ERC20,
        nep141: near_sdk::AccountId,
        ft_owner_address: Address,
        ft_owner: near_sdk_sim::UserAccount,
        aurora: AuroraAccount,
    }
}
