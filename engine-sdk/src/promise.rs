use aurora_engine_types::parameters::{
    PromiseBatchAction, PromiseCreateArgs, PromiseWithCallbackArgs,
};
use aurora_engine_types::types::PromiseResult;

#[derive(Debug, Eq, PartialEq, Copy, Clone)]
pub struct PromiseId(u64);

impl PromiseId {
    pub fn new(id: u64) -> Self {
        Self(id)
    }

    pub fn raw(self) -> u64 {
        self.0
    }
}

pub trait PromiseHandler {
    fn promise_results_count(&self) -> u64;
    fn promise_result(&self, index: u64) -> Option<PromiseResult>;

    fn promise_create_call(&mut self, args: &PromiseCreateArgs) -> PromiseId;
    fn promise_attach_callback(
        &mut self,
        base: PromiseId,
        callback: &PromiseCreateArgs,
    ) -> PromiseId;
    fn promise_create_batch(&mut self, args: &PromiseBatchAction) -> PromiseId;
    fn promise_return(&mut self, promise: PromiseId);

    fn promise_crate_with_callback(&mut self, args: &PromiseWithCallbackArgs) -> PromiseId {
        let base = self.promise_create_call(&args.base);
        self.promise_attach_callback(base, &args.callback)
    }
}
