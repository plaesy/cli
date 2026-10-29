//! Reach: one interface to external data sources (web, social, code hosting, video, feeds,
//! search). Each platform has adapters that run CLIs or HTTP calls. The router tries the
//! configured backends in order, with retries, caching, and rate limits. Port of
//! `scripts/internal/reach` (Go).

pub mod adapter;
pub mod adapters;
pub mod cache;
pub mod config;
pub mod duration;
pub mod exec;
pub mod mcp;
pub mod ratelimit;
pub mod registry;
pub mod router;
pub mod rss;
pub mod types;

pub use adapter::PlatformAdapter;
pub use cache::{CacheBackend, CacheStats, FileCache, MemoryCache, TieredCache};
pub use config::ConfigManager;
pub use exec::Deadline;
pub use ratelimit::{PlatformRateLimiter, RateLimiter};
pub use registry::{Registry, RegistryOptions};
pub use router::Router;
pub use types::{Backend, BackendHealth, BackendResult, Options, Platform, ReachConfig};

/// Failures of a reach call, a configuration change, or the keyring.
#[derive(Debug, thiserror::Error)]
pub enum ReachError {
    #[error("no adapter registered for platform {0}")]
    NoAdapter(Platform),
    #[error("no backends configured for platform {0}")]
    NoBackends(Platform),
    #[error("unknown backend {backend} for {platform}")]
    UnknownBackend { backend: String, platform: Platform },
    #[error("unknown action {action} for {context}")]
    UnknownAction { action: String, context: String },
    #[error("rate limited: {0}")]
    RateLimited(#[from] ratelimit::WaitCancelled),
    #[error("all backends failed for platform {platform}: last error: {last}")]
    AllBackendsFailed { platform: Platform, last: String },
    #[error("context deadline exceeded")]
    DeadlineExceeded,
    #[error("max retries exceeded")]
    RetriesExhausted,
    #[error("MCP: {0}")]
    Mcp(String),
    #[error("keyring: {0}")]
    Keyring(String),
    #[error("credential not found")]
    CredentialNotFound,
    #[error("configuring platform {platform}: {source}")]
    Configure {
        platform: Platform,
        source: Box<ReachError>,
    },
    #[error(transparent)]
    Config(#[from] crate::config::ConfigError),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}
