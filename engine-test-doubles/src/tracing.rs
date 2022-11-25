#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Listener {
    pub events: Vec<String>,
}

impl evm_gasometer::tracing::EventListener for Listener {
    fn event(&mut self, event: evm_gasometer::tracing::Event) {
        self.events.push(format!("{:?}", event));
    }
}

impl evm_runtime::tracing::EventListener for Listener {
    fn event(&mut self, event: evm_runtime::tracing::Event) {
        self.events.push(format!("{:?}", event));
    }
}

impl evm::tracing::EventListener for Listener {
    fn event(&mut self, event: evm::tracing::Event) {
        self.events.push(format!("{:?}", event));
    }
}
