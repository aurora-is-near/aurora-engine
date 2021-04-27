use crate::fungible_token::*;
use crate::parameters::*;
use crate::sdk;
use crate::types::*;

use crate::deposit_event::*;
use crate::prelude::{Address, U256};
use crate::prover::validate_eth_address;
#[cfg(feature = "log")]
use alloc::format;
use alloc::{
    string::{String, ToString},
    vec::Vec,
};
use borsh::{BorshDeserialize, BorshSerialize};

pub const CONTRACT_NAME_KEY: &str = "EthConnector";
pub const EVM_TOKEN_NAME_KEY: &str = "evt";
pub const EVM_RELAYER_NAME_KEY: &str = "rel";
pub const CONTRACT_FT_KEY: &str = "EthConnector.ft";
pub const NO_DEPOSIT: Balance = 0;
const GAS_FOR_FINISH_DEPOSIT: Gas = 10_000_000_000_000;
const GAS_FOR_VERIFY_LOG_ENTRY: Gas = 40_000_000_000_000;

#[derive(BorshSerialize, BorshDeserialize)]
pub struct EthConnectorContract {
    contract: EthConnector,
    ft: FungibleToken,
}

/// eth-connector specific data
#[derive(BorshSerialize, BorshDeserialize)]
pub struct EthConnector {
    pub prover_account: AccountId,
    pub eth_custodian_address: EthAddress,
}

/// Token message data
#[derive(BorshSerialize, BorshDeserialize)]
pub enum TokenMessageData {
    Near(AccountId),
    Eth { address: AccountId, message: String },
}

/// On-transfer message
pub struct OnTrasnferMessageData {
    pub relayer: AccountId,
    pub recipient: EthAddress,
    pub fee: U256,
}

impl EthConnectorContract {
    pub fn new() -> Self {
        Self {
            contract: sdk::get_contract_data(CONTRACT_NAME_KEY),
            ft: sdk::get_contract_data(CONTRACT_FT_KEY),
        }
    }

    /// Init eth-connector contract specific data
    pub fn init_contract() {
        // Check is it already initialized
        assert!(
            !sdk::storage_has_key(CONTRACT_NAME_KEY.as_bytes()),
            "ERR_CONTRACT_INITIALIZED"
        );
        #[cfg(feature = "log")]
        sdk::log("[init contract]".into());
        // Get initial contract arguments
        let args = InitCallArgs::try_from_slice(&sdk::read_input()[..]).expect(ERR_FAILED_PARSE);
        let current_account_id = sdk::current_account_id();
        let owner_id = String::from_utf8(current_account_id).unwrap();
        let mut ft = FungibleToken::new();
        // Register FT account for current contract
        ft.internal_register_account(&owner_id);
        let contract_data = EthConnector {
            prover_account: args.prover_account,
            eth_custodian_address: validate_eth_address(args.eth_custodian_address),
        };
        Self {
            contract: contract_data,
            ft,
        }
        .save_contract();
    }

    /// Parse event message data for tokens
    fn parse_event_message(&self, message: &str) -> TokenMessageData {
        let data: Vec<_> = message.split(':').collect();
        assert!(data.len() < 3);
        if data.len() == 1 {
            TokenMessageData::Near(data[0].into())
        } else {
            TokenMessageData::Eth {
                address: data[0].into(),
                message: data[1].into(),
            }
        }
    }

    /// Get on-transfer data from message
    fn parse_on_transfer_message(&self, message: &str) -> OnTrasnferMessageData {
        let data: Vec<_> = message.split(':').collect();
        assert_eq!(data.len(), 2);

        let msg = hex::decode(data[1]).expect(ERR_FAILED_PARSE);
        let mut fee: [u8; 32] = Default::default();
        fee.copy_from_slice(&msg[..31]);
        let mut recipient: EthAddress = Default::default();
        recipient.copy_from_slice(&msg[32..51]);

        OnTrasnferMessageData {
            relayer: data[0].into(),
            recipient,
            fee: U256::from(fee),
        }
    }

