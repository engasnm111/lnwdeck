pub mod adapter;
pub mod collection;
pub mod detection;
pub mod health;
pub mod permissions;
pub mod registry;
pub mod scheduler;
pub mod wasm;

pub use adapter::ProviderAdapter;
pub use collection::{CollectionOutcome, CollectionResult};
pub use detection::DetectionResult;
pub use health::{AdapterHealth, AdapterHealthStatus};
pub use permissions::{Permission, Permissions};
pub use registry::AdapterRegistry;
pub use scheduler::AdaptiveScheduler;
