use crate::adapter::ProviderAdapter;

pub struct AdapterRegistry {
    adapters: Vec<Box<dyn ProviderAdapter>>,
}

impl Default for AdapterRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl AdapterRegistry {
    pub fn new() -> Self {
        Self {
            adapters: Vec::new(),
        }
    }

    pub fn register(&mut self, adapter: Box<dyn ProviderAdapter>) {
        self.adapters.push(adapter);
    }

    pub fn adapters(&self) -> &[Box<dyn ProviderAdapter>] {
        &self.adapters
    }
}
