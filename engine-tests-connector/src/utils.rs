use aurora_engine_types::parameters::connector::{
    FungibleTokenMetadata, SetEthConnectorContractAccountArgs, WithdrawSerializeType,
};
use aurora_engine_types::types::{Address, Wei};
use near_sdk::json_types::U128;
use near_sdk::serde_json::json;
use near_workspaces::network::NetworkClient;
use near_workspaces::types::NearToken;
use near_workspaces::{result::ExecutionFinalResult, Account, AccountId, Contract, Worker};
use std::path::Path;
use std::sync::LazyLock;

pub const DEPOSITED_RECIPIENT: &str = "eth_recipient.root";
pub const DEPOSITED_RECIPIENT_NAME: &str = "eth_recipient";
pub const DEPOSITED_AMOUNT: u128 = 800400;
pub const DEPOSITED_EVM_AMOUNT: u128 = 10200;
pub static RECIPIENT_ADDRESS: LazyLock<Address> =
    LazyLock::new(|| Address::decode("891b2749238b27ff58e951088e55b04de71dc374").unwrap());

pub type PausedMask = u8;

/// Admin control flow flag indicates that all control flow unpause (unblocked).
pub const UNPAUSE_ALL: PausedMask = 0;
/// Admin control flow flag indicates that the deposit is paused.
pub const PAUSE_DEPOSIT: PausedMask = 1 << 0;
/// Admin control flow flag indicates that withdrawal is paused.
pub const PAUSE_WITHDRAW: PausedMask = 1 << 1;
/// Admin control flow flag indicates that ft transfers are paused.
pub const PAUSE_FT: PausedMask = 1 << 2;

static CONTRACT_WASM: LazyLock<Vec<u8>> = LazyLock::new(|| {
    let manifest_path = std::env::current_dir()
        .unwrap()
        .join("etc")
        .join("aurora-eth-connector")
        .join("eth-connector")
        .join("Cargo.toml");
    let artifact = cargo_near_build::build(cargo_near_build::BuildOpts {
        manifest_path: Some(manifest_path.try_into().unwrap()),
        no_abi: true,
        no_locked: true,
        features: Some("integration-test,migration".to_owned()),
        ..Default::default()
    })
    .unwrap();

    std::fs::read(artifact.path.into_std_path_buf())
        .map_err(|e| anyhow::anyhow!("failed to read the wasm file: {e}"))
        .unwrap()
});

static MOCK_CONTROLLER_WASM: LazyLock<Vec<u8>> = LazyLock::new(|| {
    let base_path = Path::new("../etc").join("tests").join("mock-controller");
    let artifact_path = crate::rust::compile(base_path);
    std::fs::read(artifact_path).unwrap()
});

pub struct TestContract {
    pub engine_contract: Contract,
    pub eth_connector_contract: Contract,
    pub controller_account: Account,
    pub owner: Option<Account>,
}

impl TestContract {
    async fn deploy_contracts() -> anyhow::Result<(Contract, Contract, Account)> {
        use near_workspaces::{
            types::{KeyType, SecretKey},
            AccessKey,
        };
        let worker = near_workspaces::sandbox()
            .await
            .map_err(|err| anyhow::anyhow!("Failed init sandbox: {err:?}"))?;
        let registrar: AccountId = "registrar".parse()?;
        let sk = SecretKey::from_seed(KeyType::ED25519, registrar.as_str());
        let registrar = worker
            .import_contract(&registrar, &worker)
            .transact()
            .await?;
        Self::waiting_account_creation(&worker, registrar.id()).await?;

        let controller: AccountId = "controller".parse()?;
        registrar
            .as_account()
            .batch(&controller)
            .create_account()
            .deploy(&MOCK_CONTROLLER_WASM)
            .add_key(sk.public_key(), AccessKey::full_access())
            .transfer(NearToken::from_near(100))
            .transact()
            .await?
            .into_result()?;

        let controller_account = Account::from_secret_key(controller, sk, &worker);
        let eth_connector = controller_account
            .create_subaccount("eth_connector")
            .initial_balance(NearToken::from_near(15))
            .transact()
            .await?
            .into_result()?;
        let engine = controller_account
            .create_subaccount("engine")
            .initial_balance(NearToken::from_near(15))
            .transact()
            .await?
            .into_result()?;
        let engine_contract_bytes = get_engine_contract();
        let engine_contract = engine.deploy(&engine_contract_bytes).await?.into_result()?;
        let eth_connector_contract = eth_connector.deploy(&CONTRACT_WASM).await?.into_result()?;

        Ok((engine_contract, eth_connector_contract, controller_account))
    }

    pub async fn new() -> anyhow::Result<Self> {
        Self::new_contract(None).await
    }

    pub async fn new_with_owner(owner: &str) -> anyhow::Result<Self> {
        Self::new_contract(Some(owner)).await
    }

