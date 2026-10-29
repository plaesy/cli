//! Platform, backend and result types, and the legacy reach configuration shape.

use std::collections::BTreeMap;
use std::fmt;
use std::str::FromStr;
use std::time::Duration;

use chrono::{DateTime, Utc};

/// A data source platform.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Platform {
    Web,
    YouTube,
    GitHub,
    Rss,
    Twitter,
    Reddit,
    Bilibili,
    XiaoHongShu,
    LinkedIn,
    ExaSearch,
    V2ex,
    Xueqiu,
    Xiaoyuzhou,
    BossZhipin,
    Facebook,
    Instagram,
}

impl Platform {
    /// Every platform, in the order the registry lists them.
    pub const ALL: [Platform; 16] = [
        Platform::Web,
        Platform::YouTube,
        Platform::GitHub,
        Platform::Rss,
        Platform::Twitter,
        Platform::Reddit,
        Platform::Bilibili,
        Platform::XiaoHongShu,
        Platform::LinkedIn,
        Platform::ExaSearch,
        Platform::V2ex,
        Platform::Xueqiu,
        Platform::Xiaoyuzhou,
        Platform::BossZhipin,
        Platform::Facebook,
        Platform::Instagram,
    ];

    /// Platforms that work without credentials.
    pub const ZERO_CONFIG: [Platform; 6] = [
        Platform::Web,
        Platform::YouTube,
        Platform::GitHub,
        Platform::Rss,
        Platform::Bilibili,
        Platform::V2ex,
    ];

    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Platform::Web => "web",
            Platform::YouTube => "youtube",
            Platform::GitHub => "github",
            Platform::Rss => "rss",
            Platform::Twitter => "twitter",
            Platform::Reddit => "reddit",
            Platform::Bilibili => "bilibili",
            Platform::XiaoHongShu => "xiaohongshu",
            Platform::LinkedIn => "linkedin",
            Platform::ExaSearch => "exa",
            Platform::V2ex => "v2ex",
            Platform::Xueqiu => "xueqiu",
            Platform::Xiaoyuzhou => "xiaoyuzhou",
            Platform::BossZhipin => "bosszhipin",
            Platform::Facebook => "facebook",
            Platform::Instagram => "instagram",
        }
    }

    /// Whether the platform needs credentials to work.
    #[must_use]
    pub fn requires_auth(self) -> bool {
        matches!(
            self,
            Platform::Twitter
                | Platform::Reddit
                | Platform::XiaoHongShu
                | Platform::LinkedIn
                | Platform::ExaSearch
                | Platform::Xiaoyuzhou
                | Platform::BossZhipin
                | Platform::Facebook
                | Platform::Instagram
        )
    }
}

impl fmt::Display for Platform {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for Platform {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Platform::ALL
            .into_iter()
            .find(|p| p.as_str() == s)
            .ok_or_else(|| format!("unknown platform {s}"))
    }
}

/// A backend name, such as `jina-reader` or `yt-dlp`.
pub type Backend = String;

/// Query options. Keys such as `action`, `limit`, `timeout`, `cache`, and `cache_ttl` are read by
/// the adapters and the router.
pub type Options = BTreeMap<String, String>;

/// The outcome of one backend call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackendResult {
    pub backend: Backend,
    pub platform: Platform,
    pub success: bool,
    pub data: String,
    pub error: String,
    pub latency: Duration,
    pub timestamp: DateTime<Utc>,
}

impl BackendResult {
    /// An empty result for `backend`, timestamped now.
    #[must_use]
    pub fn new(backend: &str, platform: Platform) -> Self {
        Self {
            backend: backend.to_owned(),
            platform,
            success: false,
            data: String::new(),
            error: String::new(),
            latency: Duration::ZERO,
            timestamp: Utc::now(),
        }
    }
}

/// The health of one backend.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackendHealth {
    pub backend: Backend,
    pub platform: Platform,
    pub available: bool,
    pub last_checked: DateTime<Utc>,
    pub error: String,
    pub latency: Duration,
    pub version: String,
    pub requires_auth: bool,
}

impl BackendHealth {
    /// A health record for `backend` with nothing else known.
    #[must_use]
    pub fn new(backend: &str, platform: Platform) -> Self {
        Self {
            backend: backend.to_owned(),
            platform,
            available: false,
            last_checked: Utc::now(),
            error: String::new(),
            latency: Duration::ZERO,
            version: String::new(),
            requires_auth: platform.requires_auth(),
        }
    }
}

/// Rate limit for one platform, in the legacy (per-second) form.
#[derive(Debug, Clone, PartialEq)]
pub struct RateLimitConfig {
    pub requests_per_second: f64,
    pub burst: usize,
    pub window: Duration,
    /// `token-bucket` or `sliding-window`.
    pub algorithm: String,
}

/// Retry policy in the legacy form.
#[derive(Debug, Clone, PartialEq)]
pub struct RetryConfig {
    pub max_retries: u32,
    pub base_delay: Duration,
    pub max_delay: Duration,
    pub multiplier: f64,
    pub retryable_errors: Vec<String>,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            base_delay: Duration::from_millis(100),
            max_delay: Duration::from_secs(5),
            multiplier: 2.0,
            retryable_errors: [
                "timeout",
                "connection refused",
                "connection reset",
                "temporary failure",
                "too many requests",
                "rate limit",
                "503",
                "504",
                "502",
            ]
            .map(String::from)
            .to_vec(),
        }
    }
}

/// One platform's settings in the legacy form.
#[derive(Debug, Clone, PartialEq)]
pub struct PlatformConfig {
    pub platform: Platform,
    pub enabled: bool,
    pub primary_backend: Backend,
    pub fallback_backends: Vec<Backend>,
    pub credentials: BTreeMap<String, String>,
    pub options: BTreeMap<String, String>,
    pub timeout: Duration,
    pub cache_ttl: Duration,
    pub rate_limit: Option<RateLimitConfig>,
    pub retry_config: Option<RetryConfig>,
    pub max_retries: u32,
}

/// The whole reach configuration in the legacy form.
#[derive(Debug, Clone, PartialEq)]
pub struct ReachConfig {
    pub platforms: BTreeMap<Platform, PlatformConfig>,
    pub global_options: BTreeMap<String, String>,
    pub default_timeout: Duration,
    pub default_cache_ttl: Duration,
    pub global_rate_limit: Option<RateLimitConfig>,
    pub global_retry_config: Option<RetryConfig>,
    pub cache_dir: String,
    pub cache_size_mb: i64,
    pub cache_memory_size: i64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn platform_names_round_trip() {
        for p in Platform::ALL {
            assert_eq!(p.as_str().parse::<Platform>(), Ok(p));
        }
        assert!("nope".parse::<Platform>().is_err());
    }

    #[test]
    fn auth_lists_match_the_go_tables() {
        assert!(Platform::Twitter.requires_auth());
        assert!(!Platform::Web.requires_auth());
        assert!(Platform::ZERO_CONFIG.iter().all(|p| !p.requires_auth()));
    }
}
