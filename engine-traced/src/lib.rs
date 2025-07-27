#![no_std]

extern crate alloc;

#[cfg(feature = "contract")]
mod contract {
    use aurora_engine::{
        contract_methods::{self, ContractError},
        errors,
    };
    use aurora_engine_sdk::{io::IO, near_runtime::Runtime, types::SdkUnwrap};
    use engine_standalone_tracing::sputnik;

    #[no_mangle]
    extern "C" fn submit_trace_tx() {
        let mut io = Runtime;
        let env = Runtime;
        let mut handler = Runtime;

        let mut listener = sputnik::TransactionTraceBuilder::default();
        let submit_result = sputnik::traced_call(&mut listener, || {
            contract_methods::evm_transactions::submit(io, &env, &mut handler)
                .map_err(ContractError::msg)
                .sdk_unwrap()
        });
        let trace_log = listener.finish().logs().clone();
        let result_bytes = borsh::to_vec(&(submit_result, trace_log))
            .map_err(|_| errors::ERR_SERIALIZE)
            .sdk_unwrap();
        io.return_output(&result_bytes);
    }
}
