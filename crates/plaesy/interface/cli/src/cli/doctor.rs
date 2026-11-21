//! `plaesy doctor`: health checks for the reach platforms and the delegate providers.

use std::path::Path;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

use chrono::{SecondsFormat, Utc};

use super::delegate::{delegate_status, load_delegate_config};
use super::CliResult;
use crate::reach::config::ConfigManager;
use crate::reach::duration;
use crate::reach::ratelimit::{default_rate_limits, PlatformRateLimiter};
use crate::reach::{CacheBackend, Platform, Registry, RegistryOptions, TieredCache};

/// Health checks for the reach platforms and the delegate providers. A failed platform check
/// exits with status 1.
#[derive(Debug, clap::Args)]
pub struct Args {
    /// Specific platform to check (web, github, youtube, etc.)
    pub platform: Option<String>,
    /// Show rate limit and cache stats
    #[arg(long)]
    pub verbose: bool,
}

pub fn run(args: &Args) -> CliResult {
    let config = ConfigManager::new().load()?;

    let cache: Option<Arc<dyn CacheBackend>> = if config.cache_dir.is_empty() {
        None
    } else {
        TieredCache::new(
            usize::try_from(config.cache_memory_size).unwrap_or(0),
            &config.cache_dir,
            config.cache_size_mb,
            config.default_cache_ttl,
        )
        .ok()
        .map(|c| Arc::new(c) as Arc<dyn CacheBackend>)
    };
    let rate_limiter = Arc::new(PlatformRateLimiter::new(default_rate_limits()));
    let registry = Registry::new(
        config.clone(),
        RegistryOptions {
            cache: cache.clone(),
            rate_limiter: Some(Arc::clone(&rate_limiter)),
            retry: None,
        },
    );

    let requested = match &args.platform {
        Some(text) => {
            Some(Platform::from_str(text).map_err(|_| format!("unknown platform {text:?}"))?)
        }
        None => None,
    };
    let registered = registry.router().platforms();
    let platforms: Vec<Platform> = match requested {
        Some(p) => vec![p],
        None => Platform::ALL
            .into_iter()
            .filter(|p| registered.contains(p))
            .collect(),
    };

    println!("=== plaesy doctor ===");
    println!();
    let mut all_healthy = true;

    println!("--- Reach Platforms ---");
    for platform in platforms {
        let enabled = config.platforms.get(&platform).is_some_and(|pc| pc.enabled);
        if !enabled && requested.is_none() {
            continue;
        }
        let health = match registry.health(platform) {
            Ok(h) => h,
            Err(e) => {
                println!("{platform}: ERROR - {e}");
                all_healthy = false;
                continue;
            }
        };

        let mut platform_healthy = false;
        println!("{platform}:");
        for h in &health {
            let status = if h.available {
                platform_healthy = true;
                "✓"
            } else {
                all_healthy = false;
                "✗"
            };
            let auth = if h.requires_auth {
                " (auth required)"
            } else {
                ""
            };
            let version = if h.version.is_empty() {
                String::new()
            } else {
                format!(" v{}", h.version)
            };
            let latency = duration::format(Duration::from_millis(
                u64::try_from(h.latency.as_millis()).unwrap_or(0),
            ));
            println!("  {status} {}{version}{auth} - {latency}", h.backend);
            if !h.error.is_empty() {
                println!("      Error: {}", h.error);
            }
        }

        if args.verbose {
            let limiter = rate_limiter.get(platform);
            let reset = Utc::now()
                + chrono::Duration::from_std(
                    limiter
                        .reset()
                        .saturating_duration_since(std::time::Instant::now()),
                )
                .unwrap_or_default();
            println!(
                "  Rate limit: {}/{} remaining, resets at {}",
                limiter.remaining(),
                limiter.limit(),
                reset.to_rfc3339_opts(SecondsFormat::Secs, true)
            );
            if let Some(c) = &cache {
                let cs = c.stats();
                println!(
                    "  Cache: {} entries, {:.1}% hit rate",
                    cs.entries,
                    cs.hit_rate * 100.0
                );
            }
        }

        if !platform_healthy && requested.is_none() {
            println!("  ⚠ No working backend for {platform}");
        }
        println!();
    }

    // Delegate providers are informational: a disabled provider is a choice, not a failure.
    println!("--- Delegate Providers ---");
    match load_delegate_config(Path::new(".")) {
        Ok(cfg) => {
            for p in &cfg.providers {
                println!("  {:<14} {:<10} {}", p.name, p.kind, delegate_status(p));
            }
        }
        Err(e) => println!("  could not read delegate config: {e}"),
    }
    println!();

    if all_healthy {
        println!("✓ All components healthy");
        Ok(())
    } else {
        println!("✗ Some components have issues");
        std::process::exit(1);
    }
}
