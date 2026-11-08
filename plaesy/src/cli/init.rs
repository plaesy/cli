//! `plaesy init`: scaffolds a new Plaesy project.

use std::path::PathBuf;

use super::common::logger;
use super::CliResult;
use crate::scaffold::init::{self, Options};
use crate::scaffold::native_install::{self, DEFAULT_SOURCE};

#[derive(Debug, clap::Args)]
pub struct Args {
    /// Directory to scaffold (default: current directory)
    pub directory: Option<PathBuf>,
    /// AI platform to configure (e.g. claude, cursor_ai, github_copilot); default: none
    #[arg(long, default_value = "")]
    pub ai: String,
    /// Print the native install commands for --ai and change nothing
    #[arg(long)]
    pub dry_run: bool,
    /// Override the Plaesy repo root (also read from the PLAESY_HOME environment variable)
    #[arg(long = "plaesy-home")]
    pub plaesy_home: Option<PathBuf>,
}

pub fn run(args: &Args) -> CliResult {
    if args.dry_run {
        return print_plan(&args.ai);
    }
    let opts = Options {
        target_dir: args.directory.clone().unwrap_or_else(|| PathBuf::from(".")),
        ai_platform: args.ai.clone(),
        plaesy_home: args.plaesy_home.clone(),
    };
    init::init(&opts, &logger())?;
    Ok(())
}

/// Prints the native install plan for `platform`, or says there is none. Changes nothing.
fn print_plan(platform: &str) -> CliResult {
    if platform.is_empty() {
        println!("Choose a platform with --ai, for example: plaesy init --ai claude --dry-run");
        return Ok(());
    }
    match native_install::plan(platform, DEFAULT_SOURCE) {
        Some(plan) => print!("{}", native_install::render(&plan)),
        None => println!("{platform} has no native install route; nothing to run."),
    }
    Ok(())
}
