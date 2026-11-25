//! `plaesy delegate`: sends one bounded prompt to a configured provider or chain, and reports usage.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use chrono::Utc;
use serde::Serialize;

use super::CliResult;
use crate::config::delegate::{DelegateConfig, DelegateProviderConfig};
use crate::config::schema::UnifiedConfig;
use crate::delegate::providers::Getenv;
use crate::delegate::usage::{self, Record, Totals};
use crate::delegate::{self, Attempt, ChainOptions, Request, Response};

/// Send one bounded prompt to a provider in `delegate.providers`, or to a chain in
/// `delegate.chains`. A billable provider also needs `--confirm`.
#[derive(Debug, clap::Args)]
#[command(args_conflicts_with_subcommands = true)]
pub struct Args {
    #[command(subcommand)]
    pub command: Option<DelegateCommand>,
    /// Project directory whose .plaesy/settings.json is read
    #[arg(long, default_value = ".")]
    pub path: PathBuf,
    /// Provider name from delegate.providers
    #[arg(long, default_value = "")]
    pub provider: String,
    /// Chain name from delegate.chains
    #[arg(long, default_value = "")]
    pub chain: String,
    /// Prompt to send
    #[arg(long, default_value = "")]
    pub prompt: String,
    /// Max output tokens (0 = provider default; CLI providers ignore it)
    #[arg(long = "max-tokens", default_value_t = 0)]
    pub max_tokens: u32,
    /// Stop each call after this long (e.g. 120s)
    #[arg(long, default_value = "120s")]
    pub timeout: String,
    /// Allow a billable provider to run
    #[arg(long)]
    pub confirm: bool,
    /// Write the JSON result to this file instead of stdout
    #[arg(long)]
    pub out: Option<PathBuf>,
    /// List configured providers and whether each may run
    #[arg(long)]
    pub list: bool,
}

#[derive(Debug, clap::Subcommand)]
pub enum DelegateCommand {
    /// Show delegate usage totals per provider and per chain
    Report {
        /// Project directory whose usage log is read
        #[arg(long, default_value = ".")]
        path: PathBuf,
    },
}

/// The JSON printed for a successful call.
#[derive(Debug, Serialize)]
struct DelegateOutput {
    #[serde(flatten)]
    response: Response,
    attempts: Vec<Attempt>,
}

fn analysis_dir(repo: &Path) -> PathBuf {
    repo.join(".plaesy").join("analysis")
}

/// The project config file under `repo`: `.plaesy/config.toml`. Chat and delegate settings are read
/// from it. Callers take the path from here rather than joining it themselves.
#[must_use]
pub fn config_path(repo: &Path) -> PathBuf {
    repo.join(".plaesy").join("config.toml")
}

/// The older JSON file, `.plaesy/settings.json`. It is read only while `config.toml` does not exist.
#[must_use]
pub fn settings_path(repo: &Path) -> PathBuf {
    repo.join(".plaesy").join("settings.json")
}

/// Reads a file, returning `None` when it does not exist.
pub fn read_optional(path: &Path) -> Result<Option<String>, Box<dyn std::error::Error>> {
    match fs::read_to_string(path) {
        Ok(text) => Ok(Some(text)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(format!("reading {}: {e}", path.display()).into()),
    }
}

/// The delegate section under `repo`. `config.toml` wins; `settings.json` is read only when it is
/// absent. A missing file or section gives the defaults.
pub fn load_delegate_config(repo: &Path) -> Result<DelegateConfig, Box<dyn std::error::Error>> {
    let toml_path = config_path(repo);
    if let Some(text) = read_optional(&toml_path)? {
        let unified: UnifiedConfig = toml::from_str(&text)
            .map_err(|e| format!("parsing {}: {e}", toml_path.display()))?;
        return Ok(unified.delegate.unwrap_or_else(DelegateConfig::defaults));
    }
    let json_path = settings_path(repo);
    let Some(text) = read_optional(&json_path)? else {
        return Ok(DelegateConfig::defaults());
    };
    let unified: UnifiedConfig = serde_json::from_str(&text)
        .map_err(|e| format!("parsing {}: {e}", json_path.display()))?;
    Ok(unified.delegate.unwrap_or_else(DelegateConfig::defaults))
}

/// Whether a provider may run now. Shared with `plaesy doctor`, so both report the same state.
#[must_use]
pub fn delegate_status(p: &DelegateProviderConfig) -> &'static str {
    if !p.enabled {
        "disabled"
    } else if p.billable {
        "ready (needs --confirm)"
    } else {
        "ready"
    }
}

fn table(headers: &[&str], rows: &[Vec<String>]) -> String {
    let mut widths: Vec<usize> = headers.iter().map(|h| h.len()).collect();
    for row in rows {
        for (i, cell) in row.iter().enumerate() {
            widths[i] = widths[i].max(cell.len());
        }
    }
    let mut out = String::new();
    let line = |cells: &[String]| {
        let mut s = String::new();
        for (i, cell) in cells.iter().enumerate() {
            if i + 1 == cells.len() {
                s.push_str(cell);
            } else {
                s.push_str(&format!("{cell:<w$}  ", w = widths[i]));
            }
        }
        s.push('\n');
        s
    };
    let head: Vec<String> = headers.iter().map(|h| (*h).to_owned()).collect();
    out.push_str(&line(&head));
    for row in rows {
        out.push_str(&line(row));
    }
    out
}

