use crate::utils::*;
use aurora_engine::deposit_event::{DepositedEvent, TokenMessageData, DEPOSITED_EVENT};
use aurora_engine::{log_entry, parameters::WithdrawResult, proof::Proof};
use aurora_engine_types::{
    types::{Address, Fee, NEP141Wei},
    H256, U256,
};
use byte_slice_cast::AsByteSlice;
use near_sdk::{json_types::U128, ONE_YOCTO};
use workspaces::AccountId;

/// Bytes for a NEAR smart contract implementing `ft_on_transfer`
fn dummy_ft_receiver_bytes() -> Vec<u8> {
    let base_path = std::path::Path::new("../etc")
        .join("tests")
        .join("ft-receiver");
    let output_path = base_path.join("target/wasm32-unknown-unknown/release/ft_receiver.wasm");
    crate::rust::compile(base_path);
    std::fs::read(output_path).unwrap()
}

#[tokio::test]
async fn test_aurora_ft_transfer() -> anyhow::Result<()> {
    let contract = TestContract::new().await?;
    let proof = contract.get_proof(PROOF_DATA_NEAR);
    let res = contract
        .eth_connector_contract
        .call("deposit")
        .args_borsh(proof)
        .gas(DEFAULT_GAS)
        .transact()
        .await?;
    assert!(res.is_success());

    let transfer_amount = 70;
    let receiver_id = AccountId::try_from(DEPOSITED_RECIPIENT.to_string()).unwrap();
    let res = contract
        .eth_connector_contract
        .call("ft_transfer")
        .args_json((&receiver_id, transfer_amount.to_string(), "transfer memo"))
        .gas(DEFAULT_GAS)
        .deposit(ONE_YOCTO)
        .transact()
        .await?;
    assert!(res.is_success());

    let balance = contract
        .eth_connector_contract
        .call("ft_balance_of")
        .args_json((&receiver_id,))
        .view()
        .await?
        .json::<U128>()
        .unwrap();
    assert_eq!(
        balance.0,
        DEPOSITED_AMOUNT - DEPOSITED_FEE + transfer_amount as u128
    );

    let balance = contract
        .eth_connector_contract
        .call("ft_balance_of")
        .args_json((&contract.eth_connector_contract.id(),))
        .view()
        .await?
        .json::<U128>()
        .unwrap();
    assert_eq!(balance.0, DEPOSITED_FEE - transfer_amount as u128);

    let balance = contract
        .eth_connector_contract
        .call("ft_total_supply")
        .view()
        .await?
        .json::<U128>()
        .unwrap();
    assert_eq!(balance.0, DEPOSITED_AMOUNT);

    let balance = contract
        .eth_connector_contract
        .call("ft_total_eth_supply_on_near")
        .view()
        .await?
        .json::<U128>()
        .unwrap();
    assert_eq!(balance.0, DEPOSITED_AMOUNT);
    Ok(())
}

#[tokio::test]
async fn test_ft_transfer() -> anyhow::Result<()> {
    let contract = TestContract::new().await?;
    contract.call_deposit_eth_to_near().await?;

    let transfer_amount = 70;
    let receiver_id = AccountId::try_from(DEPOSITED_RECIPIENT.to_string()).unwrap();
    let res = contract
        .engine_contract
        .call("ft_transfer")
        .args_json((&receiver_id, transfer_amount.to_string(), "transfer memo"))
        .gas(DEFAULT_GAS)
        .deposit(ONE_YOCTO)
        .transact()
        .await?;
    assert!(res.is_success());

    assert_eq!(
        contract.get_eth_on_near_balance(&receiver_id).await?.0,
        DEPOSITED_AMOUNT - DEPOSITED_FEE + transfer_amount as u128,
    );
    assert_eq!(
        contract
            .get_eth_on_near_balance(&contract.engine_contract.id())
            .await?
            .0,
        DEPOSITED_FEE - transfer_amount as u128,
    );
    assert_eq!(
        contract.total_eth_supply_on_near().await?.0,
        DEPOSITED_AMOUNT,
    );
    assert_eq!(DEPOSITED_AMOUNT, contract.total_supply().await?.0);
    Ok(())
}

