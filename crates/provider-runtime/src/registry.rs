use crate::adapter::ProviderAdapter;
use crate::descriptor::AdapterDescriptor;

/// Ordered set of provider adapters.
///
/// The registry is the single source of provider identity: ids, display
/// names, ordering and capabilities are read from the registered
/// descriptors instead of being restated in the application layer or in the
/// Tauri commands. Registration validates the descriptor and rejects
/// duplicate ids, so two adapters can never fight over one provider id.
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

    /// Registers an adapter. Fails when its descriptor is inconsistent or
    /// when its id is already taken; the adapter is not registered in that
    /// case.
    pub fn register(&mut self, adapter: Box<dyn ProviderAdapter>) -> Result<(), String> {
        let descriptor = adapter.descriptor();
        descriptor.check()?;
        if self.adapters.iter().any(|a| a.id() == descriptor.id) {
            return Err(format!("duplicate provider id: {}", descriptor.id));
        }
        self.adapters.push(adapter);
        Ok(())
    }

    pub fn adapters(&self) -> &[Box<dyn ProviderAdapter>] {
        &self.adapters
    }

    /// Adapter references in registration order, ready for the refresh
    /// pipeline.
    pub fn refs(&self) -> Vec<&dyn ProviderAdapter> {
        self.adapters.iter().map(|a| a.as_ref()).collect()
    }

    /// Descriptors in registration order. This is the canonical provider
    /// list used for display names and table ordering.
    pub fn descriptors(&self) -> Vec<AdapterDescriptor> {
        self.adapters.iter().map(|a| a.descriptor()).collect()
    }

    /// Looks up one adapter by canonical id.
    pub fn find(&self, provider_id: &str) -> Option<&dyn ProviderAdapter> {
        self.adapters
            .iter()
            .find(|a| a.id() == provider_id)
            .map(|a| a.as_ref())
    }

    /// Display name for a provider id, or `None` when the id is unknown to
    /// this build.
    pub fn display_name(&self, provider_id: &str) -> Option<&'static str> {
        self.adapters
            .iter()
            .find(|a| a.id() == provider_id)
            .map(|a| a.name())
    }

    /// Position of a provider id in registration order, used to sort read
    /// models consistently with the registry.
    pub fn rank(&self, provider_id: &str) -> Option<usize> {
        self.adapters.iter().position(|a| a.id() == provider_id)
    }

    pub fn len(&self) -> usize {
        self.adapters.len()
    }

    pub fn is_empty(&self) -> bool {
        self.adapters.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::descriptor::{AuthKind, ChannelSupport, SourceKind};

    struct Sample(AdapterDescriptor);

    impl ProviderAdapter for Sample {
        fn descriptor(&self) -> AdapterDescriptor {
            self.0
        }
    }

    fn descriptor(id: &'static str, name: &'static str) -> AdapterDescriptor {
        AdapterDescriptor {
            id,
            display_name: name,
            vendor: "Vendor",
            source_kind: SourceKind::LocalSqlite,
            usage_support: ChannelSupport::LocalEstimate,
            quota_support: ChannelSupport::Unsupported,
            auth: AuthKind::LocalFiles,
            adapter_version: "0.2.0",
        }
    }

    #[test]
    fn registers_adapters_in_order() {
        let mut registry = AdapterRegistry::new();
        registry
            .register(Box::new(Sample(descriptor("alpha", "Alpha"))))
            .expect("alpha");
        registry
            .register(Box::new(Sample(descriptor("beta", "Beta"))))
            .expect("beta");

        assert_eq!(registry.len(), 2);
        assert_eq!(registry.rank("alpha"), Some(0));
        assert_eq!(registry.rank("beta"), Some(1));
        assert_eq!(registry.display_name("beta"), Some("Beta"));
        assert_eq!(registry.display_name("missing"), None);
        assert!(registry.find("alpha").is_some());
    }

    #[test]
    fn duplicate_ids_are_rejected() {
        let mut registry = AdapterRegistry::new();
        registry
            .register(Box::new(Sample(descriptor("alpha", "Alpha"))))
            .expect("first");
        let err = registry
            .register(Box::new(Sample(descriptor("alpha", "Alpha Clone"))))
            .expect_err("duplicate id must be rejected");
        assert!(err.contains("duplicate provider id"), "got {err}");
        assert_eq!(registry.len(), 1, "the duplicate must not be registered");
    }

    #[test]
    fn inconsistent_descriptors_are_rejected() {
        let mut registry = AdapterRegistry::new();
        let broken = AdapterDescriptor {
            usage_support: ChannelSupport::Unsupported,
            quota_support: ChannelSupport::Unsupported,
            ..descriptor("broken", "Broken")
        };
        assert!(
            registry.register(Box::new(Sample(broken))).is_err(),
            "an adapter that declares a source but collects nothing is rejected"
        );
        assert!(registry.is_empty());
    }
}
