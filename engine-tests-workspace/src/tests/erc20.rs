use crate::contracts::erc20::{ERC20Constructor, ERC20};
use crate::runner::EvmContract;
use crate::signer::Signer;
use crate::test_utils::{self, solidity::DeployedContract};
use aurora_engine::parameters::{TransactionStatus, ViewCallArgs};
use aurora_engine_transactions::legacy::TransactionLegacy;
use aurora_engine_transactions::NormalizedEthTransaction;
use aurora_engine_types::types::{Address, Wei};
use aurora_engine_types::U256;

const INITIAL_NONCE: u64 = 0;
const INITIAL_BALANCE: u64 = 1_000_000;

/// Tests the ability to mint 10 tokens to the contract address.
#[tokio::test]
async fn erc20_mint() -> anyhow::Result<()> {
    let (evm_contract, mut signer, erc20_contract) = init().await?;
    let contract_address = erc20_contract.0.address;
    let signer_address = signer.address();

    // Validate pre-state
    assert_eq!(
        U256::zero(),
        get_address_erc20_balance(
            &evm_contract,
            signer_address,
            contract_address,
            &erc20_contract
        )
        .await?
    );

    // Do mint transaction
    let mint_amount: U256 = 10u64.into();
    let mint_tx = erc20_contract.mint(contract_address, mint_amount, signer.use_nonce().into());
    let signed_mint_tx = signer.sign_tx(mint_tx);

    let submit_result = evm_contract
        .submit(rlp::encode(&signed_mint_tx).to_vec())
        .await?;
    assert!(submit_result.status.is_ok());

    // Validate post-state
    assert_eq!(
        mint_amount,
        get_address_erc20_balance(
            &evm_contract,
            signer_address,
            contract_address,
            &erc20_contract
        )
        .await?
    );

    Ok(())
}

#[tokio::test]
async fn erc20_mint_out_of_gas() -> anyhow::Result<()> {
    let (evm_contract, mut signer, erc20_contract) = init().await?;
    let contract_address = erc20_contract.0.address;

    // Validate pre-state
    assert_eq!(
        U256::zero(),
        get_address_erc20_balance(
            &evm_contract,
            signer.address(),
            contract_address,
            &erc20_contract
        )
        .await?
    );

    let mint_amount: U256 = rand::random::<u64>().into();
    let mut mint_tx = erc20_contract.mint(contract_address, mint_amount, signer.use_nonce().into());
    let intrinsic_gas = {
        let signed_mint_tx = signer.sign_tx(mint_tx.clone());
        let normalized: NormalizedEthTransaction = signed_mint_tx.try_into()?;
        normalized.intrinsic_gas(&evm::Config::london())?
    };
    mint_tx.gas_limit = (intrinsic_gas - 1).into();
    let signed_mint_tx = signer.sign_tx(mint_tx.clone());
    let submit_result = evm_contract
        .submit(rlp::encode(&signed_mint_tx).to_vec())
        .await;
    // TODO get actual error. See: near/workspaces-rs/issues/191
    // TODO ERR_INTRINSIC_GAS check
    assert!(submit_result.is_err());

    // Validate post-state
    test_utils::validate_address_balance_and_nonce(
        &evm_contract,
        signer.address(),
        Wei::new_u64(INITIAL_BALANCE),
        (INITIAL_NONCE + 1).into(),
    )
    .await;

    const GAS_LIMIT: u64 = 67_000;
    const GAS_PRICE: u64 = 10;
    mint_tx.gas_limit = U256::from(GAS_LIMIT);
    mint_tx.gas_price = U256::from(GAS_PRICE);
    let signed_mint_tx = signer.sign_tx(mint_tx.clone());
    let submit_result = evm_contract
        .submit(rlp::encode(&signed_mint_tx).to_vec())
        .await;
    assert_eq!(submit_result.unwrap().status, TransactionStatus::OutOfGas);

    // Validate post-state
    test_utils::validate_address_balance_and_nonce(
        &evm_contract,
        signer.address(),
        Wei::new_u64(INITIAL_BALANCE - GAS_LIMIT * GAS_PRICE),
        (INITIAL_NONCE + 2).into(),
    )
    .await;

    Ok(())
}

// TODO
// #[test]
// fn profile_erc20_get_balance() {}

#[tokio::test]
async fn erc20_transfer_success() -> anyhow::Result<()> {
    let (evm_contract, mut signer, erc20_contract) = init().await?;
    let contract_address = erc20_contract.0.address;
    let signer_address = signer.address();

    // Validate pre-state
    assert_eq!(
        U256::zero(),
        get_address_erc20_balance(
            &evm_contract,
            signer_address,
            signer_address,
            &erc20_contract
        )
        .await?
    );

    let mint_tx = erc20_contract.mint(
        signer_address,
        INITIAL_BALANCE.into(),
        signer.use_nonce().into(),
    );
    let signed_mint_tx = signer.sign_tx(mint_tx);
    let submit_result = evm_contract
        .submit(rlp::encode(&signed_mint_tx).to_vec())
        .await;
    println!("{submit_result:#?}");
    assert!(submit_result.is_ok());

    assert_eq!(
        U256::from(INITIAL_BALANCE),
        get_address_erc20_balance(
            &evm_contract,
            signer_address,
            signer_address,
            &erc20_contract
        )
        .await?
    );
    assert_eq!(
        U256::zero(),
        get_address_erc20_balance(
            &evm_contract,
            signer.address(),
            contract_address,
            &erc20_contract
        )
        .await?
    );

    Ok(())
}

async fn get_address_erc20_balance(
    evm_contract: &EvmContract,
    sender: Address,
    balance_of: Address,
    contract: &ERC20,
) -> anyhow::Result<U256> {
    let tx = contract.balance_of(balance_of, U256::zero());
    let result = evm_contract
        .view(ViewCallArgs {
            sender,
            address: tx.to.unwrap(),
            amount: tx.value.to_bytes(),
            input: tx.data,
        })
        .await?
        .unwrap();

    Ok(U256::from_big_endian(&result))
}

async fn init() -> anyhow::Result<(EvmContract, Signer, ERC20)> {
    let mut evm_contract = EvmContract::new().await?;

    let mut signer = Signer::random();
    let signer_address = signer.address();

    evm_contract
        .mint_account(signer_address, INITIAL_NONCE, INITIAL_BALANCE)
        .await?;

    signer.nonce = INITIAL_NONCE;
    let constructor = ERC20Constructor::load();
    let data = constructor.deploy("Erc20", "ERC");
    let tx = TransactionLegacy {
        nonce: signer.use_nonce().into(),
        gas_limit: u64::MAX.into(),
        data,
        ..TransactionLegacy::default()
    };
    let signed_tx = signer.sign_tx(tx);

    let submit_result = evm_contract
        .submit(rlp::encode(&signed_tx).to_vec())
        .await?;
    let contract_address = Address::try_from_slice(&submit_result.unwrap())?;

    let erc20_contract = ERC20(DeployedContract {
        address: contract_address,
        abi: constructor.0.abi,
    });

    Ok((evm_contract, signer, erc20_contract))
}
