//! `plaesy images`: generated image assets through a provider API.

use std::path::PathBuf;

use super::common::logger;
use super::CliResult;
use crate::imagegen::{self, Options};

/// Work with generated image assets. With no subcommand, prints the subcommands.
#[derive(Debug, clap::Args)]
pub struct Args {
    #[command(subcommand)]
    pub command: Option<ImagesCommand>,
}

#[derive(Debug, clap::Subcommand)]
pub enum ImagesCommand {
    /// Generate an image asset via a configured provider API
    Create(CreateArgs),
}

#[derive(Debug, clap::Args)]
pub struct CreateArgs {
    /// Image prompt text (required)
    #[arg(long, default_value = "")]
    pub prompt: String,
    /// Image provider: openai (default) or gemini
    #[arg(long, default_value = "")]
    pub provider: String,
    /// Image size, e.g. 1024x1024
    #[arg(long, default_value = "")]
    pub size: String,
    /// Output file path (required)
    #[arg(long, default_value = "")]
    pub out: String,
}

pub fn run(args: &Args) -> CliResult {
    match &args.command {
        Some(ImagesCommand::Create(c)) => create(c),
        None => super::print_group_help("images"),
    }
}

fn create(args: &CreateArgs) -> CliResult {
    if args.prompt.is_empty() || args.out.is_empty() {
        // Usage errors exit non-zero, so a script can tell "forgot flags" from "generated".
        return Err("usage: plaesy images create --prompt <text> --out <file>".into());
    }
    let log = logger();
    let opts = Options {
        prompt: args.prompt.clone(),
        provider: args.provider.clone(),
        size: args.size.clone(),
        out: PathBuf::from(&args.out),
    };
    match imagegen::generate(&opts) {
        Ok(path) => {
            println!("{}", path.display());
            Ok(())
        }
        Err(e) => {
            log.error(&e.to_string());
            Err(e.into())
        }
    }
}