#[tokio::test]
async fn test_withdraw_eth_from_near() -> anyhow::Result<()> {
    let contract = TestContract::new().await?;
    contract.call_deposit_eth_to_near().await?;

    let withdraw_amount = NEP141Wei::new(100);
    let recipient_addr = validate_eth_address(RECIPIENT_ETH_ADDRESS);
    let receiver_id = AccountId::try_from(DEPOSITED_RECIPIENT.to_string()).unwrap();
    let res = contract
        .engine_contract
        .call("withdraw")
        .args_borsh((recipient_addr, withdraw_amount))
        .gas(DEFAULT_GAS)
        .deposit(ONE_YOCTO)
        .transact()
        .await?;
    assert!(res.is_success());

    let data: WithdrawResult = res.borsh()?;
    let custodian_addr = validate_eth_address(CUSTODIAN_ADDRESS);
    assert_eq!(data.recipient_id, recipient_addr);
    assert_eq!(data.amount, withdraw_amount);
    assert_eq!(data.eth_custodian_address, custodian_addr);

    assert_eq!(
        contract
            .get_eth_on_near_balance(&&contract.engine_contract.id())
            .await?
            .0,
        DEPOSITED_FEE - withdraw_amount.as_u128(),
    );
    assert_eq!(
        contract.get_eth_on_near_balance(&receiver_id).await?.0,
        DEPOSITED_AMOUNT - DEPOSITED_FEE
    );
    assert_eq!(
        contract.total_supply().await?.0,
        DEPOSITED_AMOUNT - withdraw_amount.as_u128(),
    );
    Ok(())
}

#[tokio::test]
async fn test_deposit_eth_to_near_balance_total_supply() -> anyhow::Result<()> {
    let contract = TestContract::new().await?;
    contract.call_deposit_eth_to_near().await?;
    assert!(
        contract.call_is_used_proof(PROOF_DATA_NEAR).await?,
        "Expected not to fail because the proof should have been already used",
    );

    let receiver_id = AccountId::try_from(DEPOSITED_RECIPIENT.to_string()).unwrap();
    assert_eq!(
        contract
            .get_eth_on_near_balance(&contract.engine_contract.id())
            .await?
            .0,
        DEPOSITED_FEE
    );
    assert_eq!(
        contract.get_eth_on_near_balance(&receiver_id).await?.0,
        DEPOSITED_AMOUNT - DEPOSITED_FEE
    );
    assert_eq!(
        contract.total_eth_supply_on_near().await?.0,
        DEPOSITED_AMOUNT,
    );
    assert_eq!(contract.total_supply().await?.0, DEPOSITED_AMOUNT);
    Ok(())
}

// NOTE: We don't test relayer fee
#[tokio::test]
async fn test_deposit_eth_to_aurora_balance_total_supply() -> anyhow::Result<()> {
    let contract = TestContract::new().await?;
    contract.call_deposit_eth_to_aurora().await?;
    assert!(
        contract.call_is_used_proof(PROOF_DATA_ETH).await?,
        "Expected not to fail because the proof should have been already used",
    );

    assert_eq!(
        contract
            .get_eth_balance(&validate_eth_address(RECIPIENT_ETH_ADDRESS),)
            .await?,
        DEPOSITED_EVM_AMOUNT
    );
    assert_eq!(
        contract.total_eth_supply_on_near().await?.0,
        DEPOSITED_EVM_AMOUNT,
    );
    assert_eq!(contract.total_supply().await?.0, DEPOSITED_EVM_AMOUNT,);
    Ok(())
}

#[tokio::test]
async fn test_ft_transfer_call_eth() -> anyhow::Result<()> {
    let contract = TestContract::new().await?;
    contract.call_deposit_eth_to_near().await?;

    let receiver_id = AccountId::try_from(DEPOSITED_RECIPIENT.to_string()).unwrap();
    assert_eq!(
        contract.get_eth_on_near_balance(&receiver_id).await?.0,
        DEPOSITED_AMOUNT - DEPOSITED_FEE,
    );
    assert_eq!(
        contract
            .get_eth_on_near_balance(&contract.engine_contract.id())
            .await?
            .0,
        DEPOSITED_FEE,
    );

    let transfer_amount: U128 = 50.into();
    let fee: u128 = 30;
    let mut msg = U256::from(fee).as_byte_slice().to_vec();
    msg.append(
        &mut validate_eth_address(RECIPIENT_ETH_ADDRESS)
            .as_bytes()
            .to_vec(),
    );

    let message = [CONTRACT_ACC, hex::encode(msg).as_str()].join(":");
    let memo: Option<String> = None;
    let res = contract
        .engine_contract
        .call("ft_transfer_call")
        .args_json((
            contract.engine_contract.id(),
            transfer_amount,
            memo,
            message,
        ))
        .gas(DEFAULT_GAS)
        .deposit(ONE_YOCTO)
        .transact()
        .await?;
    assert!(res.is_success());

    let receiver_id = AccountId::try_from(DEPOSITED_RECIPIENT.to_string()).unwrap();
    assert_eq!(
        contract.get_eth_on_near_balance(&receiver_id).await?.0,
        DEPOSITED_AMOUNT - DEPOSITED_FEE,
    );
    assert_eq!(
        contract
            .get_eth_on_near_balance(&contract.eth_connector_contract.id())
            .await?
            .0,
        0,
    );
    assert_eq!(
        contract
            .get_eth_on_near_balance(&contract.engine_contract.id())
            .await?
            .0,
        DEPOSITED_FEE,
    );
    assert_eq!(
        contract
            .get_eth_balance(&validate_eth_address(RECIPIENT_ETH_ADDRESS),)
            .await?,
        transfer_amount.0,
    );
    assert_eq!(
        contract.total_eth_supply_on_near().await?.0,
        DEPOSITED_AMOUNT,
    );
    assert_eq!(contract.total_supply().await?.0, DEPOSITED_AMOUNT);
    Ok(())
}

