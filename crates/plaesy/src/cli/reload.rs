//! `plaesy reload`: refreshes the generated parts of `.plaesy/` from the current sources.

use std::path::PathBuf;

use super::common::logger;
use super::CliResult;
use crate::scaffold::reload::{self, Options};

/// Reload overwrites generated files and leaves memory, context, state, tasks, analysis, and
/// memory folders alone. A file with no source counterpart is reported as stale, and kept unless
/// `--prune-apply` is given.
#[derive(Debug, clap::Args)]
pub struct Args {
    /// Project to refresh (default: current directory)
    pub directory: Option<PathBuf>,
    /// Also refresh this AI platform's core file, prompts and agent directory (e.g. claude, cursor_ai)
    #[arg(long, default_value = "")]
    pub ai: String,
    /// Override the Plaesy repo root to copy sources from
    #[arg(long = "plaesy-home")]
    pub plaesy_home: Option<PathBuf>,
    /// Report what would change without writing anything
    #[arg(long)]
    pub dry_run: bool,
    /// List generated files that no longer have a source; deletes nothing
    #[arg(long)]
    pub prune: bool,
    /// With --prune, actually delete the stale files listed
    #[arg(long = "prune-apply")]
    pub prune_apply: bool,
}

pub fn run(args: &Args) -> CliResult {
    let log = logger();
    let opts = Options {
        target_dir: args.directory.clone().unwrap_or_else(|| PathBuf::from(".")),
        ai_platform: args.ai.clone(),
        plaesy_home: args.plaesy_home.clone(),
        use_embedded: true,
        dry_run: args.dry_run,
        prune: args.prune,
        prune_apply: args.prune_apply,
    };
    let report = reload::reload(&opts, &log)?;

    log.info("");
    if !report.protected.is_empty() {
        log.info("Preserved (your data, not regenerated):");
        for p in &report.protected {
            log.info(&format!("  · {p}"));
        }
    }
    if !report.failed.is_empty() {
        log.error("Failed:");
        for p in &report.failed {
            log.error(&format!("  ✗ {p}"));
        }
    }
    if !report.stale.is_empty() {
        if report.pruned.is_empty() {
            log.info("Stale (generated, no counterpart in the sources — NOT removed):");
            for p in &report.stale {
                log.info(&format!("  · {p}"));
            }
        } else {
            log.info("Pruned (removed, no counterpart in the sources):");
            for p in &report.pruned {
                log.info(&format!("  · {p}"));
            }
        }
    }
    log.info(&format!("Summary: {}", report.summary()));

    if !report.failed.is_empty() {
        // A partial refresh that reports success is how drift becomes permanent.
        return Err(format!(
            "reload completed with {} failure(s); see the list above",
            report.failed.len()
        )
        .into());
    }
    if report.dry_run {
        if report.changed() {
            log.success("Dry run complete — re-run without --dry-run to apply.");
        } else {
            log.success("Dry run complete — everything is already up to date.");
        }
        return Ok(());
    }
    if report.changed() {
        log.success("Reload complete.");
    } else {
        log.success("Nothing to do — .plaesy/ already matches the sources.");
    }
    Ok(())
}
