use crate::admin_controlled::{AdminControlled, PausedMask};
use crate::deposit_event::DepositedEvent;
use crate::engine::Engine;
use crate::fungible_token::{FungibleToken, FungibleTokenMetadata};
use crate::json::parse_json;
use crate::parameters::{
    BalanceOfCallArgs, BalanceOfEthCallArgs, FinishDepositCallArgs, InitCallArgs,
    NEP141FtOnTransferArgs, PauseEthConnectorCallArgs, ResolveTransferCallArgs,
    SetContractDataCallArgs, StorageBalanceOfCallArgs, StorageDepositCallArgs,
    StorageWithdrawCallArgs, TransferCallArgs, TransferCallCallArgs, WithdrawResult,
};
use crate::prelude::sdk::types::{ExpectUtf8, SdkUnwrap};
use crate::prelude::{
    format, sdk, str, validate_eth_address, AccountId, Address, Balance, BorshDeserialize,
    BorshSerialize, EthAddress, EthConnectorStorageId, Gas, KeyPrefix, PromiseResult, String,
    ToString, TryFrom, Vec, WithdrawCallArgs, ERR_FAILED_PARSE, ERR_INVALID_ETH_ADDRESS, H160,
    U256,
};
use crate::proof::Proof;
use aurora_engine_sdk::io::{StorageIntermediate, IO};

pub const ERR_NOT_ENOUGH_BALANCE_FOR_FEE: &str = "ERR_NOT_ENOUGH_BALANCE_FOR_FEE";
pub const NO_DEPOSIT: Balance = 0;
const GAS_FOR_FINISH_DEPOSIT: Gas = 50_000_000_000_000;
// Note: Is 40Tgas always enough?
const GAS_FOR_VERIFY_LOG_ENTRY: Gas = 40_000_000_000_000;

pub const UNPAUSE_ALL: PausedMask = 0;
pub const PAUSE_DEPOSIT: PausedMask = 1 << 0;
pub const PAUSE_WITHDRAW: PausedMask = 1 << 1;

#[derive(BorshSerialize, BorshDeserialize)]
pub struct EthConnectorContract<I: IO + Default> {
    contract: EthConnector,
    ft: FungibleToken<I>,
    paused_mask: PausedMask,
    io: I,
}

/// eth-connector specific data
#[derive(BorshSerialize, BorshDeserialize)]
pub struct EthConnector {
    pub prover_account: AccountId,
    pub eth_custodian_address: EthAddress,
}

/// Token message data
#[derive(BorshSerialize, BorshDeserialize)]
#[cfg_attr(not(target_arch = "wasm32"), derive(Debug))]
pub enum TokenMessageData {
    Near(AccountId),
    Eth {
        receiver_id: AccountId,
        message: String,
    },
}

/// On-transfer message
pub struct OnTransferMessageData {
    pub relayer: AccountId,
    pub recipient: EthAddress,
    pub fee: U256,
}

impl<I: IO + Default + Copy> EthConnectorContract<I> {
    pub fn get_instance(io: I) -> Self {
        Self {
            contract: Self::get_contract_data(&io, &EthConnectorStorageId::Contract),
            ft: Self::get_contract_data(&io, &EthConnectorStorageId::FungibleToken),
            paused_mask: Self::get_contract_data(&io, &EthConnectorStorageId::PausedMask),
            io,
        }
    }

    fn get_contract_key(suffix: &EthConnectorStorageId) -> Vec<u8> {
        crate::prelude::bytes_to_key(KeyPrefix::EthConnector, &[*suffix as u8])
    }

    fn get_contract_data<T: BorshDeserialize>(io: &I, suffix: &EthConnectorStorageId) -> T {
        io.read_storage(&Self::get_contract_key(suffix))
            .expect("Failed read storage")
            .to_value()
            .unwrap()
    }

