#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdapterHealthStatus {
    Healthy,
    Degraded,
    Unhealthy,
}

#[derive(Debug, Clone)]
pub struct AdapterHealth {
    pub status: AdapterHealthStatus,
    pub message: String,
}
