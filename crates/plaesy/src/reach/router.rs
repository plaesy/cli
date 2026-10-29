//! Routes a query to the first backend that succeeds, with retries, caching, and rate limits.

use std::collections::BTreeMap;
use std::sync::{Arc, PoisonError, RwLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use super::adapter::PlatformAdapter;
use super::cache::{cache_key, CacheBackend};
use super::duration;
use super::exec::Deadline;
use super::ratelimit::PlatformRateLimiter;
use super::types::{BackendHealth, BackendResult, Options, Platform, ReachConfig, RetryConfig};
use super::ReachError;

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);
const BUILT_IN_CACHE_TTL: Duration = Duration::from_secs(5 * 60);

/// Whether `error_text` names one of the retryable patterns, ignoring case.
#[must_use]
pub fn is_retryable(error_text: &str, retry: &RetryConfig) -> bool {
    let lower = error_text.to_lowercase();
    retry
        .retryable_errors
        .iter()
        .any(|pattern| lower.contains(&pattern.to_lowercase()))
}

/// Exponential backoff with ±25% jitter, capped at the maximum delay.
#[must_use]
pub fn backoff(attempt: u32, retry: &RetryConfig) -> Duration {
    let base = retry.base_delay.as_secs_f64()
        * retry
            .multiplier
            .powi(i32::try_from(attempt).unwrap_or(i32::MAX));
    let delay = base.min(retry.max_delay.as_secs_f64());
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.subsec_nanos());
    let unit = f64::from(nanos % 1000) / 1000.0;
    let jitter = delay * 0.25 * (2.0 * unit - 1.0);
    Duration::from_secs_f64((delay + jitter).max(0.0))
}

/// Mutations are never cached. Setting `cache=false` disables caching for one call.
fn is_cacheable(opts: &Options) -> bool {
    if opt_is(opts, "cache", "false") {
        return false;
    }
    !matches!(
        opts.get("action").map(String::as_str),
        Some("create" | "update" | "delete" | "post" | "comment" | "vote")
    )
}

fn opt_is(opts: &Options, key: &str, value: &str) -> bool {
    opts.get(key).is_some_and(|v| v == value)
}

/// The adapters, the configuration that gates their backends, and the shared machinery.
pub struct Router {
    adapters: RwLock<BTreeMap<Platform, Box<dyn PlatformAdapter>>>,
    config: Option<ReachConfig>,
    cache: Option<Arc<dyn CacheBackend>>,
    rate_limiter: Option<Arc<PlatformRateLimiter>>,
    retry: RetryConfig,
    default_timeout: Duration,
}

impl Router {
    #[must_use]
    pub fn new(config: Option<ReachConfig>) -> Self {
        Self {
            adapters: RwLock::new(BTreeMap::new()),
            config,
            cache: None,
            rate_limiter: None,
            retry: RetryConfig::default(),
            default_timeout: DEFAULT_TIMEOUT,
        }
    }

    #[must_use]
    pub fn with_cache(mut self, cache: Arc<dyn CacheBackend>) -> Self {
        self.cache = Some(cache);
        self
    }

    #[must_use]
    pub fn with_rate_limiter(mut self, limiter: Arc<PlatformRateLimiter>) -> Self {
        self.rate_limiter = Some(limiter);
        self
    }

    #[must_use]
    pub fn with_retry(mut self, retry: RetryConfig) -> Self {
        self.retry = retry;
        self
    }