    /// Prepare message for `ft_transfer_call` -> `ft_on_transfer`
    fn set_message_for_on_transfer(&self, fee: U256, message: String) -> String {
        use byte_slice_cast::AsByteSlice;

        // Relayer == predecessor
        let relayer_account_id = String::from_utf8(sdk::predecessor_account_id()).unwrap();
        let mut data = fee.as_byte_slice().to_vec();
        let message = hex::decode(message).expect(ERR_FAILED_PARSE);
        data.append(&mut message.to_vec());
        [relayer_account_id, hex::encode(data)].join(":")
    }

    /// Deposit all types of tokens
    pub fn deposit(&self) {
        use crate::prover::Proof;
        #[cfg(feature = "log")]
        sdk::log("[Deposit tokens]".into());

        // Get incoming deposit arguments
        let raw_proof = &sdk::read_input()[..];
        let proof: Proof = Proof::try_from_slice(&raw_proof).expect("ERR_FAILED_PARSE");
        // Fetch event data from Proof
        let event = DepositedEvent::from_log_entry_data(&proof.log_entry_data);

        #[cfg(feature = "log")]
        sdk::log(format!(
            "Deposit started: from {} to recipient {:?} with amount: {:?} and fee {:?}",
            hex::encode(event.sender),
            event.recipient,
            event.amount.as_u128(),
            event.fee.as_u128()
        ));

        #[cfg(feature = "log")]
        sdk::log(format!(
            "Event's address {}, custodian address {}",
            hex::encode(&event.eth_custodian_address),
            hex::encode(&self.contract.eth_custodian_address),
        ));

        assert_eq!(
            event.eth_custodian_address, self.contract.eth_custodian_address,
            "ERR_WRONG_EVENT_ADDRESS",
        );
        assert!(event.amount < event.fee, "ERR_NOT_ENOUGH_BALANCE_FOR_FEE");

        // Verify proof data with cross-cotract call at prover account
        #[cfg(feature = "log")]
        sdk::log(format!(
            "Deposit verify_log_entry for prover: {}",
            self.contract.prover_account,
        ));
        let promise0 = sdk::promise_create(
            self.contract.prover_account.as_bytes(),
            b"verify_log_entry",
            &raw_proof,
            NO_DEPOSIT,
            GAS_FOR_VERIFY_LOG_ENTRY,
        );

        // Finilize deposit
        let promise1 = match self.parse_event_message(&event.recipient) {
            // Deposit to NEAR accounts
            TokenMessageData::Near(account_id) => {
                let data = FinishDepositCallArgs {
                    new_owner_id: account_id,
                    amount: event.amount.as_u128(),
                    fee: event.fee.as_u128(),
                    proof,
                }
                .try_to_vec()
                .unwrap();

                sdk::promise_then(
                    promise0,
                    &sdk::current_account_id(),
                    b"finish_deposit_near",
                    &data[..],
                    NO_DEPOSIT,
                    GAS_FOR_FINISH_DEPOSIT,
                )
            }
            // Deposit to Eth/ERC20 accounts
            TokenMessageData::Eth { address, message } => {
                let current_account_id = String::from_utf8(sdk::current_account_id()).unwrap();
                // Send to self - current account id
                let data = FinishDepositCallArgs {
                    new_owner_id: current_account_id,
                    amount: event.amount.as_u128(),
                    fee: event.fee.as_u128(),
                    proof,
                }
                .try_to_vec()
                .unwrap();

                let internal_promise = sdk::promise_then(
                    promise0,
                    &sdk::current_account_id(),
                    b"finish_deposit_near",
                    &data[..],
                    NO_DEPOSIT,
                    GAS_FOR_FINISH_DEPOSIT,
                );
                // Transfer to self and then transfer ETH in `ft_on_transfer`
                // address - is NEAR account
                let transfer_data = TransferCallCallArgs {
                    receiver_id: address,
                    amount: event.amount.as_u128(),
                    memo: None,
                    msg: self.set_message_for_on_transfer(event.fee, message),
                }
                .try_to_vec()
                .unwrap();
                sdk::promise_then(
                    internal_promise,
                    &sdk::current_account_id(),
                    b"ft_transfer_call",
                    &transfer_data[..],
                    NO_DEPOSIT,
                    GAS_FOR_FINISH_DEPOSIT,
                )
            }
        };

        sdk::promise_return(promise1);
    }

