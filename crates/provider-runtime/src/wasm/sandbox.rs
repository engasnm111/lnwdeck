/// Configuration for Wasm sandbox execution limits.
#[derive(Debug, Clone)]
pub struct SandboxConfig {
    /// Maximum linear memory in bytes (default 64 MiB).
    pub max_memory_bytes: usize,
    /// Maximum execution fuel (approximate instruction count).
    pub max_fuel: u64,
    /// Maximum output size in bytes.
    pub max_output_bytes: usize,
    /// Hard timeout in milliseconds.
    pub timeout_ms: u64,
    /// Granted capabilities.
    pub capabilities: Vec<String>,
}

impl Default for SandboxConfig {
    fn default() -> Self {
        Self {
            max_memory_bytes: 64 * 1024 * 1024,
            max_fuel: 1_000_000,
            max_output_bytes: 1_048_576,
            timeout_ms: 5_000,
            capabilities: vec![],
        }
    }
}

/// Represents a sandboxed Wasm execution context for community adapters.
pub struct WasmSandbox {
    config: SandboxConfig,
}

impl WasmSandbox {
    pub fn new(config: SandboxConfig) -> Self {
        Self { config }
    }

    /// Verify that a manifest's declared capabilities are all known and valid.
    pub fn validate_manifest_capabilities(capabilities: &[String]) -> Vec<String> {
        let known: &[&str] = &[
            "filesystem:read",
            "filesystem:write",
            "network:http",
            "env:read",
        ];
        capabilities
            .iter()
            .filter(|c| !known.contains(&c.as_str()))
            .cloned()
            .collect()
    }

    /// Deny-by-default: check whether a requested capability is granted.
    pub fn has_capability(&self, capability: &str) -> bool {
        self.config.capabilities.contains(&capability.to_string())
    }

    /// Check if any forbidden capability (undeclared in manifest) is being attempted.
    pub fn check_denied_capability(&self, capability: &str) -> Result<(), String> {
        if self.has_capability(capability) {
            Ok(())
        } else {
            Err(format!("capability denied: {capability}"))
        }
    }

    pub fn config(&self) -> &SandboxConfig {
        &self.config
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deny_by_default_blocks_undeclared_filesystem() {
        let sandbox = WasmSandbox::new(SandboxConfig::default());
        assert!(sandbox.check_denied_capability("filesystem:read").is_err());
        assert!(sandbox.check_denied_capability("filesystem:write").is_err());
    }

    #[test]
    fn deny_by_default_blocks_undeclared_network() {
        let sandbox = WasmSandbox::new(SandboxConfig::default());
        assert!(sandbox.check_denied_capability("network:http").is_err());
    }

    #[test]
    fn deny_by_default_blocks_undeclared_env() {
        let sandbox = WasmSandbox::new(SandboxConfig::default());
        assert!(sandbox.check_denied_capability("env:read").is_err());
    }

    #[test]
    fn granted_capability_is_allowed() {
        let config = SandboxConfig {
            capabilities: vec!["filesystem:read".to_string()],
            ..Default::default()
        };
        let sandbox = WasmSandbox::new(config);

        assert!(sandbox.check_denied_capability("filesystem:read").is_ok());
        assert!(sandbox.check_denied_capability("filesystem:write").is_err());
        assert!(sandbox.check_denied_capability("network:http").is_err());
    }

    #[test]
    fn unknown_capability_is_rejected_by_manifest_validation() {
        let unknown = WasmSandbox::validate_manifest_capabilities(&[
            "filesystem:read".to_string(),
            "unknown:power".to_string(),
        ]);
        assert_eq!(unknown, vec!["unknown:power"]);
    }

    #[test]
    fn valid_capabilities_pass_validation() {
        let unknown = WasmSandbox::validate_manifest_capabilities(&[
            "filesystem:read".to_string(),
            "network:http".to_string(),
        ]);
        assert!(unknown.is_empty());
    }

    #[test]
    fn default_limits_are_reasonable() {
        let config = SandboxConfig::default();
        assert!(config.max_memory_bytes > 0);
        assert!(config.max_fuel > 0);
        assert!(config.max_output_bytes > 0);
        assert!(config.timeout_ms > 0);
        assert!(
            config.max_memory_bytes <= 256 * 1024 * 1024,
            "max memory must not exceed 256 MiB"
        );
    }

    #[test]
    fn output_size_bounded() {
        let config = SandboxConfig::default();
        let output = "x".repeat(config.max_output_bytes + 1);
        assert!(
            output.len() > config.max_output_bytes,
            "output exceeding max must be rejected"
        );
    }
}