    #[must_use]
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.default_timeout = timeout;
        self
    }

    pub fn set_config(&mut self, config: Option<ReachConfig>) {
        self.config = config;
    }

    /// Adds or replaces the adapter for its platform.
    pub fn register(&self, adapter: Box<dyn PlatformAdapter>) {
        self.adapters
            .write()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(adapter.name(), adapter);
    }

    /// Applies credentials to the adapter for `platform`.
    ///
    /// # Errors
    ///
    /// No adapter is registered for `platform`, or the adapter rejects the credentials.
    pub fn configure(
        &self,
        platform: Platform,
        creds: &BTreeMap<String, String>,
    ) -> Result<(), ReachError> {
        let mut adapters = self
            .adapters
            .write()
            .unwrap_or_else(PoisonError::into_inner);
        let adapter = adapters
            .get_mut(&platform)
            .ok_or(ReachError::NoAdapter(platform))?;
        adapter.configure(creds)
    }

    #[must_use]
    pub fn platforms(&self) -> Vec<Platform> {
        self.adapters
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .keys()
            .copied()
            .collect()
    }

    /// Runs `query` on `platform`, trying each enabled backend in order.
    ///
    /// # Errors
    ///
    /// No adapter or backend exists, the rate limit wait is cancelled, or every backend fails.
    pub fn route(
        &self,
        platform: Platform,
        query: &str,
        opts: &Options,
    ) -> Result<BackendResult, ReachError> {
        let adapters = self.adapters.read().unwrap_or_else(PoisonError::into_inner);
        let adapter = adapters
            .get(&platform)
            .ok_or(ReachError::NoAdapter(platform))?;
        let backends = adapter.backends();
        if backends.is_empty() {
            return Err(ReachError::NoBackends(platform));
        }

        let timeout = opts
            .get("timeout")
            .and_then(|t| duration::parse(t))
            .unwrap_or(self.default_timeout);
        let deadline = Deadline::after(timeout);

        if let Some(limiter) = &self.rate_limiter {
            limiter.get(platform).wait(deadline)?;
        }

        let key = match &self.cache {
            Some(_) if is_cacheable(opts) => Some(cache_key(platform, "", query, opts)),
            _ => None,
        };
        if let (Some(cache), Some(key)) = (&self.cache, &key) {
            if let Some(hit) = cache.get(key) {
                return Ok(hit);
            }
        }

        let mut last_error = String::new();
        for backend in backends {
            if !self.backend_enabled(platform, backend) {
                continue;
            }
            match self.execute_with_retry(adapter.as_ref(), backend, query, opts, deadline) {
                Ok(result) => {
                    if let (Some(cache), Some(key)) = (&self.cache, &key) {
                        let _ = cache.set(key, &result, self.cache_ttl(opts));
                    }
                    return Ok(result);
                }
                Err(e) => last_error = e.to_string(),
            }
        }
        Err(ReachError::AllBackendsFailed {
            platform,
            last: last_error,
        })
    }

    /// Whether `backend` is the primary or a fallback in the platform's configuration. A platform
    /// with no configuration allows every backend.
    fn backend_enabled(&self, platform: Platform, backend: &str) -> bool {
        let Some(pc) = self
            .config
            .as_ref()
            .and_then(|c| c.platforms.get(&platform))
        else {
            return true;
        };
        pc.primary_backend == backend || pc.fallback_backends.iter().any(|fb| fb == backend)
    }

    /// Runs one backend, retrying retryable failures with backoff.
    fn execute_with_retry(
        &self,
        adapter: &dyn PlatformAdapter,
        backend: &str,
        query: &str,
        opts: &Options,
        deadline: Deadline,
    ) -> Result<BackendResult, ReachError> {
        let mut last_error: Option<ReachError> = None;
        for attempt in 0..=self.retry.max_retries {
            let outcome = adapter.execute(backend, query, opts, deadline);
            if let Ok(result) = &outcome {
                if result.success {
                    return outcome;
                }
            }
            let (error_retryable, result_error) = match &outcome {
                Err(e) => (is_retryable(&e.to_string(), &self.retry), String::new()),
                Ok(result) => (false, result.error.clone()),
            };
            if let Err(e) = outcome {
                last_error = Some(e);
            }
            if attempt == self.retry.max_retries {
                break;
            }
            if !error_retryable
                && !result_error.is_empty()
                && !is_retryable(&result_error, &self.retry)
            {
                break;
            }
            let Some(left) = deadline.remaining() else {
                return Err(ReachError::DeadlineExceeded);
            };
            std::thread::sleep(backoff(attempt, &self.retry).min(left));
        }
        Err(last_error.unwrap_or(ReachError::RetriesExhausted))
    }

    /// The cache lifetime: the option `cache_ttl`, then the configured default, then five minutes.
    fn cache_ttl(&self, opts: &Options) -> Duration {
        if let Some(d) = opts.get("cache_ttl").and_then(|t| duration::parse(t)) {
            return d;
        }
        match &self.config {
            Some(c) if !c.default_cache_ttl.is_zero() => c.default_cache_ttl,
            _ => BUILT_IN_CACHE_TTL,
        }
    }

    /// Health of every backend of `platform`. A backend whose check errors is reported unavailable.
    ///
    /// # Errors
    ///
    /// No adapter is registered for `platform`.
    pub fn health(&self, platform: Platform) -> Result<Vec<BackendHealth>, ReachError> {
        let adapters = self.adapters.read().unwrap_or_else(PoisonError::into_inner);
        let adapter = adapters
            .get(&platform)
            .ok_or(ReachError::NoAdapter(platform))?;
        Ok(adapter
            .backends()
            .iter()
            .map(|backend| match adapter.health(backend) {
                Ok(h) => h,
                Err(e) => {
                    let mut h = BackendHealth::new(backend, platform);
                    h.error = e.to_string();
                    h
                }
            })
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retryable_matching_ignores_case() {
        let retry = RetryConfig::default();
        assert!(is_retryable("Connection Refused by host", &retry));
        assert!(!is_retryable("invalid argument", &retry));
    }

    #[test]
    fn backoff_stays_under_the_maximum_with_jitter() {
        let retry = RetryConfig::default();
        let delay = backoff(20, &retry);
        assert!(delay <= retry.max_delay.mul_f64(1.25));
    }

    #[test]
    fn mutations_are_not_cacheable() {
        let mut opts = Options::new();
        opts.insert("action".into(), "post".into());
        assert!(!is_cacheable(&opts));
        assert!(is_cacheable(&Options::new()));
    }
}