    /// Finish deposit for NEAR accounts
    pub fn finish_deposit_near(&mut self) {
        sdk::assert_private_call();
        let data = FinishDepositCallArgs::try_from_slice(&sdk::read_input()).unwrap();
        #[cfg(feature = "log")]
        sdk::log(format!("Finish deposit NEAR amount: {}", data.amount));
        assert_eq!(sdk::promise_results_count(), 1);

        // Check promise results
        let data0: Vec<u8> = match sdk::promise_result(0) {
            PromiseResult::Successful(x) => x,
            _ => sdk::panic_utf8(b"ERR_PROMISE_INDEX"),
        };
        #[cfg(feature = "log")]
        sdk::log("Check verification_success".into());
        let verification_success: bool = bool::try_from_slice(&data0).unwrap();
        assert!(verification_success, "ERR_VERIFY_PROOF");
        self.record_proof(data.proof.get_key());

        // Mint tokens to recipient minus fee
        self.mint_near(data.new_owner_id, data.amount - data.fee);
        // Mint fee for Predecessor
        let predecessor_account_id = String::from_utf8(sdk::predecessor_account_id()).unwrap();
        self.mint_near(predecessor_account_id, data.fee);
        // Save new contract data
        self.save_contract();
    }

    /// Finish deposit for ETH accounts
    /// TODO: remove, it's not used
    pub fn finish_deposit_eth(&mut self) {
        sdk::assert_private_call();
        let data = FinishDepositEthCallArgs::try_from_slice(&sdk::read_input()).unwrap();
        #[cfg(feature = "log")]
        sdk::log(format!("Finish deposit ETH amount: {}", data.amount));
        assert_eq!(sdk::promise_results_count(), 1);

        // Check promise results
        let data0: Vec<u8> = match sdk::promise_result(0) {
            PromiseResult::Successful(x) => x,
            _ => sdk::panic_utf8(b"ERR_PROMISE_INDEX"),
        };
        #[cfg(feature = "log")]
        sdk::log("Check verification_success".into());
        let verification_success: bool = bool::try_from_slice(&data0).unwrap();
        assert!(verification_success, "ERR_VERIFY_PROOF");
        self.record_proof(data.proof.get_key());

        // Mint tokens to recipient minus fee
        self.mint_eth(data.new_owner_id, data.amount);
        // Save new contract data
        self.save_contract();
    }

    /// Internal ETH deposit logic
    pub(crate) fn internal_deposit_eth(&mut self, address: &Address, amount: &U256) {
        self.ft.internal_deposit_eth(address.0, amount.as_u128());
        self.save_contract();
    }

    /// Internal ETH withdraw ETH logic
    pub(crate) fn internal_remove_eth(&mut self, address: &Address, amount: &U256) {
        self.ft.internal_withdraw_eth(address.0, amount.as_u128());
        self.save_contract();
    }

    /// Record used proof as hash key
    fn record_proof(&mut self, key: String) {
        #[cfg(feature = "log")]
        sdk::log("Record proof".into());
        let key = key.as_str();

        assert!(!self.check_used_event(key), "ERR_PROOF_EXIST");
        self.save_used_event(key);
    }

