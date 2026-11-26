//! CLI for the audit trail: list the newest recorded authorization decisions.

use clap::{Parser, Subcommand};
use plaesy_adapter_runstore::RunLog;
use plaesy_domain_workflow::runstore::audit::{AuditQuery, AuditTrail};
use std::path::PathBuf;

use super::CliResult;

#[derive(Debug, Parser)]
#[command(arg_required_else_help = true)]
pub struct Args {
    #[command(subcommand)]
    command: Action,
}

#[derive(Debug, Subcommand)]
enum Action {
    /// Print the newest audit entries as JSON lines, one per entry
    List {
        /// Audit log file. Defaults to `.plaesy/audit.log` in the project root. Protect it as
        /// private project data.
        #[arg(long)]
        log: Option<PathBuf>,
        /// Maximum number of entries to print.
        #[arg(long, default_value_t = 20)]
        limit: usize,
        /// Only entries with this action label, for example `submit` or `tool:shell`.
        #[arg(long)]
        action: Option<String>,
        /// Only entries at or after this RFC 3339 instant, for example 2026-11-01T00:00:00Z.
        #[arg(long)]
        since: Option<String>,
        /// Only entries at or before this RFC 3339 instant.
        #[arg(long)]
        until: Option<String>,
    },
}

/// Runs the audit command.
///
/// # Errors
///
/// The time filter is not RFC 3339, or the audit log cannot be read or is corrupt.
pub fn run(args: &Args) -> CliResult {
    match &args.command {
        Action::List {
            log,
            limit,
            action,
            since,
            until,
        } => {
            let query = AuditQuery {
                action: action.clone(),
                since: since.as_deref().map(parse_instant).transpose()?,
                until: until.as_deref().map(parse_instant).transpose()?,
                limit: Some(*limit),
            };
            let log = match log {
                Some(path) => path.clone(),
                None => super::common::find_root()?
                    .join(".plaesy")
                    .join("audit.log"),
            };
            let listing = AuditTrail::new(RunLog::open(log)).list(&query)?;
            if listing.torn_tail {
                eprintln!("warning: audit log ends in an incomplete line; it was ignored");
            }
            for entry in &listing.entries {
                let line = serde_json::json!({
                    "at": entry.at,
                    "principal": entry.principal,
                    "project": entry.project,
                    "action": entry.action,
                    "allowed": entry.allowed,
                    "reason": (!entry.reason.is_empty()).then_some(&entry.reason),
                });
                println!("{line}");
            }
            Ok(())
        }
    }
}

fn parse_instant(text: &str) -> Result<u64, Box<dyn std::error::Error>> {
    let seconds = chrono::DateTime::parse_from_rfc3339(text)?.timestamp();
    u64::try_from(seconds).map_err(|_| format!("time before 1970 is not supported: {text}").into())
}
