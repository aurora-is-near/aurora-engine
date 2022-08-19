use aurora_engine_types::types::EthGas;
use evm::{Capture, Opcode};
use std::cell::RefCell;
use std::ptr::NonNull;
use std::rc::Rc;

use crate::types::{
    LogStorageKey, LogStorageValue, Logs, ProgramCounter, TraceLog, TransactionTrace,
};

/// Capture all events from SputnikVM emitted from within the given closure using the given listener.
pub fn traced_call<T, R, F>(listener: &mut T, f: F) -> R
where
    T: evm_gasometer::tracing::EventListener
        + evm_runtime::tracing::EventListener
        + evm::tracing::EventListener
        + 'static,
    F: FnOnce() -> R,
{
    let mut gas_listener = SharedMutableReference::new(listener);
    let mut runtime_listener = gas_listener.clone();
    let mut evm_listener = gas_listener.clone();

    evm_gasometer::tracing::using(&mut gas_listener, || {
        evm_runtime::tracing::using(&mut runtime_listener, || {
            evm::tracing::using(&mut evm_listener, f)
        })
    })
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TransactionTraceBuilder {
    logs: Vec<TraceLog>,
    current: TraceLog,
    current_memory_gas: u64,
    gas_used: EthGas,
    failed: bool,
    output: Vec<u8>,
}

impl TransactionTraceBuilder {
    pub fn finish(self) -> TransactionTrace {
        TransactionTrace::new(self.gas_used, self.failed, self.output, Logs(self.logs))
    }
}

impl evm_gasometer::tracing::EventListener for TransactionTraceBuilder {
    fn event(&mut self, event: evm_gasometer::tracing::Event) {
        use evm_gasometer::tracing::Event;
        match event {
            Event::RecordCost { cost, snapshot } => {
                self.current.gas_cost = EthGas::new(cost);
                if let Some(snapshot) = snapshot {
                    self.current.gas =
                        EthGas::new(snapshot.gas_limit - snapshot.used_gas - snapshot.memory_gas);
                }
            }
            Event::RecordDynamicCost {
                gas_cost,
                memory_gas,
                gas_refund: _,
                snapshot,
            } => {
                // In SputnikVM memory gas is cumulative (ie this event always shows the total) gas
                // spent on memory up to this point. But geth traces simply show how much gas each step
                // took, regardless of how that gas was used. So if this step caused an increase to the
                // memory gas then we need to record that.
                let memory_cost_diff = if memory_gas > self.current_memory_gas {
                    memory_gas - self.current_memory_gas
                } else {
                    0
                };
                self.current_memory_gas = memory_gas;
                self.current.gas_cost = EthGas::new(gas_cost + memory_cost_diff);
                if let Some(snapshot) = snapshot {
                    self.current.gas =
                        EthGas::new(snapshot.gas_limit - snapshot.used_gas - snapshot.memory_gas);
                }
            }
            Event::RecordRefund {
                refund: _,
                snapshot,
            } => {
                // This one seems to show up at the end of a transaction, so it
                // can be used to set the total gas used.
                if let Some(snapshot) = snapshot {
                    self.gas_used = EthGas::new(snapshot.used_gas);
                }
            }
            Event::RecordTransaction { .. } => (), // not useful
            Event::RecordStipend { .. } => (),     // not useful
        }
    }
}

impl evm_runtime::tracing::EventListener for TransactionTraceBuilder {
    fn event(&mut self, event: evm_runtime::tracing::Event) {
        use evm_runtime::tracing::Event;
        match event {
            Event::Step {
                address: _,
                opcode,
                position,
                stack,
                memory,
            } => {
                self.current.opcode = opcode;
                if let Ok(pc) = position {
                    self.current.program_counter = ProgramCounter(*pc as u32);
                }
                self.current.stack = stack
                    .data()
                    .iter()
                    .map(|x| {
                        let mut buf = [0u8; 32];
                        x.to_big_endian(&mut buf);
                        buf
                    })
                    .collect();
                self.current.memory = memory.data().as_slice().into();
            }

            Event::StepResult {
                result,
                return_value,
            } => {
                match result {
                    Ok(_) => {
                        // Step completed, push current log into the record
                        self.logs.push(self.current.clone());
                    }
                    Err(Capture::Exit(reason)) => {
                        // Step completed, push current log into the record
                        self.logs.push(self.current.clone());
                        // Current sub-call completed, reduce depth by 1
                        self.current.depth.decrement();

                        // if the depth is 0 then the transaction is complete
                        if self.current.depth.is_zero() {
                            if !return_value.is_empty() {
                                self.output = return_value.to_vec();
                            }
                            if !reason.is_succeed() {
                                self.failed = true;
                            }
                        }
                    }
                    Err(Capture::Trap(opcode)) => {
                        // "Trap" here means that there is some opcode which has special
                        // handling logic outside the core `step` function. This means the
                        // `StepResult` does not necessarily indicate the current log
                        // is finished yet. In particular, `SLoad` and `SStore` events come
                        // _after_ the `StepResult`, but still correspond to the current step.
                        if opcode == &Opcode::SLOAD || opcode == &Opcode::SSTORE {
                            // will push the log after processing `SLOAD` / `SSTORE` events
                        } else {
                            self.logs.push(self.current.clone());
                        }
                    }
                }
            }

            Event::SLoad {
                address: _,
                index,
                value,
            } => {
                self.current
                    .storage
                    .insert(LogStorageKey(index.0), LogStorageValue(value.0));
                self.logs.push(self.current.clone());
            }

            Event::SStore {
                address: _,
                index,
                value,
            } => {
                self.current
                    .storage
                    .insert(LogStorageKey(index.0), LogStorageValue(value.0));
                self.logs.push(self.current.clone());
            }
        }
    }
}

impl evm::tracing::EventListener for TransactionTraceBuilder {
    fn event(&mut self, event: evm::tracing::Event) {
        use evm::tracing::Event;
        match event {
            Event::Call { .. } => {
                self.current.depth.increment();
            }
            Event::Create { .. } => {
                self.current.depth.increment();
            }
            Event::Suicide { .. } => (), // TODO: ???
            Event::Exit {
                reason: _,
                return_value,
            } => {
                if !self.current.depth.is_zero() {
                    // If the depth is not zero then an error must have occurred to
                    // exit early.
                    self.failed = true;
                    self.output = return_value.to_vec();
                }
            }
            Event::PrecompileSubcall { .. } => (),
            Event::TransactCall { .. } => (), // no useful information
            Event::TransactCreate { .. } => (), // no useful information
            Event::TransactCreate2 { .. } => (), // no useful information
        }
    }
}

/// This structure is intentionally private to this module as it is memory unsafe (contains a raw pointer).
/// Its purpose here is to allow a single event handling object to be used as the listener for
/// all SputnikVM events. It is needed because the listener must be passed as an object with a `'static`
/// lifetime, hence a normal reference cannot be used and we resort to raw pointers. The usage of this
/// struct in this module is safe because the `SharedMutableReference` objects created do not outlive
/// the reference they are based on (see `pub fn traced_call`). Moreover, because the SputnikVM code
/// is single-threaded, we do not need to worry about race conditions.
struct SharedMutableReference<T> {
    pointer: Rc<RefCell<NonNull<T>>>,
}

impl<T> SharedMutableReference<T> {
    fn new(reference: &mut T) -> Self {
        let ptr = NonNull::new(reference as _).unwrap();
        Self {
            pointer: Rc::new(RefCell::new(ptr)),
        }
    }

    fn clone(&self) -> Self {
        Self {
            pointer: Rc::clone(&self.pointer),
        }
    }
}

impl<T: evm_gasometer::tracing::EventListener> evm_gasometer::tracing::EventListener
    for SharedMutableReference<T>
{
    fn event(&mut self, event: evm_gasometer::tracing::Event) {
        unsafe {
            self.pointer.borrow_mut().as_mut().event(event);
        }
    }
}

impl<T: evm_runtime::tracing::EventListener> evm_runtime::tracing::EventListener
    for SharedMutableReference<T>
{
    fn event(&mut self, event: evm_runtime::tracing::Event) {
        unsafe {
            self.pointer.borrow_mut().as_mut().event(event);
        }
    }
}

impl<T: evm::tracing::EventListener> evm::tracing::EventListener for SharedMutableReference<T> {
    fn event(&mut self, event: evm::tracing::Event) {
        unsafe {
            self.pointer.borrow_mut().as_mut().event(event);
        }
    }
}
