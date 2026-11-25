//! `plaesy telemetry`: summary of the local, opt-in telemetry log. Nothing is sent anywhere.
use super::{common::find_root, CliResult};
use clap::{Args as ClapArgs, Subcommand};

#[derive(Debug, ClapArgs)]
pub struct Args {
    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Show command runs, errors, and average durations from .plaesy/telemetry.jsonl
    Summary,
}

pub fn run(args: &Args) -> CliResult {
    let Some(command) = &args.command else {
        return super::print_group_help("telemetry");
    };
    match command {
        Command::Summary => show_summary(),
    }
}

fn show_summary() -> CliResult {
    let root = find_root()?;
    let enabled = crate::telemetry::is_enabled(&root)?;
    let summary = crate::telemetry::summarize(&root)?;

    println!(
        "Telemetry: {} (opt in with [telemetry] enabled = true in .plaesy/config.toml)",
        if enabled { "enabled" } else { "disabled" }
    );
    println!("Log: {}", crate::telemetry::log_path(&root).display());
    if summary.commands.is_empty() {
        println!("No telemetry events recorded.");
    } else {
        println!(
            "{:<16} {:>6} {:>7} {:>10}",
            "COMMAND", "RUNS", "ERRORS", "AVG_MS"
        );
        for (name, stats) in &summary.commands {
            let avg = stats.total_ms.checked_div(stats.runs).unwrap_or(0);
            println!(
                "{:<16} {:>6} {:>7} {:>10}",
                name, stats.runs, stats.errors, avg
            );
        }
        println!(
            "Total: {} runs, {} errors",
            summary.total_runs(),
            summary.total_errors()
        );
    }
    if summary.malformed_lines > 0 {
        println!("Skipped {} malformed log lines.", summary.malformed_lines);
    }
    println!("Telemetry stays local. Nothing is sent over the network.");
    Ok(())
}
