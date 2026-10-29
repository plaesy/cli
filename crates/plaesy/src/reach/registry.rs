//! The registry: builds the router with the built-in adapters, and applies configuration.

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;

use super::adapters;
use super::cache::CacheBackend;
use super::config::ConfigManager;
use super::ratelimit::PlatformRateLimiter;
use super::router::Router;
use super::types::{BackendHealth, BackendResult, Options, Platform, ReachConfig, RetryConfig};
use super::ReachError;

/// Optional parts a registry is built with.
#[derive(Default)]
pub struct RegistryOptions {
    pub cache: Option<Arc<dyn CacheBackend>>,
    pub rate_limiter: Option<Arc<PlatformRateLimiter>>,
    pub retry: Option<RetryConfig>,
}

/// Counts reported by [`Registry::stats`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RegistryStats {
    pub platforms: usize,
    pub cache: Option<super::cache::CacheStats>,
    pub initialized: bool,
}

/// Holds the router and the configuration it was built from.
pub struct Registry {
    router: Router,
    config: ReachConfig,
    cache: Option<Arc<dyn CacheBackend>>,
    rate_limiter: Option<Arc<PlatformRateLimiter>>,
    initialized: bool,
}

impl Registry {
    /// A registry with every built-in adapter registered.
    #[must_use]
    pub fn new(config: ReachConfig, opts: RegistryOptions) -> Self {
        let router = build_router(&config, &opts);
        let registry = Self {
            router,
            config,
            cache: opts.cache,
            rate_limiter: opts.rate_limiter,
            initialized: true,
        };
        for adapter in adapters::builtin() {
            registry.router.register(adapter);
        }
        registry
    }

    #[must_use]
    pub fn router(&self) -> &Router {
        &self.router
    }

    /// Runs a query on `platform`.
    ///
    /// # Errors
    ///
    /// See [`Router::route`].
    pub fn query(
        &self,
        platform: Platform,
        query: &str,
        opts: &Options,
    ) -> Result<BackendResult, ReachError> {
        self.router.route(platform, query, opts)
    }

    /// # Errors
    ///
    /// No adapter is registered for `platform`.
    pub fn health(&self, platform: Platform) -> Result<Vec<BackendHealth>, ReachError> {
        self.router.health(platform)
    }

    /// Health of every registered platform. A platform whose check fails maps to an error entry.
    #[must_use]
    pub fn health_all(&self) -> BTreeMap<Platform, Vec<BackendHealth>> {
        self.router
            .platforms()
            .into_iter()
            .map(|p| {
                let health = self.router.health(p).unwrap_or_else(|e| {
                    let mut h = BackendHealth::new("", p);
                    h.error = e.to_string();
                    vec![h]
                });
                (p, health)
            })
            .collect()
    }

    /// # Errors
    ///
    /// No adapter is registered for `platform`, or the adapter rejects the credentials.
    pub fn configure_platform(
        &self,
        platform: Platform,
        creds: &BTreeMap<String, String>,
    ) -> Result<(), ReachError> {
        self.router.configure(platform, creds)
    }

    /// Applies the saved settings: credentials for enabled platforms, and their rate limits.
    ///
    /// # Errors
    ///
    /// The config cannot be loaded, or an adapter rejects its credentials.
    pub fn load_config(&mut self, manager: &ConfigManager) -> Result<(), ReachError> {
        let saved = manager.load()?;
        for (platform, pc) in &saved.platforms {
            if !pc.enabled {
                continue;
            }
            if !pc.credentials.is_empty() {
                self.configure_platform(*platform, &pc.credentials)
                    .map_err(|e| ReachError::Configure {
                        platform: *platform,
                        source: Box::new(e),
                    })?;
            }
            if let (Some(limit), Some(limiter)) = (&pc.rate_limit, &self.rate_limiter) {
                limiter.set_config(*platform, limit.clone());
            }
        }
        self.router.set_config(Some(saved.clone()));
        self.config = saved;
        Ok(())
    }

    /// Re-reads the config and re-applies its rate limits. Adapters keep their credentials.
    ///
    /// # Errors
    ///
    /// The config cannot be loaded.
    pub fn reload_config(&mut self, manager: &ConfigManager) -> Result<(), ReachError> {
        let saved = manager.load()?;
        if let Some(limiter) = &self.rate_limiter {
            for (platform, pc) in &saved.platforms {
                if let Some(limit) = &pc.rate_limit {
                    limiter.set_config(*platform, limit.clone());
                }
            }
        }
        self.router.set_config(Some(saved.clone()));
        self.config = saved;
        Ok(())
    }

    #[must_use]
    pub fn config(&self) -> &ReachConfig {
        &self.config
    }

    #[must_use]
    pub fn cache(&self) -> Option<&Arc<dyn CacheBackend>> {
        self.cache.as_ref()
    }

    #[must_use]
    pub fn rate_limiter(&self) -> Option<&Arc<PlatformRateLimiter>> {
        self.rate_limiter.as_ref()
    }

    #[must_use]
    pub fn stats(&self) -> RegistryStats {
        RegistryStats {
            platforms: self.router.platforms().len(),
            cache: self.cache.as_ref().map(|c| c.stats()),
            initialized: self.initialized,
        }
    }
}

fn build_router(config: &ReachConfig, opts: &RegistryOptions) -> Router {
    let mut router = Router::new(Some(config.clone()));
    if let Some(cache) = &opts.cache {
        router = router.with_cache(Arc::clone(cache));
    }
    if let Some(limiter) = &opts.rate_limiter {
        router = router.with_rate_limiter(Arc::clone(limiter));
    }
    if let Some(retry) = opts
        .retry
        .clone()
        .or_else(|| config.global_retry_config.clone())
    {
        router = router.with_retry(retry);
    }
    if !config.default_timeout.is_zero() {
        router = router.with_timeout(config.default_timeout);
    } else {
        router = router.with_timeout(Duration::from_secs(30));
    }
    router
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reach::config::default_config;

    #[test]
    fn every_platform_is_registered() {
        let registry = Registry::new(default_config(), RegistryOptions::default());
        assert_eq!(registry.stats().platforms, Platform::ALL.len());
        assert!(registry.stats().initialized);
    }

    #[test]
    fn a_platform_without_credentials_reports_its_health() {
        let registry = Registry::new(default_config(), RegistryOptions::default());
        let health = registry.health(Platform::Rss).expect("rss is registered");
        assert!(health.iter().any(|h| h.backend == "builtin" && h.available));
    }
}
