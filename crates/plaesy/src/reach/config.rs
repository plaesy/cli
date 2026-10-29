//! Reach configuration: the unified `.plaesy/config.json` section, converted to and from the
//! legacy shape, with credentials kept in the OS keyring when it is enabled.

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::{Mutex, PoisonError};
use std::time::Duration;

use serde::Deserialize;

use super::duration;
use super::ratelimit::default_rate_limits;
use super::types::{Backend, Platform, PlatformConfig, RateLimitConfig, ReachConfig, RetryConfig};
use super::ReachError;
use crate::config::manager::ConfigManager as UnifiedManager;
use crate::config::schema::{self, UnifiedConfig};

/// The folder the first version of the reach config lived in, relative to the working directory.
pub const LEGACY_CONFIG_PATH: &str = ".plaesy/reach/config.json";

/// Placeholder stored in the config file in place of a credential that lives in the keyring.
pub const KEYRING_PLACEHOLDER: &str = "[keyring]";

const KEYRING_SERVICE: &str = "plaesy-reach";

/// The legacy file's JSON, with the field names the Go tool wrote. Durations are nanoseconds.
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct LegacyReach {
    #[serde(rename = "Platforms")]
    platforms: BTreeMap<String, LegacyPlatform>,
    default_timeout: u64,
    default_cache_ttl: u64,
    global_rate_limit: Option<LegacyRate>,
    global_retry_config: Option<LegacyRetry>,
    cache_dir: String,
    cache_size_mb: i64,
    cache_memory_size: i64,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct LegacyPlatform {
    #[serde(rename = "Enabled")]
    enabled: bool,
    #[serde(rename = "PrimaryBackend")]
    primary_backend: String,
    #[serde(rename = "FallbackBackends")]
    fallback_backends: Vec<String>,
    #[serde(rename = "Credentials")]
    credentials: BTreeMap<String, String>,
    #[serde(rename = "Options")]
    options: BTreeMap<String, String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct LegacyRate {
    requests_per_second: f64,
    burst: i64,
    window: u64,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct LegacyRetry {
    max_retries: i64,
    base_delay: u64,
    max_delay: u64,
    retryable_errors: Vec<String>,
}

fn nanos(n: u64) -> Duration {
    Duration::from_nanos(n)
}

/// The shipped default for each platform.
#[must_use]
pub fn default_config() -> ReachConfig {
    ReachConfig {
        platforms: Platform::ALL
            .into_iter()
            .map(|p| (p, default_platform_config(p)))
            .collect(),
        global_options: BTreeMap::new(),
        default_timeout: Duration::from_secs(30),
        default_cache_ttl: Duration::from_secs(5 * 60),
        global_rate_limit: None,
        global_retry_config: None,
        cache_dir: ".plaesy/reach/cache".to_owned(),
        cache_size_mb: 100,
        cache_memory_size: 1000,
    }
}

/// The shipped settings for one platform.
#[must_use]
pub fn default_platform_config(platform: Platform) -> PlatformConfig {
    PlatformConfig {
        platform,
        enabled: Platform::ZERO_CONFIG.contains(&platform),
        primary_backend: default_backend(platform).to_owned(),
        fallback_backends: fallback_backends(platform)
            .into_iter()
            .map(str::to_owned)
            .collect(),
        credentials: BTreeMap::new(),
        options: BTreeMap::new(),
        timeout: Duration::from_secs(30),
        cache_ttl: Duration::from_secs(5 * 60),
        rate_limit: default_rate_limits().remove(&platform),
        retry_config: Some(RetryConfig::default()),
        max_retries: 3,
    }
}

/// The backend a platform tries first.
#[must_use]
pub fn default_backend(platform: Platform) -> &'static str {
    match platform {
        Platform::Web => "jina-reader",
        Platform::YouTube => "yt-dlp",
        Platform::GitHub => "gh-cli",
        Platform::Rss => "builtin",
        Platform::Twitter => "twitter-cli",
        Platform::Reddit => "opencli",
        Platform::Bilibili => "bili-cli",
        Platform::XiaoHongShu => "opencli",
        Platform::LinkedIn => "mcp-linkedin",
        Platform::ExaSearch => "exa-mcp",
        Platform::V2ex => "v2ex-api",
        Platform::Xueqiu => "xueqiu-api",
        Platform::Xiaoyuzhou => "xiaoyuzhou-mcp",
        Platform::BossZhipin => "boss-cdp",
        Platform::Facebook | Platform::Instagram => "opencli",
    }
}

/// The backends a platform falls back to, after its primary.
#[must_use]
pub fn fallback_backends(platform: Platform) -> Vec<&'static str> {
    match platform {
        Platform::Twitter => vec!["opencli", "bird"],
        Platform::Reddit => vec!["rdt-cli"],
        Platform::Bilibili => vec!["opencli", "bili-api"],
        Platform::XiaoHongShu => vec!["xiaohongshu-mcp", "xhs-cli"],
        Platform::LinkedIn => vec!["jina-reader"],
        Platform::Rss => vec!["feedparser"],
        _ => Vec::new(),
    }
}

/// The credential keys a platform needs.
#[must_use]
pub fn credential_keys(platform: Platform) -> &'static [&'static str] {
    match platform {
        Platform::Twitter => &["auth_token", "ct0"],
        Platform::Reddit
        | Platform::XiaoHongShu
        | Platform::BossZhipin
        | Platform::Facebook
        | Platform::Instagram => &["cookie"],
        Platform::LinkedIn => &["mcp_config"],
        Platform::ExaSearch => &["api_key"],
        Platform::Xiaoyuzhou => &["whisper_key"],
        Platform::GitHub => &["token"],
        _ => &[],
    }
}

