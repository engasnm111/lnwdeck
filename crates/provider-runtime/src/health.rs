/// Health of a provider integration.
///
/// `Unsupported` and `NotConfigured` exist so an adapter that cannot collect
/// anything is never reported as `Healthy`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdapterHealthStatus {
    /// Source reachable and readable.
    Healthy,
    /// Source expected but missing, or partially readable.
    Degraded,
    /// Source present but unusable (permission denied, schema mismatch).
    Unhealthy,
    /// The integration is not implemented for this provider.
    Unsupported,
    /// The integration exists but the user has not supplied its credentials.
    NotConfigured,
}

impl AdapterHealthStatus {
    /// Stable label used in diagnostics rows and in the provider table.
    pub fn label(self) -> &'static str {
        match self {
            Self::Healthy => "Healthy",
            Self::Degraded => "Degraded",
            Self::Unhealthy => "Unhealthy",
            Self::Unsupported => "Not supported",
            Self::NotConfigured => "Not configured",
        }
    }

    /// True when the adapter can be expected to produce data.
    pub fn can_collect(self) -> bool {
        matches!(self, Self::Healthy | Self::Degraded)
    }
}

#[derive(Debug, Clone)]
pub struct AdapterHealth {
    pub status: AdapterHealthStatus,
    pub message: String,
}
