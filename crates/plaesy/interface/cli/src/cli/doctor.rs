//! `plaesy doctor`: health checks for the local install and memory vault, the reach platforms,
//! and the delegate providers.
//!
//! The local checks (binary version, vault, orphaned daemon lock, free disk space) work offline.
//! Each check is a pure `check_*` function over observed state, so tests need no filesystem; the
//! `probe_*` functions gather that state.

use std::path::{Path, PathBuf};
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

/// Daemon ownership lock directory, created by `WriteLock::acquire_named` in the memory engine.
const DAEMON_LOCK_NAME: &str = ".plaesy-memory-daemon.lock";
/// The daemon's heartbeat touches its lock about once per second. A lock untouched for longer
/// than this has no live owner.
const ORPHAN_LOCK_AGE: Duration = Duration::from_secs(15);
/// Minimum free space on the vault's volume before the disk check fails.
const MIN_FREE_BYTES: u64 = 1024 * 1024 * 1024;
const MIB: u64 = 1024 * 1024;

/// Health checks for the local install and memory vault, the reach platforms, and the delegate
/// providers. A failed check exits with status 1.
#[derive(Debug, clap::Args)]
pub struct Args {
    /// Specific platform to check (web, github, youtube, etc.)
    pub platform: Option<String>,
    /// Show rate limit and cache stats
    #[arg(long)]
    pub verbose: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Status {
    Pass,
    Fail,
    Skip,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Check {
    pub(crate) label: &'static str,
    pub(crate) status: Status,
    pub(crate) detail: String,
}

impl Check {
    fn new(label: &'static str, status: Status, detail: impl Into<String>) -> Self {
        Self {
            label,
            status,
            detail: detail.into(),
        }
    }
}

/// Observed state of the memory vault folder.
#[derive(Debug)]
pub(crate) enum VaultState {
    Unconfigured,
    ConfigError(String),
    Missing,
    NotDirectory,
    Unreadable(String),
    Readable,
}

/// Observed state of the daemon lock inside the vault.
#[derive(Debug)]
pub(crate) enum LockState {
    Absent,
    NotDirectory,
    Unreadable(String),
    /// `age` is `None` when the platform cannot report a modification time.
    Directory {
        age: Option<Duration>,
    },
}

/// Binary version. Only checks that the version is a well-formed `MAJOR.MINOR.PATCH`; it does not
/// compare against the installed binary, because the installer reports no installed version.
pub(crate) fn check_version(version: &str) -> Check {
    const LABEL: &str = "binary version";
    let core = version.split(['-', '+']).next().unwrap_or_default();
    let parts: Vec<&str> = core.split('.').collect();
    let well_formed = parts.len() == 3
        && parts
            .iter()
            .all(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit()));
    if well_formed {
        Check::new(LABEL, Status::Pass, format!("v{version}"))
    } else {
        Check::new(
            LABEL,
            Status::Fail,
            format!("not a MAJOR.MINOR.PATCH version: {version:?}"),
        )
    }
}

pub(crate) fn check_vault(path: &Path, state: &VaultState) -> Check {
    const LABEL: &str = "memory vault";
    let shown = path.display();
    match state {
        VaultState::Unconfigured => Check::new(
            LABEL,
            Status::Fail,
            "no vault folder is configured (run `plaesy memory onboard`)",
        ),
        VaultState::ConfigError(e) => Check::new(
            LABEL,
            Status::Fail,
            format!("memory configuration could not be loaded: {e}"),
        ),
        VaultState::Missing => Check::new(LABEL, Status::Fail, format!("{shown} does not exist")),
        VaultState::NotDirectory => {
            Check::new(LABEL, Status::Fail, format!("{shown} is not a directory"))
        }
        VaultState::Unreadable(e) => {
            Check::new(LABEL, Status::Fail, format!("{shown} cannot be read: {e}"))
        }
        VaultState::Readable => Check::new(
            LABEL,
            Status::Pass,
            format!("{shown} exists and is readable"),
        ),
    }
}

pub(crate) fn check_daemon_lock(state: &LockState) -> Check {
    const LABEL: &str = "daemon lock";
    match state {
        LockState::Absent => Check::new(LABEL, Status::Pass, "none present"),
        LockState::NotDirectory => Check::new(
            LABEL,
            Status::Fail,
            format!("{DAEMON_LOCK_NAME} exists but is not a directory"),
        ),
        LockState::Unreadable(e) => Check::new(
            LABEL,
            Status::Fail,
            format!("{DAEMON_LOCK_NAME} cannot be inspected: {e}"),
        ),
        LockState::Directory { age: None } => Check::new(
            LABEL,
            Status::Fail,
            format!("modification time of {DAEMON_LOCK_NAME} is unavailable"),
        ),
        LockState::Directory { age: Some(age) } if *age > ORPHAN_LOCK_AGE => Check::new(
            LABEL,
            Status::Fail,
            format!(
                "orphaned: not refreshed for {}s (limit {}s)",
                age.as_secs(),
                ORPHAN_LOCK_AGE.as_secs()
            ),
        ),
        LockState::Directory { age: Some(age) } => Check::new(
            LABEL,
            Status::Pass,
            format!("held by a live daemon, refreshed {}s ago", age.as_secs()),
        ),
    }
}

/// Disk check. `probe` is the result of querying free space on the vault's volume.
pub(crate) fn check_disk_space(probe: Result<u64, String>, minimum: u64) -> Check {
    const LABEL: &str = "free disk space";
    match probe {
        Err(e) => Check::new(LABEL, Status::Fail, format!("cannot be read: {e}")),
        Ok(free) if free < minimum => Check::new(
            LABEL,
            Status::Fail,
            format!(
                "{} MiB free, need at least {} MiB",
                free / MIB,
                minimum / MIB
            ),
        ),
        Ok(free) => Check::new(LABEL, Status::Pass, format!("{} MiB free", free / MIB)),
    }
}

/// Resolves the vault folder from the effective memory configuration (no network, no writes).
fn load_vault_path() -> Result<PathBuf, String> {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(|e| e.to_string())?;
    let config = runtime
        .block_on(crate::memory::config::MemoryConfig::load())
        .map_err(|e| e.to_string())?;
    Ok(config.watch_dir)
}

fn probe_vault(path: &Path) -> VaultState {
    if path.as_os_str().is_empty() {
        return VaultState::Unconfigured;
    }
    match std::fs::metadata(path) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => VaultState::Missing,
        Err(e) => VaultState::Unreadable(e.to_string()),
        Ok(meta) if !meta.is_dir() => VaultState::NotDirectory,
        Ok(_) => match std::fs::read_dir(path) {
            Ok(_) => VaultState::Readable,
            Err(e) => VaultState::Unreadable(e.to_string()),
        },
    }
}

