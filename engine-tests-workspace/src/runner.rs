use crate::prelude::U256;
use crate::prelude::{AccountId, Wei, H256};
use aurora_engine::parameters::{
    DeployErc20TokenArgs, FunctionCallArgsV2, GetStorageAtArgs, IsUsedProofCallArgs,
    NEP141FtOnTransferArgs, NewCallArgs, SubmitResult, TransactionStatus, ViewCallArgs,
    WithdrawResult,
};
use aurora_engine::xcc::AddressVersionUpdateArgs;
use aurora_engine_sdk::promise::PromiseId;
use aurora_engine_types::parameters::WithdrawCallArgs;
use aurora_engine_types::types::Address;
use borsh::BorshSerialize;
use workspaces::result::{ExecutionFinalResult, ViewResultDetails};
use workspaces::Contract;

#[cfg(feature = "mainnet-test")]
const ENGINE_WASM_FILEPATH: &str = "../bin/aurora-mainnet-test.wasm";
#[cfg(feature = "testnet-test")]
const ENGINE_WASM_FILEPATH: &str = "../bin/aurora-testnet-test.wasm";
pub const AURORA_LOCAL_CHAIN_ID: u64 = 1313161556;
const AURORA_ACCOUNT_ID: &str = "aurora";
const AURORA_BRIDGE_PROVER: &str = "bridge_prover.near";

trait AuroraReturns {
    fn try_to_account_id(self) -> anyhow::Result<AccountId>;

    fn try_to_address(self) -> anyhow::Result<Address>;

    fn try_to_bool(self) -> anyhow::Result<bool>;

    fn try_to_evm_result(self) -> anyhow::Result<SubmitResult>;

    fn try_to_promise_id(self) -> anyhow::Result<PromiseId>;

    fn try_to_string(self) -> anyhow::Result<String>;

    fn try_to_transaction_status(self) -> anyhow::Result<TransactionStatus>;

    fn try_to_hash(self) -> anyhow::Result<H256>;

    fn try_to_u64(self) -> anyhow::Result<u64>;

    fn try_to_u256(self) -> anyhow::Result<U256>;

    fn try_to_withdraw_result(self) -> anyhow::Result<WithdrawResult>;

    fn try_to_vec(self) -> anyhow::Result<Vec<u8>>;
}

impl AuroraReturns for ExecutionFinalResult {
    fn try_to_account_id(self) -> anyhow::Result<AccountId> {
        // TODO fix unwrap here
        Ok(AccountId::new(&self.try_to_string()?)?)
    }

    fn try_to_address(self) -> anyhow::Result<Address> {
        Ok(Address::try_from_slice(&self.raw_bytes()?)?)
    }

    fn try_to_bool(self) -> anyhow::Result<bool> {
        Ok(self.borsh()?)
    }

    fn try_to_evm_result(self) -> anyhow::Result<SubmitResult> {
        match self.into_result() {
            Ok(success) => Ok(success.borsh()?),
            Err(err) => {
                // TODO get actual error. See: near/workspaces-rs/issues/191
                Err(err.into())
            }
        }
    }

    fn try_to_promise_id(self) -> anyhow::Result<PromiseId> {
        Ok(PromiseId::new(self.try_to_u64()?))
    }

    fn try_to_string(self) -> anyhow::Result<String> {
        Ok(String::from_utf8(self.raw_bytes()?)?)
    }

    fn try_to_transaction_status(self) -> anyhow::Result<TransactionStatus> {
        Ok(self.borsh()?)
    }

    fn try_to_hash(self) -> anyhow::Result<H256> {
        Ok(H256::from_slice(&self.raw_bytes()?))
    }

    fn try_to_u64(self) -> anyhow::Result<u64> {
        let bytes = self.raw_bytes()?;
        let mut buf = [0u8; 8];
        buf.copy_from_slice(&bytes[0..8]);
        Ok(u64::from_le_bytes(buf))
    }

    fn try_to_u256(self) -> anyhow::Result<U256> {
        Ok(U256::from_big_endian(&self.raw_bytes()?))
    }

    fn try_to_withdraw_result(self) -> anyhow::Result<WithdrawResult> {
        Ok(self.borsh()?)
    }

    fn try_to_vec(self) -> anyhow::Result<Vec<u8>> {
        Ok(self.raw_bytes()?)
    }
}

// TODO: use me
pub struct CallResult<T> {
    result: T,
    logs: Vec<String>,
}

