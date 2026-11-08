//! `plaesy reach`: external data sources across platforms, with routing, caching, rate limits,
//! and fallback.

use std::collections::BTreeMap;
use std::io::{self, BufRead, Write};
use std::sync::Arc;
use std::time::Instant;

use chrono::SecondsFormat;
use serde_json::json;

use super::CliResult;
use crate::reach::config::{credential_keys, ConfigManager};
use crate::reach::duration;
use crate::reach::ratelimit::{default_rate_limits, PlatformRateLimiter};
use crate::reach::{CacheBackend, Platform, Registry, RegistryOptions, TieredCache};
use std::str::FromStr;

/// Access external data sources. With no arguments, prints the help.
#[derive(Debug, clap::Args)]
#[command(args_conflicts_with_subcommands = true)]
pub struct Args {
    #[command(subcommand)]
    pub command: Option<ReachCommand>,
    /// Platform to query (web, youtube, github, rss, twitter, reddit, bilibili, xiaohongshu,
    /// linkedin, exa, v2ex, xueqiu, xiaoyuzhou, bosszhipin, facebook, instagram)
    pub platform: Option<String>,
    /// Query for the platform (a URL, search terms, or a repository)
    pub query: Option<String>,
    /// Action for the platform (search, view, etc.)
    #[arg(long, default_value = "")]
    pub action: String,
    /// Max results for search actions
    #[arg(long, default_value_t = 10)]
    pub limit: usize,
    /// Query timeout (e.g. 30s, 1m)
    #[arg(long, default_value = "")]
    pub timeout: String,
    /// Cache TTL (e.g. 5m, 1h)
    #[arg(long = "cache-ttl", default_value = "")]
    pub cache_ttl: String,
    /// Disable caching for this query
    #[arg(long = "no-cache")]
    pub no_cache: bool,
    /// Verbose output
    #[arg(long)]
    pub verbose: bool,
    /// Output as JSON
    #[arg(long = "json")]
    pub json_output: bool,
}

#[derive(Debug, clap::Subcommand)]
pub enum ReachCommand {
    /// Configure credentials for a platform
    Configure { platform: String },
    /// List all available platforms
    List,
}

pub fn run(args: &Args) -> CliResult {
    match &args.command {
        Some(ReachCommand::Configure { platform }) => configure(platform),
        Some(ReachCommand::List) => list(),
        None => match (&args.platform, &args.query) {
            (None, _) => {
                use clap::CommandFactory;
                super::Cli::command()
                    .find_subcommand_mut("reach")
                    .ok_or("reach command missing")?
                    .print_help()?;
                println!();
                Ok(())
            }
            (Some(platform), query) => {
                query_platform(platform, query.as_deref().unwrap_or(""), args)
            }
        },
    }
}

fn parse_platform(text: &str) -> Result<Platform, String> {
    Platform::from_str(text).map_err(|_| {
        format!("unknown platform \"{text}\". Run 'plaesy reach list' for available platforms")
    })
}

fn build_registry(config: crate::reach::ReachConfig, verbose: bool) -> Registry {
    let cache: Option<Arc<dyn CacheBackend>> = if config.cache_dir.is_empty() {
        None
    } else {
        match TieredCache::new(
            usize::try_from(config.cache_memory_size).unwrap_or(0),
            &config.cache_dir,
            config.cache_size_mb,
            config.default_cache_ttl,
        ) {
            Ok(c) => Some(Arc::new(c)),
            Err(e) => {
                eprintln!("Warning: failed to create cache: {e}");
                None
            }
        }
    };
    let _ = verbose;
    let rate_limiter = Arc::new(PlatformRateLimiter::new(default_rate_limits()));
    Registry::new(
        config,
        RegistryOptions {
            cache,
            rate_limiter: Some(rate_limiter),
            retry: None,
        },
    )
}

fn query_platform(platform_text: &str, query: &str, args: &Args) -> CliResult {
    let platform = parse_platform(platform_text)?;
    let config = ConfigManager::new().load()?;
    let mut registry = build_registry(config, args.verbose);
    registry
        .load_config(&ConfigManager::new())
        .map_err(|e| format!("loading credentials: {e}"))?;

    let mut opts: BTreeMap<String, String> = BTreeMap::new();
    if !args.action.is_empty() {
        opts.insert("action".into(), args.action.clone());
    }
    if args.limit > 0 {
        opts.insert("limit".into(), args.limit.to_string());
    }
    if !args.timeout.is_empty() {
        opts.insert("timeout".into(), args.timeout.clone());
    }
    if !args.cache_ttl.is_empty() {
        opts.insert("cache_ttl".into(), args.cache_ttl.clone());
    }
    if args.no_cache {
        opts.insert("cache".into(), "false".into());
    }

    if args.verbose {
        eprintln!("[plaesy-reach] Querying {platform} via {query}...");
    }
    let start = Instant::now();
    let result = registry
        .query(platform, query, &opts)
        .map_err(|e| format!("query failed: {e}"))?;
    let elapsed = start.elapsed();

    if !result.success {
        return Err(format!("backend {} failed: {}", result.backend, result.error).into());
    }

    if args.json_output {
        let output = json!({
            "platform": platform.as_str(),
            "backend": result.backend,
            "success": result.success,
            "data": result.data,
            "latency": duration::format(result.latency),
            "timestamp": result.timestamp.to_rfc3339_opts(SecondsFormat::Secs, true),
        });
        println!("{}", serde_json::to_string_pretty(&output)?);
    } else {
        if args.verbose {
            eprintln!(
                "[plaesy-reach] Success via {} ({:.2}s)",
                result.backend,
                elapsed.as_secs_f64()
            );
        }
        println!("{}", result.data);
    }
    io::stdout().flush()?;
    Ok(())
}

fn configure(platform_text: &str) -> CliResult {
    let platform = parse_platform(platform_text)?;
    let keys = credential_keys(platform);
    if keys.is_empty() {
        println!("Platform {platform} does not require configuration (zero-config).");
        return Ok(());
    }

    println!("Configuring {platform}...");
    println!("Required credentials: {}", keys.join(", "));
    println!("Enter values (press Enter to skip):");

    let mut creds: BTreeMap<String, String> = BTreeMap::new();
    let mut input = io::stdin().lock();
    for key in keys {
        print!("  {key}: ");
        io::stdout().flush()?;
        let mut line = String::new();
        input.read_line(&mut line)?;
        let value = line.trim();
        if !value.is_empty() {
            creds.insert((*key).to_owned(), value.to_owned());
        }
    }

    let manager = ConfigManager::new();
    let mut config = manager.load()?;
    let pc = config
        .platforms
        .entry(platform)
        .or_insert_with(|| crate::reach::config::default_platform_config(platform));
    pc.enabled = true;
    for (k, v) in creds {
        pc.credentials.insert(k, v);
    }
    manager
        .save(config)
        .map_err(|e| format!("saving config: {e}"))?;

    println!("✓ Configuration saved for {platform}");
    Ok(())
}

fn list() -> CliResult {
    println!("Available platforms:");
    for p in Platform::ALL {
        let status = if Platform::ZERO_CONFIG.contains(&p) {
            " (zero-config)"
        } else {
            " (requires auth)"
        };
        println!("  {p}{status}");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_platforms_are_rejected_with_the_list_hint() {
        let err = parse_platform("nope").expect_err("unknown");
        assert!(err.contains("plaesy reach list"));
    }
}
