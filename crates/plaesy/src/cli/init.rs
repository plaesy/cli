//! `plaesy init`: scaffolds a new Plaesy project.

use std::path::PathBuf;

use super::common::logger;
use super::CliResult;
use crate::scaffold::init::{self, Options};

#[derive(Debug, clap::Args)]
pub struct Args {
    /// Directory to scaffold (default: current directory)
    pub directory: Option<PathBuf>,
    /// AI platform to configure (e.g. claude, cursor_ai, github_copilot); default: none
    #[arg(long, default_value = "")]
    pub ai: String,
    /// Override the Plaesy repo root (also read from the PLAESY_HOME environment variable)
    #[arg(long = "plaesy-home")]
    pub plaesy_home: Option<PathBuf>,
}

pub fn run(args: &Args) -> CliResult {
    let opts = Options {
        target_dir: args.directory.clone().unwrap_or_else(|| PathBuf::from(".")),
        ai_platform: args.ai.clone(),
        plaesy_home: args.plaesy_home.clone(),
    };
    init::init(&opts, &logger())?;
    Ok(())
}