#[tokio::test]
async fn test_ft_transfer_call_without_message() -> anyhow::Result<()> {
    let contract = TestContract::new().await?;
    contract.call_deposit_eth_to_near().await?;

    let receiver_id = AccountId::try_from(DEPOSITED_RECIPIENT.to_string()).unwrap();
    assert_eq!(
        contract.get_eth_on_near_balance(&receiver_id).await?.0,
        DEPOSITED_AMOUNT - DEPOSITED_FEE,
    );
    assert_eq!(
        contract
            .get_eth_on_near_balance(&contract.engine_contract.id())
            .await?
            .0,
        DEPOSITED_FEE,
    );
    assert_eq!(
        contract
            .get_eth_on_near_balance(&contract.eth_connector_contract.id())
            .await?
            .0,
        0,
    );

    let transfer_amount: U128 = 50.into();
    let memo: Option<String> = None;
    let message = "";
    // Send to Aurora contract with wrong message should failed
    let res = contract
        .engine_contract
        .call("ft_transfer_call")
        .args_json((
            contract.engine_contract.id(),
            transfer_amount,
            &memo,
            message,
        ))
        .gas(DEFAULT_GAS)
        .deposit(ONE_YOCTO)
        .transact()
        .await?;
    assert!(contract.check_error_message(res, "ERR_INVALID_ON_TRANSFER_MESSAGE_FORMAT"));

    // Assert balances remain unchanged
    assert_eq!(
        contract.get_eth_on_near_balance(&receiver_id).await?.0,
        DEPOSITED_AMOUNT - DEPOSITED_FEE
    );
    assert_eq!(
        contract
            .get_eth_on_near_balance(&contract.engine_contract.id())
            .await?
            .0,
        DEPOSITED_FEE
    );
    assert_eq!(
        contract
            .get_eth_on_near_balance(&contract.eth_connector_contract.id())
            .await?
            .0,
        0
    );

    // Sending to random account should not change balances
    let some_acc = AccountId::try_from("some-test-acc".to_string()).unwrap();
    let res = contract
        .engine_contract
        .call("ft_transfer_call")
        .args_json((&some_acc, transfer_amount, &memo, message))
        .gas(DEFAULT_GAS)
        .deposit(ONE_YOCTO)
        .transact()
        .await?;
    assert!(res.is_success());

    // some-test-acc does not implement `ft_on_transfer` therefore the call fails and the transfer is reverted.
    assert_eq!(
        contract.get_eth_on_near_balance(&receiver_id).await?.0,
        DEPOSITED_AMOUNT - DEPOSITED_FEE
    );
    assert_eq!(contract.get_eth_on_near_balance(&some_acc).await?.0, 0);
    assert_eq!(
        contract
            .get_eth_on_near_balance(&contract.engine_contract.id())
            .await?
            .0,
        DEPOSITED_FEE
    );
    assert_eq!(
        contract
            .get_eth_on_near_balance(&contract.eth_connector_contract.id())
            .await?
            .0,
        0
    );

    let dummy_contract = contract
        .create_sub_account("ft-rec")
        .await?
        .deploy(&dummy_ft_receiver_bytes()[..])
        .await?
        .into_result()?;

    // Sending to external receiver with empty message should be success
    let res = contract
        .engine_contract
        .call("ft_transfer_call")
        .args_json((&dummy_contract.id(), transfer_amount, &memo, message))
        .gas(DEFAULT_GAS)
        .deposit(ONE_YOCTO)
        .transact()
        .await?;
    assert!(res.is_success());

    assert_eq!(
        contract.get_eth_on_near_balance(&receiver_id).await?.0,
        DEPOSITED_AMOUNT - DEPOSITED_FEE
    );
    assert_eq!(
        contract
            .get_eth_on_near_balance(&dummy_contract.id())
            .await?
            .0,
        transfer_amount.0
    );
    assert_eq!(
        contract
            .get_eth_on_near_balance(&contract.engine_contract.id())
            .await?
            .0,
        DEPOSITED_FEE - transfer_amount.0
    );
    assert_eq!(
        contract
            .get_eth_on_near_balance(&contract.eth_connector_contract.id())
            .await?
            .0,
        0
    );
    assert_eq!(
        contract
            .get_eth_balance(&validate_eth_address(RECIPIENT_ETH_ADDRESS))
            .await?,
        0
    );
    assert_eq!(contract.total_supply().await?.0, DEPOSITED_AMOUNT);
    assert_eq!(
        contract.total_eth_supply_on_near().await?.0,
        DEPOSITED_AMOUNT
    );
    Ok(())
}