    /// Init eth-connector contract specific data
    pub fn init_contract(mut io: I, args: InitCallArgs) {
        // Check is it already initialized
        assert!(
            !io.storage_has_key(&Self::get_contract_key(&EthConnectorStorageId::Contract)),
            "ERR_CONTRACT_INITIALIZED"
        );
        sdk::log!("[init contract]");

        let contract_data = Self::set_contract_data(
            &mut io,
            SetContractDataCallArgs {
                prover_account: args.prover_account,
                eth_custodian_address: args.eth_custodian_address,
                metadata: args.metadata,
            },
        );

        let current_account_id = sdk::current_account_id();
        let owner_id = AccountId::try_from(current_account_id).unwrap();
        let mut ft = FungibleToken::new();
        // Register FT account for current contract
        ft.internal_register_account(&owner_id);

        let paused_mask = UNPAUSE_ALL;
        io.write_borsh(
            &Self::get_contract_key(&EthConnectorStorageId::PausedMask),
            &paused_mask,
        );

        Self {
            contract: contract_data,
            ft,
            paused_mask,
            io,
        }
        .save_ft_contract();
    }

    /// Sets the contract data and returns it back
    pub fn set_contract_data(io: &mut I, args: SetContractDataCallArgs) -> EthConnector {
        // Get initial contract arguments
        let contract_data = EthConnector {
            prover_account: args.prover_account,
            eth_custodian_address: validate_eth_address(args.eth_custodian_address).sdk_unwrap(),
        };
        // Save eth-connector specific data
        io.write_borsh(
            &Self::get_contract_key(&EthConnectorStorageId::Contract),
            &contract_data,
        );

        io.write_borsh(
            &Self::get_contract_key(&EthConnectorStorageId::FungibleTokenMetadata),
            &args.metadata,
        );

        contract_data
    }

    /// Parse event message data for tokens
    fn parse_event_message(&self, message: &str) -> TokenMessageData {
        let data: Vec<_> = message.split(':').collect();
        assert!(data.len() < 3);
        let account_id = AccountId::try_from(data[0].as_bytes()).sdk_unwrap();
        if data.len() == 1 {
            TokenMessageData::Near(account_id)
        } else {
            TokenMessageData::Eth {
                receiver_id: account_id,
                message: data[1].into(),
            }
        }
    }

    /// Get on-transfer data from message
    fn parse_on_transfer_message(&self, message: &str) -> OnTransferMessageData {
        let data: Vec<_> = message.split(':').collect();
        assert_eq!(data.len(), 2);

        let msg = hex::decode(data[1]).expect(ERR_FAILED_PARSE);
        let mut fee: [u8; 32] = Default::default();
        assert_eq!(msg.len(), 52, "ERR_WRONG_MESSAGE_LENGTH");
        fee.copy_from_slice(&msg[..32]);
        let mut recipient: EthAddress = Default::default();
        recipient.copy_from_slice(&msg[32..52]);
        // Check account
        let account_id = AccountId::try_from(data[0].as_bytes()).sdk_unwrap();
        OnTransferMessageData {
            relayer: account_id,
            recipient,
            fee: U256::from_little_endian(&fee[..]),
        }
    }

    /// Prepare message for `ft_transfer_call` -> `ft_on_transfer`
    fn set_message_for_on_transfer(&self, fee: U256, message: String) -> String {
        use byte_slice_cast::AsByteSlice;

        // Relayer == predecessor
        let relayer_account_id = String::from_utf8(sdk::predecessor_account_id()).unwrap();
        let mut data = fee.as_byte_slice().to_vec();
        let address = if message.len() == 42 {
            message
                .strip_prefix("0x")
                .expect(ERR_INVALID_ETH_ADDRESS)
                .to_string()
        } else {
            message
        };
        let address_bytes = hex::decode(address).expect(ERR_FAILED_PARSE);
        data.extend(address_bytes);
        [relayer_account_id, hex::encode(data)].join(":")
    }

