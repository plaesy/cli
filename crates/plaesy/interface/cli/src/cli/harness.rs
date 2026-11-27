//! `plaesy harness`: lists the 40 agent harnesses of the Orchestrator reference, and checks which of
//! them have an executable on `PATH`. Nothing here starts an agent; `check` only runs each found
//! executable once with `--version`.

use std::fmt::Write as _;

use plaesy_common::harness::{self, Harness, Readiness};

use super::CliResult;

/// List the agent harnesses, or check which are installed on `PATH` and at what version.
#[derive(Debug, clap::Args)]
pub struct Args {
    #[command(subcommand)]
    pub command: HarnessCommand,
}

#[derive(Debug, clap::Subcommand)]
pub enum HarnessCommand {
    /// List every harness with the surfaces it supports (TUI, chat, reviewer)
    Ls,
    /// Check whether harnesses are installed on PATH. Exits 1 when the given harness is not ready.
    Check {
        /// Harness id, for example `claude-code`. Omit to check all of them.
        id: Option<String>,
    },
}

/// Runs the `harness` subcommand.
///
/// # Errors
///
/// `check` names an id that is not in the registry, or printing fails.
pub fn run(args: &Args) -> CliResult {
    match &args.command {
        HarnessCommand::Ls => {
            print!("{}", render_list(harness::HARNESSES));
            Ok(())
        }
        HarnessCommand::Check { id: None } => {
            for h in harness::HARNESSES {
                println!("{}", render_check_line(h, &harness::check(h)));
            }
            Ok(())
        }
        HarnessCommand::Check { id: Some(id) } => {
            let h = harness::HARNESSES
                .iter()
                .find(|h| h.id == id.as_str())
                .ok_or_else(|| format!("unknown harness id: {id} (see `plaesy harness ls`)"))?;
            let readiness = harness::check(h);
            println!("{}", render_check_line(h, &readiness));
            if is_ready(&readiness) {
                Ok(())
            } else {
                Err(format!("harness {id} is not ready").into())
            }
        }
    }
}

/// True when the harness is installed at a version that meets its minimum.
#[must_use]
pub fn is_ready(readiness: &Readiness) -> bool {
    matches!(readiness, Readiness::Found { .. })
}

fn mark(on: bool) -> &'static str {
    if on {
        "yes"
    } else {
        "-"
    }
}

/// One line per registry entry: id, executable name, and the surfaces it supports.
#[must_use]
pub fn render_list(entries: &[Harness]) -> String {
    let mut out = String::new();
    let _ = writeln!(
        out,
        "{:<18} {:<14} {:<4} {:<5} {:<9} MIN",
        "ID", "BINARY", "TUI", "CHAT", "REVIEWER"
    );
    for h in entries {
        let _ = writeln!(
            out,
            "{:<18} {:<14} {:<4} {:<5} {:<9} {}",
            h.id,
            h.binary.unwrap_or("(unverified)"),
            mark(h.tui),
            mark(h.chat),
            mark(h.reviewer),
            h.min_version.unwrap_or("-"),
        );
    }
    out
}

/// One line for a checked harness, with its status and, when found, its path and version.
#[must_use]
pub fn render_check_line(h: &Harness, readiness: &Readiness) -> String {
    let status = match readiness {
        Readiness::Unverified => {
            "unverified: the executable name is not stated in the reference".to_owned()
        }
        Readiness::Missing => "missing: not on PATH".to_owned(),
        Readiness::Found {
            path,
            version: Some(v),
        } => format!("installed v{v} at {}", path.display()),
        Readiness::Found {
            path,
            version: None,
        } => format!("installed, version unknown, at {}", path.display()),
        Readiness::Outdated { path, version, min } => {
            format!("outdated: v{version} at {}, needs >= {min}", path.display())
        }
    };
    format!("{:<18} {status}", h.id)
}

#[cfg(test)]
#[path = "harness_test.rs"]
mod harness_test;
