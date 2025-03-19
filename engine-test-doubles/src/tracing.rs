#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Listener {
    pub events: Vec<String>,
}

impl aurora_evm::gasometer::tracing::EventListener for Listener {
    fn event(&mut self, event: aurora_evm::gasometer::tracing::Event) {
        self.events.push(format!("{event:?}"));
    }
}

impl aurora_evm::runtime::tracing::EventListener for Listener {
    fn event(&mut self, event: aurora_evm::runtime::tracing::Event) {
        self.events.push(format!("{event:?}"));
    }
}

impl aurora_evm::tracing::EventListener for Listener {
    fn event(&mut self, event: aurora_evm::tracing::Event) {
        self.events.push(format!("{event:?}"));
    }
}