// TODO: implement a signer when a method is called, return a signer with
// information required about the transaction to be made. Then give the option
// to sign with another key, or with some default. Preferably, run `transact`.
pub(crate) struct EvmContract {
    // The Aurora account ID.
    // pub aurora_account_id: String,
    /// The Ethereum chain ID.
    pub chain_id: u64,
    pub contract: Contract,
    // pub cache: MockCompiledContractCache,
    // pub ext: mocked_external::MockedExternalWithTrie,
    // pub context: VMContext,
    // pub wasm_config: VMConfig,
    // pub fees_config: RuntimeFeesConfig,
    // pub current_protocol_version: u32,
    // pub previous_logs: Vec<String>,
    // Use the standalone in parallel if set. This allows checking both
    // implementations give the same results.
    // pub standalone_runner: Option<standalone::StandaloneRunner>,
    // Empty by default. Can be set in tests if the transaction should be
    // executed as if it was a callback.
    // pub promise_results: Vec<PromiseResult>,
}

impl EvmContract {
    pub async fn new() -> anyhow::Result<Self> {
        let worker = workspaces::sandbox().await?;
        let wasm = std::fs::read(ENGINE_WASM_FILEPATH)?;
        let contract = worker.dev_deploy(&wasm).await?;

        let args = NewCallArgs {
            chain_id: crate::prelude::u256_to_arr(&U256::from(AURORA_LOCAL_CHAIN_ID)),
            // TODO: implement std::error::Error for ParseAccountError.
            // TODO: use near-account-id library across whole repository
            owner_id: AccountId::new(contract.id().as_str()).unwrap(),
            bridge_prover_id: AccountId::new(AURORA_BRIDGE_PROVER).unwrap(),
            upgrade_delay_blocks: 1,
        };

        let outcome = contract.call("new").args_borsh(args).transact().await?;

        assert!(outcome.is_success());

        Ok(Self {
            chain_id: AURORA_LOCAL_CHAIN_ID,
            contract,
        })
    }

    pub async fn call_near(
        &self,
        function: &str,
        args: Vec<u8>,
    ) -> anyhow::Result<ExecutionFinalResult> {
        let execution_result = self
            .contract
            .call(function)
            .args(args)
            .max_gas()
            .transact()
            .await?;

        // println!("{:#?}", execution_result);

        // assert!(execution_result.is_success());

        Ok(execution_result)
    }

    // TODO: improve view to be like call.
    pub async fn view_near(&self, function: &str, args: Vec<u8>) -> ViewResultDetails {
        self.contract.view(function, args).await.unwrap() // TODO: fix error handling here
    }

    // TODO: improve with making the args vec on method above
    pub async fn call_near_borsh<U: borsh::BorshSerialize>(
        &self,
        function: &str,
        args: U,
    ) -> anyhow::Result<ExecutionFinalResult> {
        let execution_result = self
            .contract
            .call(function)
            .max_gas()
            .args_borsh(args)
            .transact()
            .await?;

        // assert!(execution_result.is_success());

        Ok(execution_result)
    }

    // TODO: Test which ensures that the version was properly bumped.
    // TODO: replace with view
    // pub async fn get_version(&self) -> String {
    //     self.call_near("get_version", vec![])
    //         .await
    //         .unwrap()
    //         .to_string()
    // }

    // TODO: replace with view
    pub async fn get_owner(&self) -> anyhow::Result<AccountId> {
        self.call_near("get_owner", vec![])
            .await?
            .try_to_account_id()
    }

    // TODO: replace with view
    pub async fn get_bridge_prover(&self) -> anyhow::Result<AccountId> {
        self.call_near("get_bridge_prover", vec![])
            .await?
            .try_to_account_id()
    }

    // TODO: replace with view
    pub async fn get_chain_id(&self) -> anyhow::Result<U256> {
        self.call_near("get_chain_id", vec![]).await?.try_to_u256()
    }

    // TODO: replace with view
    pub async fn get_upgrade_index(&self) -> anyhow::Result<u64> {
        self.call_near("get_upgrade_index", vec![])
            .await?
            .try_to_u64()
    }

    pub async fn stage_upgrade(&self) {
        todo!()
    }

    pub async fn deploy_upgrade(&self) {
        todo!()
    }

    pub async fn stage_migration(&self) {
        todo!()
    }

    pub async fn deploy_code(&self, code: Vec<u8>) -> anyhow::Result<SubmitResult> {
        self.call_near("deploy_code", code)
            .await?
            .try_to_evm_result()
    }

