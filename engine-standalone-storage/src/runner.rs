use aurora_engine_sdk::{env::Env, io::IO};

pub trait AbstractContractRunner {
    type Error;

    fn call_contract<E, I>(
        &self,
        method: &str,
        promise_data: Vec<Option<Vec<u8>>>,
        env: &E,
        io: I,
        override_input: Option<Vec<u8>>,
    ) -> Result<Option<Vec<u8>>, Self::Error>
    where
        E: Env,
        I: IO + Send,
        I::StorageValue: AsRef<[u8]>;
}