    /// Deposit all types of tokens
    pub fn deposit(&self, raw_proof: Vec<u8>) {
        self.assert_not_paused(PAUSE_DEPOSIT);

        sdk::log!("[Deposit tokens]");

        // Get incoming deposit arguments
        let proof: Proof = Proof::try_from_slice(&raw_proof).expect(ERR_FAILED_PARSE);
        // Fetch event data from Proof
        let event = DepositedEvent::from_log_entry_data(&proof.log_entry_data);

        sdk::log!(&format!(
            "Deposit started: from {} to recipient {:?} with amount: {:?} and fee {:?}",
            hex::encode(event.sender),
            event.recipient,
            event.amount.as_u128(),
            event.fee.as_u128()
        ));

        sdk::log!(&format!(
            "Event's address {}, custodian address {}",
            hex::encode(&event.eth_custodian_address),
            hex::encode(&self.contract.eth_custodian_address),
        ));

        assert_eq!(
            event.eth_custodian_address, self.contract.eth_custodian_address,
            "ERR_WRONG_EVENT_ADDRESS",
        );

        assert!(
            event.amount > event.fee,
            "{}",
            ERR_NOT_ENOUGH_BALANCE_FOR_FEE,
        );

        // Verify proof data with cross-contract call to prover account
        sdk::log!(&format!(
            "Deposit verify_log_entry for prover: {}",
            self.contract.prover_account,
        ));

        // Do not skip bridge call. This is only used for development and diagnostics.
        let skip_bridge_call = false.try_to_vec().unwrap();
        let mut proof_to_verify = raw_proof;
        proof_to_verify.extend(skip_bridge_call);
        let promise0 = sdk::promise_create(
            self.contract.prover_account.as_bytes(),
            b"verify_log_entry",
            &proof_to_verify,
            NO_DEPOSIT,
            GAS_FOR_VERIFY_LOG_ENTRY,
        );
        let predecessor_account_id = AccountId::try_from(sdk::predecessor_account_id()).unwrap();

        // Finalize deposit
        let data = match self.parse_event_message(&event.recipient) {
            // Deposit to NEAR accounts
            TokenMessageData::Near(account_id) => FinishDepositCallArgs {
                new_owner_id: account_id,
                amount: event.amount.as_u128(),
                proof_key: proof.get_key(),
                relayer_id: predecessor_account_id,
                fee: event.fee.as_u128(),
                msg: None,
            }
            .try_to_vec()
            .unwrap(),
            // Deposit to Eth accounts
            // fee is being minted in the `ft_on_transfer` callback method
            TokenMessageData::Eth {
                receiver_id,
                message,
            } => {
                // Transfer to self and then transfer ETH in `ft_on_transfer`
                // address - is NEAR account
                let transfer_data = TransferCallCallArgs {
                    receiver_id,
                    amount: event.amount.as_u128(),
                    memo: None,
                    msg: self.set_message_for_on_transfer(event.fee, message),
                }
                .try_to_vec()
                .unwrap();

                let current_account_id = AccountId::try_from(sdk::current_account_id()).unwrap();
                // Send to self - current account id
                FinishDepositCallArgs {
                    new_owner_id: current_account_id,
                    amount: event.amount.as_u128(),
                    proof_key: proof.get_key(),
                    relayer_id: predecessor_account_id,
                    fee: event.fee.as_u128(),
                    msg: Some(transfer_data),
                }
                .try_to_vec()
                .unwrap()
            }
        };

        let promise1 = sdk::promise_then(
            promise0,
            &sdk::current_account_id(),
            b"finish_deposit",
            &data[..],
            NO_DEPOSIT,
            GAS_FOR_FINISH_DEPOSIT,
        );
        sdk::promise_return(promise1);
    }

    /// Finish deposit (private method)
    /// NOTE: we should `record_proof` only after `mint` operation. The reason
    /// is that in this case we only calculate the amount to be credited but
    /// do not save it, however, if an error occurs during the calculation,
    /// this will happen before `record_proof`. After that contract will save.
    pub fn finish_deposit(&mut self, data: FinishDepositCallArgs) {
        sdk::log!(&format!("Finish deposit with the amount: {}", data.amount));
        assert_eq!(sdk::promise_results_count(), 1);

        // Check promise results
        let data0: Vec<u8> = match sdk::promise_result(0) {
            PromiseResult::Successful(x) => x,
            PromiseResult::Failed => sdk::panic_utf8(b"ERR_PROMISE_FAILED"),
            // This shouldn't be reachable
            PromiseResult::NotReady => sdk::panic_utf8(b"ERR_PROMISE_NOT_READY"),
        };
        sdk::log!("Check verification_success");
        let verification_success = bool::try_from_slice(&data0).unwrap();
        assert!(verification_success, "ERR_VERIFY_PROOF");

        // Mint tokens to recipient minus fee
        if let Some(msg) = data.msg {
            // Mint - calculate new balances
            self.mint_eth_on_near(data.new_owner_id, data.amount);
            // Store proof only after `mint` calculations
            self.record_proof(&data.proof_key);
            // Save new contract data
            self.save_ft_contract();
            let transfer_call_args = TransferCallCallArgs::try_from_slice(&msg).unwrap();
            self.ft_transfer_call(transfer_call_args);
        } else {
            // Mint - calculate new balances
            self.mint_eth_on_near(data.new_owner_id.clone(), data.amount - data.fee);
            self.mint_eth_on_near(data.relayer_id, data.fee);
            // Store proof only after `mint` calculations
            self.record_proof(&data.proof_key);
            // Save new contract data
            self.save_ft_contract();
        }
    }

