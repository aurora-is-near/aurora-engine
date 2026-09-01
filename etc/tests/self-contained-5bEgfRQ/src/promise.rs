use aurora_engine_sdk::promise::{PromiseHandler, PromiseId};
use aurora_engine_types::parameters::{PromiseBatchAction, PromiseCreateArgs};
use aurora_engine_types::types::PromiseResult;

/// A promise handler that does nothing. Should only be used when promises can be safely ignored.
#[derive(Copy, Clone)]
pub struct Noop;

impl PromiseHandler for Noop {
    type ReadOnly = Self;

    fn promise_results_count(&self) -> u64 {
        0
    }

    fn promise_result(&self, _index: u64) -> Option<PromiseResult> {
        None
    }

    fn promise_create_call(&mut self, _args: &PromiseCreateArgs) -> PromiseId {
        PromiseId::new(0)
    }

    fn promise_attach_callback(
        &mut self,
        _base: PromiseId,
        _callback: &PromiseCreateArgs,
    ) -> PromiseId {
        PromiseId::new(0)
    }

    fn promise_create_batch(&mut self, _args: &PromiseBatchAction) -> PromiseId {
        PromiseId::new(0)
    }

    fn promise_return(&mut self, _promise: PromiseId) {}

    fn promise_create_and_combine(&mut self, _args: &[PromiseCreateArgs]) -> PromiseId {
        PromiseId::new(0)
    }

    fn promise_attach_batch_callback(
        &mut self,
        _base: PromiseId,
        _args: &PromiseBatchAction,
    ) -> PromiseId {
        PromiseId::new(0)
    }

    fn read_only(&self) -> Self::ReadOnly {
        *self
    }
}