    ///  Mint NEAR tokens
    fn mint_near(&mut self, owner_id: AccountId, amount: Balance) {
        #[cfg(feature = "log")]
        sdk::log(format!("Mint NEAR {} tokens for: {}", amount, owner_id));

        if self.ft.accounts_get(&owner_id).is_none() {
            self.ft.accounts_insert(&owner_id, 0);
        }
        self.ft.internal_deposit(&owner_id, amount);
        #[cfg(feature = "log")]
        sdk::log("Mint NEAR success".into());
    }

    ///  Mint ETH tokens
    fn mint_eth(&mut self, owner_id: EthAddress, amount: Balance) {
        #[cfg(feature = "log")]
        sdk::log(format!(
            "Mint ETH {} tokens for: {}",
            amount,
            hex::encode(owner_id)
        ));
        self.ft.internal_deposit_eth(owner_id, amount);
        #[cfg(feature = "log")]
        sdk::log("Mint ETH success".into());
    }

    /// Burn NEAR tokens
    fn burn_near(&mut self, owner_id: AccountId, amount: Balance) {
        #[cfg(feature = "log")]
        sdk::log(format!("Burn NEAR {} tokens for: {}", amount, owner_id));
        self.ft.internal_withdraw(&owner_id, amount);
    }

    /// Burn ETH tokens
    #[allow(dead_code)]
    fn burn_eth(&mut self, address: EthAddress, amount: Balance) {
        #[cfg(feature = "log")]
        sdk::log(format!(
            "Burn ETH {} tokens for: {}",
            amount,
            hex::encode(address)
        ));
        self.ft.internal_withdraw_eth(address, amount);
    }

    pub fn withdraw_near(&mut self) {
        #[cfg(feature = "log")]
        sdk::log("Start withdraw NEAR".into());
        let args =
            WithdrawCallArgs::try_from_slice(&sdk::read_input()[..]).expect(ERR_FAILED_PARSE);
        let recipient_address = validate_eth_address(args.recipient_id);
        let res = WithdrawResult {
            recipient_id: recipient_address,
            amount: args.amount,
            eth_custodian_address: self.contract.eth_custodian_address,
        }
        .try_to_vec()
        .unwrap();
        // Burn tokens to recipient
        let predecessor_account_id = String::from_utf8(sdk::predecessor_account_id()).unwrap();
        self.burn_near(predecessor_account_id, args.amount);
        // Save new contract data
        self.save_contract();
        sdk::return_output(&res[..]);
    }

    /// Return total supply of NEAR + ETH
    pub fn ft_total_supply(&self) {
        let total_supply = self.ft.ft_total_supply();
        sdk::return_output(&total_supply.to_string().as_bytes());
        #[cfg(feature = "log")]
        sdk::log(format!("Total supply: {}", total_supply));
    }

    /// Return total supply of NEAR
    pub fn ft_total_supply_near(&self) {
        let total_supply = self.ft.ft_total_supply_near();
        sdk::return_output(&total_supply.to_string().as_bytes());
        #[cfg(feature = "log")]
        sdk::log(format!("Total supply NEAR: {}", total_supply));
    }

    /// Return total supply of ETH
    pub fn ft_total_supply_eth(&self) {
        let total_supply = self.ft.ft_total_supply_eth();
        sdk::return_output(&total_supply.to_string().as_bytes());
        #[cfg(feature = "log")]
        sdk::log(format!("Total supply ETH: {}", total_supply));
    }

    /// Return balance of NEAR
    pub fn ft_balance_of(&self) {
        let args =
            BalanceOfCallArgs::try_from_slice(&sdk::read_input()[..]).expect(ERR_FAILED_PARSE);
        let balance = self.ft.ft_balance_of(&args.account_id);
        sdk::return_output(&balance.to_string().as_bytes());
        #[cfg(feature = "log")]
        sdk::log(format!(
            "Balance of NEAR [{}]: {}",
            args.account_id, balance
        ));
    }