    /// Internal ETH withdraw ETH logic
    pub(crate) fn internal_remove_eth(&mut self, address: &Address, amount: &U256) {
        self.burn_eth_on_aurora(address.0, amount.as_u128());
        self.save_ft_contract();
    }

    /// Record used proof as hash key
    fn record_proof(&mut self, key: &str) {
        sdk::log!(&format!("Record proof: {}", key));

        assert!(!self.check_used_event(key), "ERR_PROOF_EXIST");
        self.save_used_event(key);
    }

    ///  Mint nETH tokens
    fn mint_eth_on_near(&mut self, owner_id: AccountId, amount: Balance) {
        sdk::log!(&format!("Mint {} nETH tokens for: {}", amount, owner_id));

        if self.ft.accounts_get(&owner_id).is_none() {
            self.ft.accounts_insert(&owner_id, 0);
        }
        self.ft.internal_deposit_eth_to_near(&owner_id, amount);
    }

    ///  Mint ETH tokens
    fn mint_eth_on_aurora(&mut self, owner_id: EthAddress, amount: Balance) {
        sdk::log!(&format!(
            "Mint {} ETH tokens for: {}",
            amount,
            hex::encode(owner_id)
        ));
        self.ft.internal_deposit_eth_to_aurora(owner_id, amount);
    }

    /// Burn ETH tokens
    fn burn_eth_on_aurora(&mut self, address: EthAddress, amount: Balance) {
        sdk::log!(&format!(
            "Burn {} ETH tokens for: {}",
            amount,
            hex::encode(address)
        ));
        self.ft.internal_withdraw_eth_from_aurora(address, amount);
    }

    /// Withdraw nETH from NEAR accounts
    /// NOTE: it should be without any log data
    pub fn withdraw_eth_from_near(&mut self, args: WithdrawCallArgs) -> WithdrawResult {
        self.assert_not_paused(PAUSE_WITHDRAW);

        sdk::assert_one_yocto();
        // Burn tokens to recipient
        let predecessor_account_id = AccountId::try_from(sdk::predecessor_account_id()).unwrap();
        self.ft
            .internal_withdraw_eth_from_near(&predecessor_account_id, args.amount);
        // Save new contract data
        self.save_ft_contract();

        WithdrawResult {
            recipient_id: args.recipient_address,
            amount: args.amount,
            eth_custodian_address: self.contract.eth_custodian_address,
        }
    }

    /// Returns total ETH supply on NEAR (nETH as NEP-141 token)
    pub fn ft_total_eth_supply_on_near(&mut self) {
        let total_supply = self.ft.ft_total_eth_supply_on_near();
        sdk::log!(&format!("Total ETH supply on NEAR: {}", total_supply));
        self.io
            .return_output(format!("\"{}\"", total_supply.to_string()).as_bytes());
    }

    /// Returns total ETH supply on Aurora (ETH in Aurora EVM)
    pub fn ft_total_eth_supply_on_aurora(&mut self) {
        let total_supply = self.ft.ft_total_eth_supply_on_aurora();
        sdk::log!(&format!("Total ETH supply on Aurora: {}", total_supply));
        self.io
            .return_output(format!("\"{}\"", total_supply.to_string()).as_bytes());
    }

