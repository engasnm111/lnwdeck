use lnwdeck_domain::{QuotaKind, QuotaReport, QuotaWindow, QuotaWindowScope, DEFAULT_FRESHNESS};
use lnwdeck_provider_runtime::{
    AdapterDescriptor, AdapterHealth, AdapterHealthStatus, AuthKind, ChannelSupport, Permission,
    ProviderAdapter, SourceKind,
};
use std::net::{SocketAddr, TcpStream};
use std::time::Duration;

const DEFAULT_HOST: &str = "127.0.0.1:11434";
const ADAPTER_VERSION: &str = "0.2.0";

/// Ollama is a local model runtime without subscription quota. When the
/// local API is reachable the adapter reports a genuine `Local / Unlimited`
/// window; when it is not, it reports no quota rather than a fabricated bar.
pub struct OllamaAdapter {
    addr: SocketAddr,
}

impl Default for OllamaAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl OllamaAdapter {
    pub fn new() -> Self {
        let addr = std::env::var("OLLAMA_HOST")
            .ok()
            .and_then(|host| host.parse().ok())
            .unwrap_or_else(|| DEFAULT_HOST.parse().expect("valid default host"));
        Self { addr }
    }

    /// Test helper: pin the probe target to an explicit address.
    #[allow(dead_code)]
    fn with_addr(addr: SocketAddr) -> Self {
        Self { addr }
    }

    /// Bounded reachability probe; never blocks longer than `timeout`.
    fn probe(&self, timeout: Duration) -> bool {
        TcpStream::connect_timeout(&self.addr, timeout).is_ok()
    }

    fn unlimited_report() -> QuotaReport {
        QuotaReport::new(
            "ollama_local",
            "local_api",
            vec![QuotaWindow::unlimited(
                QuotaWindowScope::Other,
                QuotaKind::Requests,
            )],
            DEFAULT_FRESHNESS,
        )
    }
}

impl ProviderAdapter for OllamaAdapter {
    fn descriptor(&self) -> AdapterDescriptor {
        AdapterDescriptor {
            id: "ollama_local",
            display_name: "Ollama",
            vendor: "Ollama",
            source_kind: SourceKind::LocalApi,
            // The local API exposes running models, not a request history.
            usage_support: ChannelSupport::Unsupported,
            quota_support: ChannelSupport::Native,
            auth: AuthKind::None,
            adapter_version: ADAPTER_VERSION,
        }
    }
    fn collect_quota(&self) -> Result<Option<QuotaReport>, String> {
        if self.probe(Duration::from_millis(500)) {
            Ok(Some(Self::unlimited_report()))
        } else {
            Ok(None)
        }
    }
    fn health_check(&self) -> AdapterHealth {
        if self.probe(Duration::from_millis(500)) {
            AdapterHealth {
                status: AdapterHealthStatus::Healthy,
                message: "Local server reachable".to_string(),
            }
        } else {
            AdapterHealth {
                status: AdapterHealthStatus::Degraded,
                message: "Local server not reachable".to_string(),
            }
        }
    }
    fn required_permissions(&self) -> Vec<Permission> {
        vec![Permission::Network]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;

    /// A port with no listener is deterministically unreachable.
    fn free_addr() -> SocketAddr {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind ephemeral");
        let addr = listener.local_addr().expect("addr");
        drop(listener);
        addr
    }

    #[test]
    fn id_is_correct() {
        assert_eq!(OllamaAdapter::new().id(), "ollama_local");
    }

    #[test]
    fn requires_network() {
        assert!(OllamaAdapter::new()
            .required_permissions()
            .contains(&Permission::Network));
    }

    #[test]
    fn unreachable_local_api_reports_no_quota() {
        let adapter = OllamaAdapter::with_addr(free_addr());
        assert!(
            adapter.collect_quota().expect("quota call").is_none(),
            "must not fabricate quota when the local API is down"
        );
        assert_eq!(
            adapter.health_check().status,
            AdapterHealthStatus::Degraded,
            "health reflects reachability"
        );
    }

    #[test]
    fn reachable_local_api_reports_unlimited() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let addr = listener.local_addr().expect("addr");
        let adapter = OllamaAdapter::with_addr(addr);
        let report = adapter
            .collect_quota()
            .expect("quota call")
            .expect("report");
        assert_eq!(report.status, lnwdeck_domain::QuotaStatus::Fresh);
        assert_eq!(report.windows.len(), 1);
        assert!(
            report.windows[0].is_unlimited,
            "local provider is unlimited"
        );
        assert_eq!(report.windows[0].label, "Unlimited");
        assert!(
            lnwdeck_security::PrivacyGuard::validate_quota_report(&report).is_ok(),
            "unlimited report must pass the privacy guard"
        );
    }
}