fn print_list(cfg: &DelegateConfig) {
    let rows: Vec<Vec<String>> = cfg
        .providers
        .iter()
        .map(|p| {
            vec![
                p.name.clone(),
                p.kind.clone(),
                p.enabled.to_string(),
                p.billable.to_string(),
                delegate_status(p).to_owned(),
            ]
        })
        .collect();
    print!(
        "{}",
        table(&["NAME", "TYPE", "ENABLED", "BILLABLE", "STATUS"], &rows)
    );
}

/// The provider names a call tries: one name for `--provider`, or the chain's ordered list.
pub fn chain_names(
    cfg: &DelegateConfig,
    provider: &str,
    chain: &str,
) -> Result<Vec<String>, String> {
    if !provider.is_empty() {
        return Ok(vec![provider.to_owned()]);
    }
    match cfg.chains.get(chain) {
        Some(names) => Ok(names.clone()),
        None => {
            let mut known: Vec<&String> = cfg.chains.keys().collect();
            known.sort();
            let known: Vec<&str> = known.iter().map(|s| s.as_str()).collect();
            Err(format!(
                "unknown chain {chain:?} (configured: {})",
                known.join(", ")
            ))
        }
    }
}

fn print_totals(label: &str, empty: &str, totals: &BTreeMap<String, Totals>) {
    let rows: Vec<Vec<String>> = totals
        .iter()
        .map(|(k, t)| {
            vec![
                if k.is_empty() {
                    empty.to_owned()
                } else {
                    k.clone()
                },
                t.calls.to_string(),
                t.input_tokens.to_string(),
                t.output_tokens.to_string(),
                t.unknown_tokens.to_string(),
                format_micro_usd(t.cost_micro_usd),
                t.uncosted_calls.to_string(),
                t.wall_ms.to_string(),
            ]
        })
        .collect();
    print!(
        "{}",
        table(
            &[
                label,
                "CALLS",
                "INPUT_TOKENS",
                "OUTPUT_TOKENS",
                "NO_TOKEN_COUNT",
                "COST_USD",
                "NO_COST",
                "WALL_MS"
            ],
            &rows
        )
    );
}

pub fn run(args: &Args) -> CliResult {
    if let Some(DelegateCommand::Report { path }) = &args.command {
        return report(path);
    }
    let cfg = load_delegate_config(&args.path)?;
    if args.list {
        print_list(&cfg);
        return Ok(());
    }
    if args.prompt.is_empty() || args.provider.is_empty() == args.chain.is_empty() {
        return Err("--prompt is required, and exactly one of --provider or --chain (use --list to see providers)".into());
    }
    let names = chain_names(&cfg, &args.provider, &args.chain)?;
    let timeout = crate::reach::duration::parse(&args.timeout)
        .ok_or_else(|| format!("invalid --timeout {:?}", args.timeout))?;

    let usage_path = usage::usage_path(&analysis_dir(&args.path));
    let records = usage::read(&usage_path)?;

    let getenv: Getenv = Arc::new(|name: &str| std::env::var(name).ok());
    let build = |entry: &DelegateProviderConfig| delegate::build(entry, Arc::clone(&getenv));
    let check = |entry: &DelegateProviderConfig, req: &Request| {
        usage::check_budget(entry, req, &records, Utc::now())
    };
    let request = Request {
        prompt: args.prompt.clone(),
        max_output_tokens: args.max_tokens,
        timeout,
        dir: String::new(),
    };
    let opts = ChainOptions {
        chain: args.chain.clone(),
        confirm: args.confirm,
        max_retry_after: Duration::from_secs(60),
        build: &build,
        check: Some(&check),
        sleep: &delegate::sleep_for,
    };
    let (resp, attempts) =
        delegate::run_chain(&cfg, &names, &request, &opts).map_err(|e| e.to_string())?;

    let entry = usage::entry_named(&cfg, &resp.provider);
    usage::append(
        &usage_path,
        &usage::record_for(
            &entry,
            &args.chain,
            "",
            resp.input_tokens,
            resp.output_tokens,
            resp.wall_ms,
        ),
    )?;

    let text = serde_json::to_string_pretty(&DelegateOutput {
        response: resp,
        attempts,
    })?;
    match &args.out {
        Some(out) => fs::write(out, format!("{text}\n"))
            .map_err(|e| format!("writing {}: {e}", out.display()))?,
        None => println!("{text}"),
    }
    Ok(())
}

fn report(path: &Path) -> CliResult {
    let records: Vec<Record> = usage::read(&usage::usage_path(&analysis_dir(path)))?;
    if records.is_empty() {
        println!("no delegate calls recorded");
        return Ok(());
    }
    let (by_provider, by_chain) = usage::summarize(&records);
    let by_task = usage::summarize_by_task(&records);
    print_totals("PROVIDER", "(direct)", &by_provider);
    println!();
    print_totals("CHAIN", "(direct)", &by_chain);
    println!();
    print_totals("TASK", "(no task)", &by_task);
    Ok(())
}

/// Formats micro-USD as dollars with six decimals, without going through floating point.
fn format_micro_usd(micro: i64) -> String {
    let micro = micro.max(0);
    format!("{}.{:06}", micro / 1_000_000, micro % 1_000_000)
}