    /// Return balance of nETH (ETH on Near)
    pub fn ft_balance_of(&mut self) {
        let args = BalanceOfCallArgs::from(
            parse_json(&self.io.read_input().to_vec()).expect_utf8(ERR_FAILED_PARSE.as_bytes()),
        );

        let balance = self.ft.ft_balance_of(&args.account_id);
        sdk::log!(&format!(
            "Balance of nETH [{}]: {}",
            args.account_id, balance
        ));

        self.io
            .return_output(format!("\"{}\"", balance.to_string()).as_bytes());
    }

    /// Return balance of ETH (ETH in Aurora EVM)
    pub fn ft_balance_of_eth_on_aurora(&mut self) {
        let args: BalanceOfEthCallArgs = self.io.read_input().to_value().expect(ERR_FAILED_PARSE);
        let balance = self
            .ft
            .internal_unwrap_balance_of_eth_on_aurora(args.address);
        sdk::log!(&format!(
            "Balance of ETH [{}]: {}",
            hex::encode(args.address),
            balance
        ));
        self.io
            .return_output(format!("\"{}\"", balance.to_string()).as_bytes());
    }

    /// Transfer between NEAR accounts
    pub fn ft_transfer(&mut self) {
        sdk::assert_one_yocto();
        let args = TransferCallArgs::from(
            parse_json(&self.io.read_input().to_vec()).expect_utf8(ERR_FAILED_PARSE.as_bytes()),
        );
        self.ft
            .ft_transfer(&args.receiver_id, args.amount, &args.memo);
        self.save_ft_contract();
        sdk::log!(&format!(
            "Transfer amount {} to {} success with memo: {:?}",
            args.amount, args.receiver_id, args.memo
        ));
    }

    /// FT resolve transfer logic
    pub fn ft_resolve_transfer(&mut self) {
        sdk::assert_private_call();
        // Check if previous promise succeeded
        assert_eq!(sdk::promise_results_count(), 1);

        let args: ResolveTransferCallArgs = self.io.read_input().to_value().unwrap();
        let amount = self
            .ft
            .ft_resolve_transfer(&args.sender_id, &args.receiver_id, args.amount);
        sdk::log!(&format!(
            "Resolve transfer from {} to {} success",
            args.sender_id, args.receiver_id
        ));
        // `ft_resolve_transfer` can change `total_supply` so we should save the contract
        self.save_ft_contract();
        self.io
            .return_output(format!("\"{}\"", amount.to_string()).as_bytes());
    }

    /// FT transfer call from sender account (invoker account) to receiver
    /// We starting early checking for message data to avoid `ft_on_transfer` call panics
    /// But we don't check relayer exists. If relayer doesn't exist we simply not mint/burn the amount of the fee
    pub fn ft_transfer_call(&mut self, args: TransferCallCallArgs) {
        sdk::log!(&format!(
            "Transfer call to {} amount {}",
            args.receiver_id, args.amount,
        ));
        // Verify message data before `ft_on_transfer` call to avoid verification panics
        if args.receiver_id.as_bytes() == &sdk::current_account_id()[..] {
            let message_data = self.parse_on_transfer_message(&args.msg);
            // Check is transfer amount > fee
            assert!(
                args.amount > message_data.fee.as_u128(),
                "{}",
                ERR_NOT_ENOUGH_BALANCE_FOR_FEE,
            );

            // Additional check overflow before process `ft_on_transfer`
            // But don't check overflow for relayer
            // Note: It can't overflow because the total supply doesn't change during transfer.
            let amount_for_check = self
                .ft
                .internal_unwrap_balance_of_eth_on_aurora(message_data.recipient);
            assert!(amount_for_check.checked_add(args.amount).is_some());

            // It'll call only on Aurora contract
            assert!(self
                .ft
                .total_eth_supply_on_aurora
                .checked_add(args.amount)
                .is_some());
        }

        self.ft
            .ft_transfer_call(&args.receiver_id, args.amount, &args.memo, args.msg);
    }

    /// FT storage deposit logic
    pub fn storage_deposit(&mut self) {
        let args = StorageDepositCallArgs::from(
            parse_json(&self.io.read_input().to_vec()).expect_utf8(ERR_FAILED_PARSE.as_bytes()),
        );

        let res = self
            .ft
            .storage_deposit(args.account_id.as_ref(), args.registration_only);
        self.save_ft_contract();
        self.io.return_output(&res.to_json_bytes());
    }