fn parse_or(text: &str, default: Duration) -> Duration {
    if text.is_empty() {
        return default;
    }
    duration::parse(text).unwrap_or(default)
}

fn rate_from_unified(rl: Option<&schema::RateLimitConfig>) -> Option<RateLimitConfig> {
    let rl = rl?;
    if rl.requests == 0 && rl.window.is_empty() {
        return None;
    }
    Some(RateLimitConfig {
        requests_per_second: rl.requests as f64 / 60.0,
        burst: usize::try_from(rl.burst).unwrap_or(0),
        window: parse_or(&rl.window, Duration::from_secs(60)),
        algorithm: "token-bucket".to_owned(),
    })
}

fn retry_from_unified(rc: Option<&schema::RetryConfig>) -> Option<RetryConfig> {
    let rc = rc?;
    Some(RetryConfig {
        max_retries: u32::try_from(rc.max_retries).unwrap_or(0),
        base_delay: parse_or(&rc.base_delay, Duration::from_millis(100)),
        max_delay: parse_or(&rc.max_delay, Duration::from_secs(5)),
        multiplier: 2.0,
        retryable_errors: rc.retryable_errors.clone(),
    })
}

/// The legacy shape of a unified reach section.
#[must_use]
pub fn from_unified(u: &schema::ReachConfig) -> ReachConfig {
    let platforms = u
        .platforms
        .iter()
        .filter_map(|(name, pc)| {
            let platform: Platform = name.parse().ok()?;
            let primary = if pc.default_backend.is_empty() {
                pc.backends.first().cloned().unwrap_or_default()
            } else {
                pc.default_backend.clone()
            };
            let fallback_backends: Vec<Backend> = pc
                .backends
                .iter()
                .filter(|b| **b != primary)
                .cloned()
                .collect();
            let config = PlatformConfig {
                platform,
                enabled: pc.enabled,
                primary_backend: primary,
                fallback_backends,
                credentials: pc.credentials.clone(),
                options: pc.options.clone(),
                timeout: parse_or(
                    u.timeouts.get(name).map_or("", String::as_str),
                    Duration::from_secs(30),
                ),
                cache_ttl: parse_or(&u.default_cache_ttl, Duration::from_secs(5 * 60)),
                rate_limit: rate_from_unified(u.rate_limits.get(name)),
                retry_config: retry_from_unified(u.retry_config.as_ref()),
                max_retries: 3,
            };
            Some((platform, config))
        })
        .collect();
    ReachConfig {
        platforms,
        global_options: BTreeMap::new(),
        default_timeout: Duration::from_secs(30),
        default_cache_ttl: parse_or(&u.default_cache_ttl, Duration::from_secs(5 * 60)),
        global_rate_limit: rate_from_unified(u.rate_limits.get("global")),
        global_retry_config: retry_from_unified(u.retry_config.as_ref()),
        cache_dir: u.cache_dir.clone(),
        cache_size_mb: u.cache_size_mb,
        cache_memory_size: u.cache_memory_size,
    }
}

