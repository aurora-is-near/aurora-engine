use aurora_engine_sdk::promise::{PromiseHandler, PromiseId};
use aurora_engine_types::parameters::{PromiseBatchAction, PromiseCreateArgs};
use aurora_engine_types::types::PromiseResult;

/// Implements `PromiseHandler` so that it can be used in the standalone engine implementation of
/// methods like `call`, however since the standalone engine cannot schedule promises in a
/// meaningful way, the mutable implementations are no-ops. Functionally, this is only an implementation
/// of `ReadOnlyPromiseHandler`, which is needed for the standalone engine to properly serve the
/// EVM precompile that gives back information on the results of promises (possibly scheduled using
/// the cross-contract calls feature).
#[derive(Debug, Clone, Copy)]
pub struct NoScheduler<'a> {
    pub promise_data: &'a [Option<Vec<u8>>],
}

impl<'a> PromiseHandler for NoScheduler<'a> {
    type ReadOnly = Self;

    fn promise_results_count(&self) -> u64 {
        u64::try_from(self.promise_data.len()).unwrap_or_default()
    }

    fn promise_result(&self, index: u64) -> Option<PromiseResult> {
        let i = usize::try_from(index).ok()?;
        let result = match self.promise_data.get(i)? {
            Some(bytes) => PromiseResult::Successful(bytes.clone()),
            None => PromiseResult::Failed,
        };
        Some(result)
    }

    unsafe fn promise_create_call(&mut self, _args: &PromiseCreateArgs) -> PromiseId {
        PromiseId::new(0)
    }

    unsafe fn promise_attach_callback(
        &mut self,
        _base: PromiseId,
        _callback: &PromiseCreateArgs,
    ) -> PromiseId {
        PromiseId::new(0)
    }

    unsafe fn promise_create_batch(&mut self, _args: &PromiseBatchAction) -> PromiseId {
        PromiseId::new(0)
    }

    fn promise_return(&mut self, _promise: PromiseId) {}

    fn read_only(&self) -> Self::ReadOnly {
        *self
    }
}