fn probe_daemon_lock(vault: &Path) -> LockState {
    let lock = vault.join(DAEMON_LOCK_NAME);
    match std::fs::symlink_metadata(&lock) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => LockState::Absent,
        Err(e) => LockState::Unreadable(e.to_string()),
        Ok(meta) if !meta.is_dir() => LockState::NotDirectory,
        Ok(meta) => LockState::Directory {
            age: meta.modified().ok().and_then(|t| t.elapsed().ok()),
        },
    }
}

/// Runs the local checks in a fixed order. The lock check is skipped when the vault is not
/// readable, since the vault check already reports that failure.
fn local_checks() -> Vec<Check> {
    let mut checks = vec![check_version(crate::version())];

    let (vault_dir, vault_state) = match load_vault_path() {
        Ok(path) => {
            let state = probe_vault(&path);
            (path, state)
        }
        Err(e) => (PathBuf::new(), VaultState::ConfigError(e)),
    };
    let vault_ok = matches!(vault_state, VaultState::Readable);
    checks.push(check_vault(&vault_dir, &vault_state));

    if vault_ok {
        checks.push(check_daemon_lock(&probe_daemon_lock(&vault_dir)));
    } else {
        checks.push(Check::new(
            "daemon lock",
            Status::Skip,
            "skipped: vault is not readable",
        ));
    }

    // Measure the vault's volume when it is usable; otherwise the working directory's volume.
    let disk_dir = if vault_ok {
        vault_dir
    } else {
        PathBuf::from(".")
    };
    let free = crate::common::disk::available_bytes(&disk_dir).map_err(|e| e.to_string());
    checks.push(check_disk_space(free, MIN_FREE_BYTES));

    checks
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

    if requested.is_none() {
        println!("--- Local ---");
        for check in local_checks() {
            let mark = match check.status {
                Status::Pass => "✓",
                Status::Fail => {
                    all_healthy = false;
                    "✗"
                }
                Status::Skip => "-",
            };
            println!("  {mark} {}: {}", check.label, check.detail);
        }
        println!();
    }

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

#[cfg(test)]
#[path = "doctor_test.rs"]
mod doctor_test;