fn rate_to_unified(rl: &RateLimitConfig) -> schema::RateLimitConfig {
    schema::RateLimitConfig {
        requests: (rl.requests_per_second * 60.0) as i64,
        window: duration::format(rl.window),
        burst: i64::try_from(rl.burst).unwrap_or(i64::MAX),
    }
}

fn retry_to_unified(rc: &RetryConfig) -> schema::RetryConfig {
    schema::RetryConfig {
        max_retries: i64::from(rc.max_retries),
        base_delay: duration::format(rc.base_delay),
        max_delay: duration::format(rc.max_delay),
        retryable_errors: rc.retryable_errors.clone(),
    }
}

/// The unified shape of a legacy reach config.
#[must_use]
pub fn to_unified(cfg: &ReachConfig) -> schema::ReachConfig {
    let mut platforms = BTreeMap::new();
    let mut rate_limits = BTreeMap::new();
    let mut timeouts = BTreeMap::new();
    for (platform, pc) in &cfg.platforms {
        let mut backends = vec![pc.primary_backend.clone()];
        backends.extend(pc.fallback_backends.iter().cloned());
        platforms.insert(
            platform.as_str().to_owned(),
            schema::ReachPlatformConfig {
                enabled: pc.enabled,
                backends,
                default_backend: pc.primary_backend.clone(),
                credentials: pc.credentials.clone(),
                options: pc.options.clone(),
            },
        );
        if let Some(rl) = &pc.rate_limit {
            rate_limits.insert(platform.as_str().to_owned(), rate_to_unified(rl));
        }
        timeouts.insert(platform.as_str().to_owned(), duration::format(pc.timeout));
    }
    if let Some(rl) = &cfg.global_rate_limit {
        rate_limits.insert("global".to_owned(), rate_to_unified(rl));
    }
    schema::ReachConfig {
        cache_dir: cfg.cache_dir.clone(),
        cache_size_mb: cfg.cache_size_mb,
        cache_memory_size: cfg.cache_memory_size,
        default_cache_ttl: duration::format(cfg.default_cache_ttl),
        platforms,
        rate_limits,
        timeouts,
        retry_config: cfg.global_retry_config.as_ref().map(retry_to_unified),
    }
}

/// Converts the legacy file into a unified config, for the one-time migration.
fn migrate_legacy(bytes: &[u8]) -> Result<UnifiedConfig, ReachError> {
    let legacy: LegacyReach = serde_json::from_slice(bytes)?;
    let mut platforms = BTreeMap::new();
    for (name, pc) in &legacy.platforms {
        platforms.insert(
            name.clone(),
            schema::ReachPlatformConfig {
                enabled: pc.enabled,
                backends: vec![pc.primary_backend.clone()],
                default_backend: pc.primary_backend.clone(),
                credentials: pc.credentials.clone(),
                options: pc.options.clone(),
            },
        );
    }
    let mut rate_limits = BTreeMap::new();
    if let Some(rl) = &legacy.global_rate_limit {
        rate_limits.insert(
            "global".to_owned(),
            schema::RateLimitConfig {
                requests: (rl.requests_per_second * 60.0) as i64,
                window: duration::format(nanos(rl.window)),
                burst: rl.burst,
            },
        );
    }
    let mut unified = UnifiedConfig::defaults();
    unified.reach = Some(schema::ReachConfig {
        cache_dir: legacy.cache_dir,
        cache_size_mb: legacy.cache_size_mb,
        cache_memory_size: legacy.cache_memory_size,
        default_cache_ttl: duration::format(nanos(legacy.default_cache_ttl)),
        platforms,
        rate_limits,
        timeouts: BTreeMap::new(),
        retry_config: legacy.global_retry_config.map(|rc| schema::RetryConfig {
            max_retries: rc.max_retries,
            base_delay: duration::format(nanos(rc.base_delay)),
            max_delay: duration::format(nanos(rc.max_delay)),
            retryable_errors: rc.retryable_errors,
        }),
    });
    Ok(unified)
}

/// The OS keyring, one entry per credential.
#[derive(Debug, Clone)]
pub struct Keyring {
    service: String,
}

impl Keyring {
    #[must_use]
    pub fn new(service: &str) -> Self {
        Self {
            service: service.to_owned(),
        }
    }