    /// Return balance of ETH
    pub fn ft_balance_of_eth(&self) {
        let args =
            BalanceOfEthCallArgs::try_from_slice(&sdk::read_input()[..]).expect(ERR_FAILED_PARSE);
        let balance = self.ft.internal_unwrap_balance_of_eth(args.address);
        #[cfg(feature = "log")]
        sdk::log(format!(
            "Balance of ETH [{}]: {}",
            hex::encode(args.address),
            balance
        ));
        sdk::return_output(&balance.to_string().as_bytes());
    }

    /// Transfer between NEAR accounts
    pub fn ft_transfer(&mut self) {
        let args =
            TransferCallArgs::try_from_slice(&sdk::read_input()[..]).expect(ERR_FAILED_PARSE);
        self.ft
            .ft_transfer(&args.receiver_id, args.amount, &args.memo);
        self.save_contract();
        #[cfg(feature = "log")]
        sdk::log(format!(
            "Transfer amount {} to {} success with memo: {:?}",
            args.amount, args.receiver_id, args.memo
        ));
    }

    /// FT resolve transfer logic
    pub fn ft_resolve_transfer(&mut self) {
        sdk::assert_private_call();
        let args = ResolveTransferCallArgs::try_from_slice(&sdk::read_input()).unwrap();
        let amount = self
            .ft
            .ft_resolve_transfer(&args.sender_id, &args.receiver_id, args.amount);
        // `ft_resolve_transfer` can changed `total_supply` so we should save contract
        self.save_contract();
        sdk::return_output(&amount.to_string().as_bytes());
        #[cfg(feature = "log")]
        sdk::log(format!(
            "Resolve transfer of {} from {} to {} success",
            args.amount, args.sender_id, args.receiver_id
        ));
    }

    /// FT transfer call from sender account (invoker account) to receiver
    pub fn ft_transfer_call(&mut self) {
        let args =
            TransferCallCallArgs::try_from_slice(&sdk::read_input()).expect(ERR_FAILED_PARSE);
        #[cfg(feature = "log")]
        sdk::log(format!(
            "Transfer call to {} amount {}",
            args.receiver_id, args.amount,
        ));

        self.ft
            .ft_transfer_call(&args.receiver_id, args.amount, &args.memo, args.msg);
    }

    /// FT storage deposit logic
    pub fn storage_deposit(&mut self) {
        let args =
            StorageDepositCallArgs::try_from_slice(&sdk::read_input()[..]).expect(ERR_FAILED_PARSE);
        let res = self
            .ft
            .storage_deposit(args.account_id.as_ref(), args.registration_only)
            .try_to_vec()
            .unwrap();
        self.save_contract();
        sdk::return_output(&res[..]);
    }

    /// FT storage withdraw
    pub fn storage_withdraw(&mut self) {
        let args = StorageWithdrawCallArgs::try_from_slice(&sdk::read_input()[..])
            .expect(ERR_FAILED_PARSE);
        let res = self.ft.storage_withdraw(args.amount).try_to_vec().unwrap();
        self.save_contract();
        sdk::return_output(&res[..]);
    }

    /// Get balance of storage
    pub fn storage_balance_of(&self) {
        let args = StorageBalanceOfCallArgs::try_from_slice(&sdk::read_input()[..])
            .expect(ERR_FAILED_PARSE);
        let res = self
            .ft
            .storage_balance_of(&args.account_id)
            .try_to_vec()
            .unwrap();
        sdk::return_output(&res[..]);
    }

    /// Save to storage Relayed address as NEAR account alias
    pub fn register_relayer(&self) {
        let args: RegisterRelayerCallArgs =
            RegisterRelayerCallArgs::try_from_slice(&sdk::read_input()[..])
                .expect(ERR_FAILED_PARSE);
        let account_id = String::from_utf8(sdk::predecessor_account_id()).unwrap();
        sdk::write_storage(self.evm_relayer_key(&account_id).as_bytes(), &args.address)
    }