    async fn new_contract(owner: Option<&str>) -> anyhow::Result<Self> {
        let (engine_contract, eth_connector_contract, controller_account) =
            Self::deploy_contracts().await?;

        let owner = if let Some(owner) = owner {
            Some(
                controller_account
                    .create_subaccount(owner)
                    .initial_balance(NearToken::from_near(15))
                    .transact()
                    .await?
                    .into_result()?,
            )
        } else {
            None
        };

        let metadata = FungibleTokenMetadata::default();
        // Init eth-connector
        let metadata = json!({
            "spec": metadata.spec,
            "name": metadata.name,
            "symbol": metadata.symbol,
            "icon": metadata.icon,
            "reference": metadata.reference,
            "decimals": metadata.decimals,
        });
        let res = eth_connector_contract
            .call("new")
            .args_json(json!({
                "metadata": metadata,
                "aurora_engine_account_id": engine_contract.id(),
                "owner_id": owner.as_ref().map_or_else(|| engine_contract.id(), |owner| owner.id()),
                "controller": controller_account.id(),
            }))
            .max_gas()
            .transact()
            .await?;
        assert!(res.is_success());

        let result = eth_connector_contract
            .call("pa_unpause_feature")
            .args_json(json!({ "key": "ALL" }))
            .max_gas()
            .transact()
            .await?;
        assert!(result.is_success(), "{result:#?}");

        let chain_id = [0u8; 32];
        let res = engine_contract
            .call("new")
            .args_borsh((chain_id, engine_contract.id(), engine_contract.id(), 1_u64))
            .max_gas()
            .transact()
            .await?;
        assert!(res.is_success());

        let acc = SetEthConnectorContractAccountArgs {
            account: eth_connector_contract.id().as_str().parse().unwrap(),
            withdraw_serialize_type: WithdrawSerializeType::Borsh,
        };
        let res = engine_contract
            .call("set_eth_connector_contract_account")
            .args_borsh(acc)
            .max_gas()
            .transact()
            .await?;
        assert!(res.is_success());

        Ok(Self {
            engine_contract,
            eth_connector_contract,
            controller_account,
            owner,
        })
    }

    /// Waiting for the account creation
    async fn waiting_account_creation<T: NetworkClient + ?Sized + Send + Sync>(
        worker: &Worker<T>,
        account_id: &AccountId,
    ) -> anyhow::Result<()> {
        let timer = std::time::Instant::now();
        // Try to get account within 30 secs
        for _ in 0..60 {
            if worker.view_account(account_id).await.is_err() {
                tokio::time::sleep(std::time::Duration::from_millis(500)).await;
            } else {
                return Ok(());
            }
        }

        anyhow::bail!(
            "Account `{account_id}` was not created in {:?} sec",
            timer.elapsed()
        )
    }

    pub async fn create_sub_account(&self, name: &str) -> anyhow::Result<Account> {
        Ok(self
            .controller_account
            .create_subaccount(name)
            .initial_balance(NearToken::from_near(15))
            .transact()
            .await?
            .into_result()?)
    }

    pub async fn deposit_eth_to_near(
        &self,
        account_id: &AccountId,
        amount: U128,
    ) -> anyhow::Result<ExecutionFinalResult> {
        Ok(self
            .controller_account // Only controller can deposit/mint tokens on eth-connector
            .call(self.eth_connector_contract.id(), "mint")
            .args_json(json!({
                "account_id": account_id,
                "amount": amount,
                "msg": None::<String>,
            }))
            .deposit(NearToken::from_yoctonear(1))
            .max_gas()
            .transact()
            .await?)
    }

    pub async fn deposit_eth_to_aurora(
        &self,
        amount: U128,
        recepient: &Address,
    ) -> anyhow::Result<ExecutionFinalResult> {
        Ok(self
            .controller_account
            .call(self.eth_connector_contract.id(), "mint")
            .args_json(json!({
                "account_id": self.engine_contract.id(),
                "amount": amount,
                "msg": recepient.encode(),
            }))
            .deposit(NearToken::from_yoctonear(1))
            .max_gas()
            .transact()
            .await?)
    }

    pub fn check_error_message(
        &self,
        res: &ExecutionFinalResult,
        error_msg: &str,
    ) -> anyhow::Result<bool> {
        if let Some(out) = res.receipt_failures().into_iter().next() {
            let actual_err_msg = format!("{out:?}");
            return if actual_err_msg.contains(error_msg) {
                Ok(true)
            } else {
                anyhow::bail!("Error message is different: {actual_err_msg:#?}");
            };
        }

        anyhow::bail!("There are no errors in the result");
    }

    pub async fn get_eth_on_near_balance(&self, account: &AccountId) -> anyhow::Result<U128> {
        let res = self
            .engine_contract
            .call("ft_balance_of")
            .args_json((account,))
            .max_gas()
            .transact()
            .await?
            .into_result()?
            .json::<U128>()?;
        Ok(res)
    }

    pub async fn get_eth_balance(&self, address: &Address) -> anyhow::Result<u128> {
        let res = self
            .engine_contract
            .call("ft_balance_of_eth") // `get_balance` returns tha same value but in borsh
            .args_borsh((address,))
            .view()
            .await?;

        res.json::<Wei>().map_err(Into::into).and_then(|res| {
            res.try_into_u128()
                .map_err(|e| anyhow::anyhow!(e.to_string()))
        })
    }

    pub async fn total_supply(&self) -> anyhow::Result<u128> {
        let res = self
            .engine_contract
            .call("ft_total_supply")
            .max_gas()
            .transact()
            .await?
            .into_result()?
            .json::<U128>()?;
        Ok(res.0)
    }
}

fn get_engine_contract() -> Vec<u8> {
    std::fs::read("../bin/aurora-engine-test.wasm").expect("Failed to read the wasm file")
}

/// Bytes for a NEAR smart contract implementing `ft_on_transfer`
#[must_use]
pub fn dummy_ft_receiver_bytes() -> Vec<u8> {
    let base_path = Path::new("../etc").join("tests").join("ft-receiver");
    let artifact_path = crate::rust::compile(base_path);
    std::fs::read(artifact_path).unwrap()
}
