//! `plaesy memory`: passes the arguments to the `memory` binary built from the `memory` submodule.
//!
//! The binary is found through `PLAESY_MEMORY_BIN`, then on `PATH`. Build it with
//! `cargo build --release` inside `memory/`.

use std::path::PathBuf;
use std::process::Command as Process;

use clap::Parser;

use super::CliResult;
use crate::common::validate;

/// Environment variable that overrides the location of the `memory` binary.
const BIN_ENV: &str = "PLAESY_MEMORY_BIN";

#[derive(Debug, Parser)]
#[command(disable_help_flag = true)]
pub struct Args {
    /// Arguments passed to `memory` unchanged (for example `search "query"` or `--help`)
    #[arg(trailing_var_arg = true, allow_hyphen_values = true, num_args = 0..)]
    pub args: Vec<String>,
}

/// Finds the `memory` binary: `PLAESY_MEMORY_BIN` first, then `PATH`.
fn binary() -> Option<PathBuf> {
    std::env::var_os(BIN_ENV)
        .map(PathBuf::from)
        .filter(|path| path.is_file())
        .or_else(|| validate::find_command("memory"))
}

/// Runs `memory` with the given arguments and exits with its status code.
///
/// # Errors
///
/// The `memory` binary is not found, or it cannot be started.
pub fn run(args: &Args) -> CliResult {
    let Some(bin) = binary() else {
        return Err(format!(
            "memory binary not found: run `cargo build --release` in memory/, or set {BIN_ENV}"
        )
        .into());
    };
    let status = Process::new(&bin)
        .args(&args.args)
        .status()
        .map_err(|e| format!("cannot start {}: {e}", bin.display()))?;
    std::process::exit(status.code().unwrap_or(1));
}
