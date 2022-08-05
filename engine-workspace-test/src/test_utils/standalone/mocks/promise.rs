use aurora_engine_sdk::promise::PromiseHandler;
use aurora_engine_sdk::promise::PromiseId;
use aurora_engine_types::parameters::{PromiseBatchAction, PromiseCreateArgs};
use aurora_engine_types::types::PromiseResult;
use std::collections::HashMap;

pub enum PromiseArgs {
    Create(PromiseCreateArgs),
    #[allow(dead_code)]
    Callback {
        base: PromiseId,
        callback: PromiseCreateArgs,
    },
    Batch(PromiseBatchAction),
}

/// Doesn't actually schedule any promises, only tracks what promises should be scheduled
#[derive(Default)]
pub struct PromiseTracker {
    internal_index: u64,
    pub promise_results: Vec<PromiseResult>,
    pub scheduled_promises: HashMap<u64, PromiseArgs>,
    pub returned_promise: Option<PromiseId>,
}

impl PromiseTracker {
    fn take_id(&mut self) -> u64 {
        let id = self.internal_index;
        self.internal_index += 1;
        id
    }
}

impl PromiseHandler for PromiseTracker {
    fn promise_results_count(&self) -> u64 {
        self.promise_results.len() as u64
    }

    fn promise_result(&self, index: u64) -> Option<PromiseResult> {
        self.promise_results.get(index as usize).cloned()
    }

    fn promise_create_call(&mut self, args: &PromiseCreateArgs) -> PromiseId {
        let id = self.take_id();
        self.scheduled_promises
            .insert(id, PromiseArgs::Create(args.clone()));
        PromiseId::new(id)
    }

    fn promise_attach_callback(
        &mut self,
        base: PromiseId,
        callback: &aurora_engine_types::parameters::PromiseCreateArgs,
    ) -> aurora_engine_sdk::promise::PromiseId {
        let id = self.take_id();
        self.scheduled_promises.insert(
            id,
            PromiseArgs::Callback {
                base,
                callback: callback.clone(),
            },
        );
        PromiseId::new(id)
    }

    fn promise_create_batch(&mut self, args: &PromiseBatchAction) -> PromiseId {
        let id = self.take_id();
        self.scheduled_promises
            .insert(id, PromiseArgs::Batch(args.clone()));
        PromiseId::new(id)
    }

    fn promise_return(&mut self, promise: PromiseId) {
        self.returned_promise = Some(promise);
    }
}