    /// FT storage withdraw
    pub fn storage_withdraw(&mut self) {
        sdk::assert_one_yocto();
        let args = StorageWithdrawCallArgs::from(
            parse_json(&self.io.read_input().to_vec()).expect_utf8(ERR_FAILED_PARSE.as_bytes()),
        );
        let res = self.ft.storage_withdraw(args.amount);
        self.save_ft_contract();
        self.io.return_output(&res.to_json_bytes());
    }

    /// Get balance of storage
    pub fn storage_balance_of(&mut self) {
        let args = StorageBalanceOfCallArgs::from(
            parse_json(&self.io.read_input().to_vec()).expect_utf8(ERR_FAILED_PARSE.as_bytes()),
        );
        self.io
            .return_output(&self.ft.storage_balance_of(&args.account_id).to_json_bytes());
    }

    /// ft_on_transfer callback function
    pub fn ft_on_transfer(&mut self, engine: &Engine<I>, args: &NEP141FtOnTransferArgs) {
        sdk::log!("Call ft_on_transfer");
        // Parse message with specific rules
        let message_data = self.parse_on_transfer_message(&args.msg);

        // Special case when predecessor_account_id is current_account_id
        let fee = message_data.fee.as_u128();
        // Mint fee to relayer
        let relayer = engine.get_relayer(message_data.relayer.as_bytes());
        match (fee, relayer) {
            (fee, Some(H160(evm_relayer_address))) if fee > 0 => {
                self.mint_eth_on_aurora(message_data.recipient, args.amount - fee);
                self.mint_eth_on_aurora(evm_relayer_address, fee);
            }
            _ => self.mint_eth_on_aurora(message_data.recipient, args.amount),
        }
        self.save_ft_contract();
        self.io.return_output("\"0\"".as_bytes());
    }

    /// Get accounts counter for statistics.
    /// It represents total unique accounts (all-time, including accounts which now have zero balance).
    pub fn get_accounts_counter(&mut self) {
        self.io
            .return_output(&self.ft.get_accounts_counter().to_le_bytes());
    }

    /// Save eth-connector contract data
    fn save_ft_contract(&mut self) {
        self.io.write_borsh(
            &Self::get_contract_key(&EthConnectorStorageId::FungibleToken),
            &self.ft,
        );
    }

    /// Generate key for used events from Prood
    fn used_event_key(&self, key: &str) -> Vec<u8> {
        let mut v = Self::get_contract_key(&EthConnectorStorageId::UsedEvent).to_vec();
        v.extend_from_slice(key.as_bytes());
        v
    }

    /// Save already used event proof as hash key
    fn save_used_event(&mut self, key: &str) {
        self.io.write_borsh(&self.used_event_key(key), &0u8);
    }

    /// Check is event of proof already used
    fn check_used_event(&self, key: &str) -> bool {
        self.io.storage_has_key(&self.used_event_key(key))
    }

    /// Checks whether the provided proof was already used
    pub fn is_used_proof(&self, proof: Proof) -> bool {
        self.check_used_event(&proof.get_key())
    }

    /// Get Eth connector paused flags
    pub fn get_paused_flags(&self) -> PausedMask {
        self.get_paused()
    }

    /// Set Eth connector paused flags
    pub fn set_paused_flags(&mut self, args: PauseEthConnectorCallArgs) {
        self.set_paused(args.paused_mask);
    }

    /// Return metdata
    pub fn get_metadata(io: &I) -> Option<FungibleTokenMetadata> {
        io.read_storage(&Self::get_contract_key(
            &EthConnectorStorageId::FungibleTokenMetadata,
        ))
        .and_then(|data| data.to_value().ok())
    }
}

impl<I: IO + Default + Copy> AdminControlled for EthConnectorContract<I> {
    fn get_paused(&self) -> PausedMask {
        self.paused_mask
    }

    fn set_paused(&mut self, paused_mask: PausedMask) {
        self.paused_mask = paused_mask;
        self.io.write_borsh(
            &Self::get_contract_key(&EthConnectorStorageId::PausedMask),
            &self.paused_mask,
        );
    }
}
