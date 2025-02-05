#[macro_export]
macro_rules! impl_view_return  {
    ($(($name:ident => $return:ty, $fn_name:expr, $deserialize_fn:ident)),* $(,)?) => {
        use aurora_engine_types::borsh;
        $(pub struct $name<'a>(ViewTransaction<'a>);
        impl<'a> $name<'a> {
            pub(crate) fn view(contract: &'a RawContract) -> Self {
                Self(contract.view(&$fn_name))
            }

            #[must_use]
            pub fn args(mut self, args: Vec<u8>) -> Self {
                self.0 = self.0.args(args);
                self
            }

            #[must_use]
            pub fn args_json<S: serde::Serialize>(mut self, args: S) -> Self {
                self.0 = self.0.args_json(args);
                self
            }

            #[must_use]
            pub fn args_borsh<B: borsh::BorshSerialize>(mut self, args: B) -> Self {
                self.0 = self.0.args_borsh(args);
                self
            }
        }

        impl<'a> std::future::IntoFuture for $name<'a> {
            type Output = anyhow::Result<ViewResult<$return>>;
            type IntoFuture = near_workspaces::rpc::BoxFuture<'a, Self::Output>;

            fn into_future(self) -> Self::IntoFuture {
                Box::pin(async { ViewResult::$deserialize_fn(self.0.await?) }.into_future())
            }
        })*
    };
}

#[macro_export]
macro_rules! impl_call_return  {
    ($(($name:ident => $return:ty, $fn_name:expr, $deserialize_fn:ident)),* $(,)?) => {
        $(pub struct $name(CallTransaction);
        impl $name {
            pub(crate) fn call(contract: &RawContract) -> Self {
                Self(contract.call(&$fn_name))
            }

            #[must_use]
            pub fn gas(mut self, gas: u64) -> Self {
                self.0 = self.0.gas(gas);
                self
            }

            #[must_use]
            pub fn max_gas(mut self) -> Self {
                self.0 = self.0.max_gas();
                self
            }

            #[must_use]
            pub fn deposit(mut self, deposit: near_workspaces::types::NearToken) -> Self {
                self.0 = self.0.deposit(deposit);
                self
            }

            #[must_use]
            pub fn args(mut self, args: Vec<u8>) -> Self {
                self.0 = self.0.args(args);
                self
            }

            #[must_use]
            pub fn args_json<S: serde::Serialize>(mut self, args: S) -> Self {
                self.0 = self.0.args_json(args);
                self
            }

            #[must_use]
            pub fn args_borsh<B: borsh::BorshSerialize>(mut self, args: B) -> Self {
                self.0 = self.0.args_borsh(args);
                self
            }

            pub async fn transact(self) -> anyhow::Result<ExecutionResult<$return>> {
                ExecutionResult::$deserialize_fn(self.0.transact().await?)
            }
        })*
    };
    ($(($name:ident, $fn_name:expr)),* $(,)?) => {
        $(pub struct $name(CallTransaction);
        impl $name {
            pub(crate) fn call(contract: &RawContract) -> Self {
                Self(contract.call(&$fn_name))
            }

            #[must_use]
            pub fn gas(mut self, gas: u64) -> Self {
                self.0 = self.0.gas(gas);
                self
            }

            #[must_use]
            pub fn max_gas(mut self) -> Self {
                self.0 = self.0.max_gas();
                self
            }

            #[must_use]
            pub fn deposit(mut self, deposit: near_workspaces::types::NearToken) -> Self {
                self.0 = self.0.deposit(deposit);
                self
            }

            #[must_use]
            pub fn args(mut self, args: Vec<u8>) -> Self {
                self.0 = self.0.args(args);
                self
            }

            #[must_use]
            pub fn args_json<S: serde::Serialize>(mut self, args: S) -> Self {
                self.0 = self.0.args_json(args);
                self

            }

            #[must_use]
            pub fn args_borsh<B: borsh::BorshSerialize>(mut self, args: B) -> Self {
                self.0 = self.0.args_borsh(args);
                self
            }

            pub async fn transact(self) -> anyhow::Result<ExecutionResult<()>> {
                let result = self.0.transact().await?;
                let success = result.is_success();
                let inner = result.into_result()?;
                Ok(ExecutionResult::new(inner, (), success))
            }
        })*
    };
}