    /// Save to storage erc20 address as NEAR account alias
    pub fn save_evm_token_address(&self, account_id: &str, address: EthAddress) {
        sdk::write_storage(self.evm_token_key(account_id).as_bytes(), &address)
    }

    /// Get EVM ERC20 token address
    pub fn get_evm_token_address(&self, account_id: &str) -> EthAddress {
        let acc = sdk::read_storage(self.evm_token_key(account_id).as_bytes())
            .expect("ERR_WRONG_EVM_TOKEN_KEY");
        let mut addr: EthAddress = Default::default();
        addr.copy_from_slice(&acc);
        addr
    }

    /// Get EVM Relayer address
    pub fn get_evm_relayer_address(&self, account_id: &str) -> EthAddress {
        let acc = sdk::read_storage(self.evm_relayer_key(account_id).as_bytes())
            .expect("ERR_WRONG_EVM_TOKEN_KEY");
        let mut addr: EthAddress = Default::default();
        addr.copy_from_slice(&acc);
        addr
    }

    /// ft_on_transfer call back function
    pub fn ft_on_transfer(&mut self) {
        #[cfg(feature = "log")]
        sdk::log("Call ft_on_trasfer".into());
        let args = FtOnTransfer::try_from_slice(&sdk::read_input()[..]).expect(ERR_FAILED_PARSE);
        let predecessor_account_id = String::from_utf8(sdk::predecessor_account_id()).unwrap();
        let current_account_id = String::from_utf8(sdk::current_account_id()).unwrap();
        let message_data = self.parse_on_transfer_message(&args.msg);

        // Special case when current_account_id is predecessor
        if current_account_id == predecessor_account_id {
            self.ft.internal_withdraw(&current_account_id, args.amount);
            self.ft
                .internal_deposit_eth(message_data.recipient, args.amount);
            self.save_contract();

            #[cfg(feature = "log")]
            sdk::log(format!(
                "Transfer NEAR tokens {} amount to {} ETH success",
                args.amount,
                hex::encode(message_data.recipient),
            ));
        } else {
            // ERC20 address
            let _evm_token_addres = self.get_evm_token_address(&predecessor_account_id);
            let evm_relayer_addres = self.get_evm_relayer_address(&message_data.relayer);
            let recipient_address = message_data.recipient;

            // Transfer fee to Relayer
            let fee = message_data.fee.as_u128();
            if fee > 0 {
                self.ft.internal_withdraw_eth(recipient_address, fee);
                self.ft.internal_deposit_eth(evm_relayer_addres, fee);
            }
            self.save_contract();
        }

        // Return unused tokens
        let data = 0u128.try_to_vec().unwrap();
        sdk::return_output(&data[..]);
    }

    /// EVM ERC20 token key
    fn evm_token_key(&self, account_id: &str) -> String {
        [EVM_TOKEN_NAME_KEY, account_id].join(":")
    }

    /// EVM relayer address key
    fn evm_relayer_key(&self, account_id: &str) -> String {
        [EVM_RELAYER_NAME_KEY, account_id].join(":")
    }

    /// Save eth-connecor contract data
    fn save_contract(&mut self) {
        sdk::save_contract(CONTRACT_NAME_KEY.as_bytes(), &self.contract);
        sdk::save_contract(CONTRACT_FT_KEY.as_bytes(), &self.ft);
    }

    /// Generate key for used events from Prood
    fn used_event_key(&self, key: &str) -> String {
        [CONTRACT_NAME_KEY, "used-event", key].join(".")
    }

    /// Save already used event proof as hash key
    fn save_used_event(&self, key: &str) {
        sdk::save_contract(&self.used_event_key(key).as_bytes(), &0u8);
    }

    /// Check is event of proof already used
    fn check_used_event(&self, key: &str) -> bool {
        sdk::storage_has_key(&self.used_event_key(key).as_bytes())
    }
}
