use evm::Context;

pub fn new_context() -> Context {
    Context {
        address: Default::default(),
        caller: Default::default(),
        apparent_value: Default::default(),
    }
}