    fn entry(&self, platform: Platform, key: &str) -> Result<keyring::Entry, ReachError> {
        keyring::Entry::new(&self.service, &format!("{platform}/{key}"))
            .map_err(|e| ReachError::Keyring(e.to_string()))
    }

    /// # Errors
    ///
    /// The keyring refuses the write.
    pub fn set(&self, platform: Platform, key: &str, value: &str) -> Result<(), ReachError> {
        self.entry(platform, key)?
            .set_password(value)
            .map_err(|e| ReachError::Keyring(e.to_string()))
    }

    /// # Errors
    ///
    /// The entry is missing, or the keyring cannot be read.
    pub fn get(&self, platform: Platform, key: &str) -> Result<String, ReachError> {
        self.entry(platform, key)?
            .get_password()
            .map_err(|e| ReachError::Keyring(e.to_string()))
    }

    /// Removes an entry. A missing entry is not an error.
    pub fn delete(&self, platform: Platform, key: &str) {
        if let Ok(entry) = self.entry(platform, key) {
            let _ = entry.delete_credential();
        }
    }
}

/// Loads and saves the reach configuration.
pub struct ConfigManager {
    unified: UnifiedManager<UnifiedConfig>,
    use_keyring: bool,
    keyring: Keyring,
    cache: Mutex<Option<ReachConfig>>,
}

impl Default for ConfigManager {
    fn default() -> Self {
        Self::new()
    }
}

impl ConfigManager {
    /// The manager for the unified config file, with keyring integration on.
    #[must_use]
    pub fn new() -> Self {
        Self {
            unified: UnifiedManager::unified(UnifiedConfig::defaults()),
            use_keyring: true,
            keyring: Keyring::new(KEYRING_SERVICE),
            cache: Mutex::new(None),
        }
    }

    /// Turns keyring integration on or off. With it off, credentials stay in the config file.
    pub fn set_keyring_enabled(&mut self, enabled: bool) {
        self.use_keyring = enabled;
    }

    /// Imports the legacy reach file, if present, into the unified config.
    ///
    /// # Errors
    ///
    /// The legacy file exists but cannot be read or parsed, or the unified config cannot be saved.
    pub fn migrate_from_legacy(&self) -> Result<(), ReachError> {
        self.unified
            .migrate_from(Path::new(LEGACY_CONFIG_PATH), migrate_legacy)
            .map_err(ReachError::from)
    }

    /// The reach configuration, read from the unified file (migrating the legacy one first when
    /// the unified file has no reach platforms). Credentials are read back from the keyring.
    ///
    /// # Errors
    ///
    /// A config file cannot be read or parsed.
    pub fn load(&self) -> Result<ReachConfig, ReachError> {
        if let Some(cached) = self
            .cache
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
        {
            return Ok(cached);
        }
        let mut unified = self.unified.load()?;
        let mut has_reach = unified
            .reach
            .as_ref()
            .is_some_and(|r| !r.platforms.is_empty());
        if !has_reach {
            let legacy_present = Path::new(LEGACY_CONFIG_PATH).exists();
            self.migrate_from_legacy()?;
            unified = self.unified.load()?;
            has_reach = unified
                .reach
                .as_ref()
                .is_some_and(|r| !r.platforms.is_empty() || legacy_present);
        }
        let mut cfg = match (&unified.reach, has_reach) {
            (Some(reach), true) => from_unified(reach),
            _ => default_config(),
        };
        if self.use_keyring {
            for (platform, pc) in &mut cfg.platforms {
                for (key, stored) in pc.credentials.iter_mut() {
                    if let Ok(value) = self.keyring.get(*platform, key) {
                        *stored = value;
                    }
                }
            }
        }
        *self.cache.lock().unwrap_or_else(PoisonError::into_inner) = Some(cfg.clone());
        Ok(cfg)
    }

    /// Saves the configuration. With keyring integration on, credentials go to the keyring and
    /// the file keeps a placeholder.
    ///
    /// # Errors
    ///
    /// The unified config cannot be read or written.
    pub fn save(&self, mut cfg: ReachConfig) -> Result<(), ReachError> {
        if self.use_keyring {
            for (platform, pc) in &mut cfg.platforms {
                for (key, value) in pc.credentials.iter_mut() {
                    if let Err(e) = self.keyring.set(*platform, key, value) {
                        eprintln!("Warning: failed to save credential to keyring: {e}");
                    }
                    *value = KEYRING_PLACEHOLDER.to_owned();
                }
            }
        }
        let mut unified = self.unified.load()?.as_ref().clone();
        unified.reach = Some(to_unified(&cfg));
        self.unified.save(unified)?;
        *self.cache.lock().unwrap_or_else(PoisonError::into_inner) = None;
        Ok(())
    }

