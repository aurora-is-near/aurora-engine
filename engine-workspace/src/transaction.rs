use aurora_engine_types::borsh::BorshSerialize;
use near_workspaces::result::ExecutionFinalResult;
use near_workspaces::rpc::BoxFuture;
use near_workspaces::rpc::query::{Query, ViewFunction};
use near_workspaces::types::NearToken;
use std::future::IntoFuture;

pub struct ViewTransaction<'a> {
    pub(crate) inner: Query<'a, ViewFunction>,
}

impl<'a> ViewTransaction<'a> {
    pub(crate) const fn new(view_tx: Query<'a, ViewFunction>) -> Self {
        Self { inner: view_tx }
    }

    #[must_use]
    pub fn args(mut self, args: Vec<u8>) -> Self {
        self.inner = self.inner.args(args);
        self
    }

    #[must_use]
    pub fn args_json<U: serde::Serialize>(mut self, args: U) -> Self {
        self.inner = self.inner.args_json(args);
        self
    }

    #[must_use]
    pub fn args_borsh<U: BorshSerialize>(mut self, args: U) -> Self {
        self.inner = self.inner.args_borsh(args);
        self
    }
}

impl<'a> IntoFuture for ViewTransaction<'a> {
    type Output = anyhow::Result<near_workspaces::result::ViewResultDetails>;
    type IntoFuture = BoxFuture<'a, Self::Output>;

    fn into_future(self) -> Self::IntoFuture {
        Box::pin(async { Ok(self.inner.await?) }.into_future())
    }
}

pub struct CallTransaction {
    inner: near_workspaces::operations::CallTransaction,
}

impl CallTransaction {
    pub(crate) const fn new(call_tx: near_workspaces::operations::CallTransaction) -> Self {
        Self { inner: call_tx }
    }

    #[must_use]
    pub fn args(mut self, args: Vec<u8>) -> Self {
        self.inner = self.inner.args(args);
        self
    }

    #[must_use]
    pub fn args_json<S: serde::Serialize>(mut self, args: S) -> Self {
        self.inner = self.inner.args_json(args);
        self
    }

    #[must_use]
    pub fn args_borsh<B: BorshSerialize>(mut self, args: B) -> Self {
        self.inner = self.inner.args_borsh(args);
        self
    }

    #[must_use]
    pub fn gas(mut self, gas: u64) -> Self {
        self.inner = self.inner.gas(near_gas::NearGas::from_gas(gas));
        self
    }

    #[must_use]
    pub fn max_gas(mut self) -> Self {
        self.inner = self.inner.max_gas();
        self
    }

    #[must_use]
    pub fn deposit(mut self, deposit: NearToken) -> Self {
        self.inner = self.inner.deposit(deposit);
        self
    }

    pub async fn transact(self) -> anyhow::Result<ExecutionFinalResult> {
        Ok(self.inner.transact().await?)
    }
}
