use aurora_engine_types::borsh::BorshDeserialize;
use aurora_engine_types::types::Address;
use aurora_engine_types::{H256, U256};
use near_sdk::{json_types::U128, PromiseOrValue};
use serde::de::DeserializeOwned;
use workspaces::result::{ExecutionFinalResult, ExecutionOutcome, ViewResultDetails};
use workspaces::types::Gas;

#[derive(Debug, Eq, PartialOrd, PartialEq)]
pub struct ViewResult<T> {
    pub result: T,
    pub logs: Vec<String>,
}

impl<T: DeserializeOwned> ViewResult<T> {
    pub fn json(view: ViewResultDetails) -> anyhow::Result<Self> {
        Ok(Self {
            result: view.json()?,
            logs: view.logs,
        })
    }
}

impl<T: BorshDeserialize> ViewResult<T> {
    pub fn borsh(view: ViewResultDetails) -> anyhow::Result<Self> {
        Ok(Self {
            result: view.borsh()?,
            logs: view.logs,
        })
    }
}

impl ViewResult<Vec<u8>> {
    pub fn vec(view: ViewResultDetails) -> anyhow::Result<Self> {
        Ok(Self {
            result: view.result,
            logs: view.logs,
        })
    }
}

impl ViewResult<U256> {
    #[allow(non_snake_case)]
    pub fn borsh_U256(view: ViewResultDetails) -> anyhow::Result<Self> {
        let mut buf = [0u8; 32];
        buf.copy_from_slice(view.result.as_slice());
        Ok(Self {
            result: U256::from(buf),
            logs: view.logs,
        })
    }
}

impl ViewResult<H256> {
    #[allow(non_snake_case)]
    pub fn borsh_H256(view: ViewResultDetails) -> anyhow::Result<Self> {
        let mut buf = [0u8; 32];
        buf.copy_from_slice(view.result.as_slice());
        Ok(Self {
            result: H256::from(buf),
            logs: view.logs,
        })
    }
}

#[derive(Debug)]
pub struct ExecutionResult<T> {
    inner: workspaces::result::ExecutionSuccess,
    value: T,
    success: bool,
}

impl<T: DeserializeOwned> ExecutionResult<T> {
    pub fn json(result: ExecutionFinalResult) -> anyhow::Result<Self> {
        let success = result.is_success();
        let inner = result.into_result()?;
        let value = inner.json()?;
        Ok(Self::new(inner, value, success))
    }
}

impl TryFrom<ExecutionFinalResult> for ExecutionResult<PromiseOrValue<U128>> {
    type Error = anyhow::Error;

    fn try_from(result: ExecutionFinalResult) -> Result<Self, Self::Error> {
        let success = result.is_success();
        let inner = result.into_result()?;
        let res: U128 = inner.json()?;
        let value = PromiseOrValue::Value(res);
        Ok(Self::new(inner, value, success))
    }
}

impl<T: BorshDeserialize> ExecutionResult<T> {
    pub fn borsh(result: ExecutionFinalResult) -> anyhow::Result<Self> {
        let success = result.is_success();
        let inner = result.into_result()?;
        let value = inner.borsh()?;
        Ok(Self::new(inner, value, success))
    }
}

impl ExecutionResult<Address> {
    pub fn borsh_address(result: ExecutionFinalResult) -> anyhow::Result<Self> {
        let success = result.is_success();
        let inner = result.into_result()?;
        let bytes: Vec<u8> = inner.borsh()?;
        let value = Address::try_from_slice(&bytes)
            .map_err(|e| anyhow::anyhow!("Error while creating an address from slice: {e}"))?;
        Ok(Self::new(inner, value, success))
    }
}

impl<T> ExecutionResult<T> {
    pub fn new(inner: workspaces::result::ExecutionSuccess, value: T, success: bool) -> Self {
        Self {
            inner,
            value,
            success,
        }
    }

    pub fn value(&self) -> &T {
        &self.value
    }

    pub fn into_value(self) -> T {
        self.value
    }

    pub fn total_gas_burnt(&self) -> Gas {
        self.inner.total_gas_burnt
    }

    pub fn outcome(&self) -> &ExecutionOutcome {
        self.inner.outcome()
    }

    pub fn outcomes(&self) -> Vec<&ExecutionOutcome> {
        self.inner.outcomes()
    }

    pub fn receipt_outcomes(&self) -> &[ExecutionOutcome] {
        self.inner.receipt_outcomes()
    }

    pub fn failures(&self) -> Vec<&ExecutionOutcome> {
        self.inner.failures()
    }

    pub fn receipt_failures(&self) -> Vec<&ExecutionOutcome> {
        self.inner.receipt_failures()
    }

    pub fn logs(&self) -> Vec<&str> {
        self.inner.logs()
    }

    pub fn is_success(&self) -> bool {
        self.success
    }

    pub fn is_failure(&self) -> bool {
        !self.success
    }
}