    /// The settings for `platform`, or the default when none are saved.
    ///
    /// # Errors
    ///
    /// The config cannot be loaded.
    pub fn platform_config(&self, platform: Platform) -> Result<PlatformConfig, ReachError> {
        Ok(self
            .load()?
            .platforms
            .get(&platform)
            .cloned()
            .unwrap_or_else(|| default_platform_config(platform)))
    }

    /// Saves the settings for `platform`.
    ///
    /// # Errors
    ///
    /// The config cannot be loaded or saved.
    pub fn set_platform_config(
        &self,
        platform: Platform,
        pc: PlatformConfig,
    ) -> Result<(), ReachError> {
        let mut cfg = self.load()?;
        cfg.platforms.insert(platform, pc);
        self.save(cfg)
    }

    /// # Errors
    ///
    /// The config cannot be loaded or saved.
    pub fn set_enabled(&self, platform: Platform, enabled: bool) -> Result<(), ReachError> {
        let mut pc = self.platform_config(platform)?;
        pc.enabled = enabled;
        self.set_platform_config(platform, pc)
    }

    /// Merges `creds` into the platform's credentials.
    ///
    /// # Errors
    ///
    /// The config cannot be loaded or saved.
    pub fn set_credentials(
        &self,
        platform: Platform,
        creds: &BTreeMap<String, String>,
    ) -> Result<(), ReachError> {
        let mut pc = self.platform_config(platform)?;
        for (k, v) in creds {
            pc.credentials.insert(k.clone(), v.clone());
        }
        self.set_platform_config(platform, pc)
    }

    /// One credential: the keyring first, then the file. A placeholder in the file is not a value.
    ///
    /// # Errors
    ///
    /// The credential is not stored anywhere.
    pub fn credential(&self, platform: Platform, key: &str) -> Result<String, ReachError> {
        if self.use_keyring {
            if let Ok(value) = self.keyring.get(platform, key) {
                return Ok(value);
            }
        }
        let cfg = self.load()?;
        match cfg
            .platforms
            .get(&platform)
            .and_then(|pc| pc.credentials.get(key))
        {
            Some(v) if v != KEYRING_PLACEHOLDER => Ok(v.clone()),
            _ => Err(ReachError::CredentialNotFound),
        }
    }

    /// Removes one credential from the keyring and the file.
    ///
    /// # Errors
    ///
    /// The config cannot be loaded or saved.
    pub fn delete_credential(&self, platform: Platform, key: &str) -> Result<(), ReachError> {
        if self.use_keyring {
            self.keyring.delete(platform, key);
        }
        let mut pc = self.platform_config(platform)?;
        pc.credentials.remove(key);
        self.set_platform_config(platform, pc)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_config_enables_only_the_zero_config_platforms() {
        let cfg = default_config();
        assert!(cfg.platforms[&Platform::Web].enabled);
        assert!(!cfg.platforms[&Platform::Twitter].enabled);
        assert_eq!(cfg.platforms.len(), Platform::ALL.len());
    }

    #[test]
    fn a_legacy_file_converts_to_the_unified_shape() {
        let legacy = br#"{"Platforms":{"web":{"Enabled":true,"PrimaryBackend":"jina-reader","FallbackBackends":["curl"]}},"cache_dir":"x","cache_size_mb":5,"default_cache_ttl":300000000000}"#;
        let unified = migrate_legacy(legacy).expect("convert");
        let reach = unified.reach.expect("reach section");
        assert_eq!(reach.cache_dir, "x");
        assert_eq!(reach.default_cache_ttl, "5m0s");
        assert_eq!(reach.platforms["web"].default_backend, "jina-reader");
    }

    #[test]
    fn the_legacy_and_unified_shapes_round_trip_the_backends() {
        let cfg = default_config();
        let back = from_unified(&to_unified(&cfg));
        assert_eq!(
            back.platforms[&Platform::Web].primary_backend,
            "jina-reader"
        );
        assert_eq!(
            back.platforms[&Platform::Rss].fallback_backends,
            vec!["feedparser".to_owned()]
        );
    }
}