#[tokio::test]
async fn test_deposit_with_0x_prefix() -> anyhow::Result<()> {
    let contract = TestContract::new().await?;

    let eth_custodian_address: Address = Address::decode(&CUSTODIAN_ADDRESS.to_string()).unwrap();
    let recipient_address = Address::from_array([10u8; 20]);
    let deposit_amount = 17;
    let recipient_address_encoded = recipient_address.encode();

    // Note the 0x prefix before the deposit address.
    let message = [CONTRACT_ACC, ":", "0x", &recipient_address_encoded].concat();
    let fee: Fee = Fee::new(NEP141Wei::new(0));
    let token_message_data =
        TokenMessageData::parse_event_message_and_prepare_token_message_data(&message, fee)
            .unwrap();

    let deposit_event = DepositedEvent {
        eth_custodian_address,
        sender: Address::zero(),
        token_message_data,
        amount: NEP141Wei::new(deposit_amount),
        fee,
    };

    let event_schema = ethabi::Event {
        name: DEPOSITED_EVENT.into(),
        inputs: DepositedEvent::event_params(),
        anonymous: false,
    };
    let log_entry = log_entry::LogEntry {
        address: eth_custodian_address.raw(),
        topics: vec![
            event_schema.signature(),
            // the sender is not important
            H256::zero(),
        ],
        data: ethabi::encode(&[
            ethabi::Token::String(message),
            ethabi::Token::Uint(U256::from(deposit_event.amount.as_u128())),
            ethabi::Token::Uint(U256::from(deposit_event.fee.as_u128())),
        ]),
    };
    let proof = Proof {
        log_index: 1,
        // Only this field matters for the purpose of this test
        log_entry_data: rlp::encode(&log_entry).to_vec(),
        receipt_index: 1,
        receipt_data: Vec::new(),
        header_data: Vec::new(),
        proof: Vec::new(),
    };

    let res = contract.deposit_with_proof(&proof).await?;
    assert!(res.is_success());

    let balance = contract
        .get_eth_on_near_balance(&contract.engine_contract.id())
        .await?;
    assert_eq!(balance.0, deposit_amount);

    let balance = contract.get_eth_balance(&recipient_address).await?;
    assert_eq!(balance, deposit_amount);
    Ok(())
}

#[tokio::test]
async fn test_deposit_with_same_proof() -> anyhow::Result<()> {
    let contract = TestContract::new().await?;
    assert!(!contract.call_is_used_proof(PROOF_DATA_NEAR).await?);
    contract.call_deposit_eth_to_near().await?;
    assert!(contract.call_is_used_proof(PROOF_DATA_NEAR).await?);

    let res = contract
        .deposit_with_proof(&contract.get_proof(PROOF_DATA_NEAR))
        .await?;
    assert!(res.is_failure());
    assert!(contract.check_error_message(res, "ERR_PROOF_EXIST"));
    Ok(())
}

#[tokio::test]
async fn test_deposit_wrong_custodian_address() -> anyhow::Result<()> {
    let contract =
        TestContract::new_with_custodian("0000000000000000000000000000000000000001").await?;
    let res = contract
        .deposit_with_proof(&contract.get_proof(PROOF_DATA_NEAR))
        .await?;
    assert!(res.is_failure());
    assert!(contract.check_error_message(res, "ERR_WRONG_EVENT_ADDRESS"));
    assert!(!contract.call_is_used_proof(PROOF_DATA_NEAR).await?);
    Ok(())
}
