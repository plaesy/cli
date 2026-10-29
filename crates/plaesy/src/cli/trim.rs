//! `plaesy trim`: token and context compression for command output, memory, and instruction files.

use std::io::{self, Write};
use std::path::{Path, PathBuf};

use super::CliResult;
use crate::trimmer::{directory, layer1, llm, rules, stats};

/// Layer 1 runs a command and compresses its output. Layer 2 rewrites prose, either with a
/// heuristic (`compress`) or through the calling assistant (`llm-queue` / `apply-llm`).
#[derive(Debug, clap::Args)]
pub struct Args {
    #[command(subcommand)]
    pub command: Option<TrimCommand>,
}

#[derive(Debug, clap::Subcommand)]
pub enum TrimCommand {
    /// Layer 1: run a command and compress its output
    Run {
        /// The command to run, after `--`
        #[arg(trailing_var_arg = true, allow_hyphen_values = true, required = true)]
        command: Vec<String>,
    },
    /// Layer 2: fast heuristic prose compression
    Compress {
        /// File or directory to compress
        #[arg(long, default_value = "")]
        path: String,
        /// lite|full|ultra (auto-resolved from the graph when omitted)
        #[arg(long, default_value = "")]
        level: String,
        /// Recurse into subdirectories (directory targets only)
        #[arg(long)]
        recurse: bool,
        /// Report savings without writing changes
        #[arg(long)]
        dry_run: bool,
    },
    /// Layer 2 (LLM mode): export prose segments to rewrite
    LlmQueue {
        /// File to export prose segments from
        #[arg(long, default_value = "")]
        path: String,
    },
    /// Layer 2 (LLM mode): merge rewritten segments back
    ApplyLlm {
        /// File to apply rewritten segments to
        #[arg(long, default_value = "")]
        path: String,
        /// JSON file of rewritten segments
        #[arg(long, default_value = "")]
        annotations: String,
    },
    /// Cumulative savings across layers
    Report,
}

/// The paths and rules a trim command works with, all under the working directory.
struct Paths {
    repo_root: PathBuf,
    rules: rules::Rules,
    graph: PathBuf,
    stats: PathBuf,
}

fn resolve() -> Paths {
    let repo_root = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let rules_path = rules::resolve_path(&repo_root, Some(&repo_root));
    Paths {
        rules: rules::load(&rules_path),
        graph: repo_root
            .join(".plaesy")
            .join("analysis")
            .join("project.graph.json"),
        stats: repo_root
            .join(".plaesy")
            .join("memory")
            .join("token-stats.json"),
        repo_root,
    }
}

pub fn run(args: &Args) -> CliResult {
    let Some(command) = &args.command else {
        return super::print_group_help("trim");
    };
    let stdout = io::stdout();
    match command {
        TrimCommand::Run { command } => {
            let paths = resolve();
            let outcome = layer1::run(&paths.rules, Some(&paths.stats), command);
            match outcome {
                Ok(result) => {
                    layer1::print(&mut stdout.lock(), &result)?;
                    Ok(())
                }
                Err(e) => Err(e.into()),
            }
        }
        TrimCommand::Compress {
            path,
            level,
            recurse,
            dry_run,
        } => {
            if path.is_empty() {
                return Err("usage: plaesy trim compress --path <file|dir> [--level lite|full|ultra] [--recurse] [--dry-run]".into());
            }
            let paths = resolve();
            let results = directory::compress_path(
                &paths.rules,
                &paths.repo_root,
                &paths.graph,
                Some(&paths.stats),
                path,
                level,
                *recurse,
                *dry_run,
            )?;
            let mut out = stdout.lock();
            for r in results {
                write!(
                    out,
                    "{} [{}]\n  {} tokens -> {} tokens  ({:.1}% saved)\n",
                    r.relative,
                    r.level,
                    r.before,
                    r.after,
                    stats::pct_saved(r.before, r.after)
                )?;
            }
            Ok(())
        }
        TrimCommand::LlmQueue { path } => {
            if path.is_empty() {
                return Err("usage: plaesy trim llm-queue --path <file>".into());
            }
            let paths = resolve();
            let (queue_path, count) = llm::write_queue(&paths.repo_root, Path::new(path))?;
            let relative = Path::new(path)
                .strip_prefix(&paths.repo_root)
                .map_or_else(|_| path.clone(), |p| p.display().to_string());
            println!(
                "Wrote {count} prose segment(s) from {relative} to {}",
                queue_path.display()
            );
            println!("Next: have the calling assistant rewrite each segment.text denser (same meaning, no code/commands touched),");
            println!("save as {{file, segments:[{{index, text}}]}} JSON, then run:");
            println!("  plaesy trim apply-llm --path {path} --annotations <annotations.json>");
            Ok(())
        }
        TrimCommand::ApplyLlm { path, annotations } => {
            if path.is_empty() || annotations.is_empty() {
                return Err(
                    "usage: plaesy trim apply-llm --path <file> --annotations <annotations.json>"
                        .into(),
                );
            }
            let paths = resolve();
            let result = llm::apply(
                &paths.repo_root,
                Some(&paths.stats),
                Path::new(path),
                Path::new(annotations),
            )?;
            println!(
                "{} [llm]\n  {} tokens -> {} tokens  ({:.1}% saved)",
                result.relative,
                result.before,
                result.after,
                stats::pct_saved(result.before, result.after)
            );
            Ok(())
        }
        TrimCommand::Report => {
            let paths = resolve();
            print!("{}", stats::report(&paths.stats));
            io::stdout().flush()?;
            Ok(())
        }
    }
}