    pub async fn call(&self, args: FunctionCallArgsV2) -> anyhow::Result<SubmitResult> {
        self.call_near_borsh("call", args)
            .await?
            .try_to_evm_result()
    }

    pub async fn submit(&self, args: Vec<u8>) -> anyhow::Result<SubmitResult> {
        self.call_near("submit", args).await?.try_to_evm_result()
    }

    pub async fn register_relayer(&self, relayer_id: AccountId) -> anyhow::Result<()> {
        // We don't need the result here as there is none.
        let _ = self
            .call_near("register_relayer", relayer_id.to_vec())
            .await?;
        Ok(())
    }

    pub fn factory_update(&self, _router_bytes: Vec<u8>) -> anyhow::Result<()> {
        todo!()
    }

    pub async fn factory_update_address_version(
        &self,
        _update_args: AddressVersionUpdateArgs,
    ) -> anyhow::Result<()> {
        todo!()
    }

    pub async fn factory_set_wnear_address(&self, wnear_address: Address) -> anyhow::Result<()> {
        let _ = self
            .call_near("set_wnear_address", wnear_address.to_vec())
            .await?;
        Ok(())
    }

    pub async fn ft_on_transfer(
        &self,
        ft_on_transfer_args: NEP141FtOnTransferArgs,
    ) -> anyhow::Result<()> {
        let _ = self
            .call_near_borsh("ft_on_transfer", ft_on_transfer_args)
            .await?;
        Ok(())
    }

    pub async fn deploy_erc20_token(
        &self,
        deploy_args: DeployErc20TokenArgs,
    ) -> anyhow::Result<Address> {
        Ok(self
            .call_near_borsh("deploy_erc20_token", deploy_args)
            .await?
            .try_to_address()?)
    }

    // TODO: check if this is necessary, considering that it is a private
    // function. Else, could improve it with adding a removal of
    // io.assert_private_call in refund_on_error engine function for tests only.
    // pub async fn refund_on_error(&self)

    // TODO: all view returns should be the correct types.
    pub async fn view(&self, view_args: ViewCallArgs) -> anyhow::Result<TransactionStatus> {
        self.call_near_borsh("view", view_args)
            .await?
            .try_to_transaction_status()
    }

    pub async fn get_block_hash(&self, block_height: u64) -> ViewResultDetails {
        self.view_near("get_block_hash", block_height.try_to_vec().unwrap()) // TODO: fix unwrap
            .await
    }

    pub async fn get_code(&self, address: Address) -> Vec<u8> {
        self.view_near("get_code", address.to_vec()).await.result
    }

    pub async fn get_balance(&self, address: Address) -> Wei {
        Wei::new(U256::from_big_endian(
            &self.view_near("get_balance", address.to_vec()).await.result,
        ))
    }

    pub async fn get_nonce(&self, address: Address) -> U256 {
        U256::from_big_endian(&self.view_near("get_nonce", address.to_vec()).await.result)
    }

    // TODO return
    pub async fn get_storage_at(&self, args: GetStorageAtArgs) -> ViewResultDetails {
        self.view_near("get_storage_at", args.try_to_vec().unwrap())
            .await // TODO: fix unwrap
    }

    // pub async fn new_eth_connector

    // pub async fn set_eth_connector_contract_data()

    pub async fn withdraw(&self, args: WithdrawCallArgs) -> anyhow::Result<WithdrawResult> {
        self.call_near_borsh("withdraw", args)
            .await?
            .try_to_withdraw_result()
    }

    pub async fn deposit(&self, raw_proof: Vec<u8>) -> anyhow::Result<PromiseId> {
        self.call_near("deposit", raw_proof)
            .await?
            .try_to_promise_id()
    }

    // pub async fn finish_deposit

    pub async fn is_used_proof(&self, args: IsUsedProofCallArgs) -> anyhow::Result<bool> {
        self.call_near_borsh("is_used_proof", args)
            .await?
            .try_to_bool()
    }

    // TODO ... rest

    pub async fn mint_account(
        &mut self,
        address: Address,
        nonce: u64,
        balance: u64,
    ) -> anyhow::Result<()> {
        let args = (address, nonce, balance);
        let _ = self.call_near_borsh("mint_account", args).await;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use aurora_engine_types::types::Address;

    #[tokio::test]
    async fn new() {
        let outcome = super::EvmContract::new().await.unwrap();
        let res = outcome.get_nonce(Address::zero()).await;
    }
}
