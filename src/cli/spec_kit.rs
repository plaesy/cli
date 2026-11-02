//! `plaesy spec-kit`: namespace for the built-in Constitution Kit workflow.
//!
//! The umbrella CLI owns the `spec-kit` namespace while preserving the existing command surface,
//! so both `plaesy init` and `plaesy spec-kit init` remain usable during the transition.

use clap::Parser;

use super::{Cli, CliResult};

#[derive(Debug, Parser)]
#[command(disable_help_flag = true)]
pub struct Args {
    /// Arguments for the Spec Kit command (for example `init`, `analyze`, or `validate memory`).
    #[arg(trailing_var_arg = true, allow_hyphen_values = true, num_args = 0..)]
    pub args: Vec<String>,
}

/// Dispatches a namespaced Spec Kit command through the existing command implementation.
pub fn run(args: &Args) -> CliResult {
    if args.args.is_empty() || args.args.iter().any(|arg| arg == "--help" || arg == "-h") {
        return super::print_group_help("spec-kit");
    }

    let mut argv = vec!["plaesy".to_owned()];
    argv.extend(args.args.iter().cloned());
    let cli = Cli::try_parse_from(argv)?;
    super::run(cli)
}
