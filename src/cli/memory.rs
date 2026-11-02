//! `plaesy memory`: passes the arguments to the `memory` binary built from the `memory` submodule.
//!
//! The binary is found through `PLAESY_MEMORY_BIN`, then on `PATH`. Build it with
//! `cargo build --release` inside `memory/`.

use std::path::{Path, PathBuf};
use std::process::Command as Process;

use clap::Parser;

use super::CliResult;
use crate::common::validate;

/// Environment variable that overrides the location of the `memory` binary.
const BIN_ENV: &str = "PLAESY_MEMORY_BIN";

#[derive(Debug, Parser)]
#[command(disable_help_flag = true)]
pub struct Args {
    /// Shared vault to index instead of the project-local `.plaesy` directory.
    #[arg(long, conflicts_with = "project")]
    pub vault: Option<PathBuf>,
    /// Explicitly use the current project's `.plaesy` directory.
    #[arg(long, conflicts_with = "vault")]
    pub project: bool,
    /// Arguments passed to `memory` unchanged (for example `search "query"` or `--help`).
    #[arg(trailing_var_arg = true, allow_hyphen_values = true, num_args = 0..)]
    pub args: Vec<String>,
}

/// Finds the `memory` binary.
///
/// Resolution is explicit and profile-safe: an override wins, then the binary built from
/// Plaesy's checked-out `memory` submodule, then `PATH`. The submodule lookup makes a fresh
/// Plaesy checkout usable without installing a second global binary.
fn binary() -> Option<PathBuf> {
    let override_path = std::env::var_os(BIN_ENV)
        .map(PathBuf::from)
        .filter(|path| path.is_file());
    if override_path.is_some() {
        return override_path;
    }

    let repo_root = crate::common::git::repo_root().ok();
    let submodule_binary = repo_root
        .into_iter()
        .map(|root| {
            root.join("memory")
                .join("target")
                .join("release")
                .join("memory")
        })
        .find(|path| path.is_file());

    submodule_binary.or_else(|| validate::find_command("memory"))
}

/// Runs `memory` with the given arguments and exits with its status code.
///
/// By default, Plaesy uses the shared `~/_` vault when it exists. `--project` opts into the
/// project-local `.plaesy` tree, while `--vault` and `PLAESY_MEMORY_VAULT` select another vault.
/// Explicit `MEMORY_WATCH_DIR` and `MEMORY_DB_PATH` values always win.
///
/// # Errors
///
/// The `memory` binary is not found, the selected vault cannot be resolved, or the child cannot
/// be started.
pub fn run(args: &Args) -> CliResult {
    let Some(bin) = binary() else {
        return Err(format!(
            "memory binary not found: run `cargo build --release` in memory/, set {BIN_ENV}, or install `memory` on PATH"
        )
        .into());
    };

    let vault = selected_vault(args)?;
    let mut command = Process::new(&bin);
    command.args(&args.args);
    if std::env::var_os("MEMORY_WATCH_DIR").is_none() {
        command.env("MEMORY_WATCH_DIR", &vault);
    }
    if std::env::var_os("MEMORY_DB_PATH").is_none() {
        let db_dir = vault.join(".memory");
        std::fs::create_dir_all(&db_dir).map_err(|e| {
            format!(
                "cannot create memory index directory {}: {e}",
                db_dir.display()
            )
        })?;
        command.env("MEMORY_DB_PATH", db_dir.join("index.db"));
    }
    let status = command
        .status()
        .map_err(|e| format!("cannot start {}: {e}", bin.display()))?;
    std::process::exit(status.code().unwrap_or(1));
}

fn selected_vault(args: &Args) -> Result<PathBuf, Box<dyn std::error::Error>> {
    if let Some(vault) = &args.vault {
        return existing_directory(vault);
    }
    if args.project {
        let root = crate::common::git::repo_root()
            .map_err(|e| format!("cannot find project root for --project: {e}"))?;
        return existing_directory(&root.join(".plaesy"));
    }
    if let Some(vault) = std::env::var_os("PLAESY_MEMORY_VAULT") {
        return existing_directory(Path::new(&vault));
    }
    if let Some(home) = std::env::var_os("HOME") {
        let shared = Path::new(&home).join("_");
        if shared.is_dir() {
            return Ok(shared);
        }
    }
    let root =
        crate::common::git::repo_root().map_err(|e| format!("cannot find project root: {e}"))?;
    existing_directory(&root.join(".plaesy"))
}

fn existing_directory(path: &Path) -> Result<PathBuf, Box<dyn std::error::Error>> {
    if path.is_dir() {
        Ok(path.to_path_buf())
    } else {
        Err(format!("memory vault is not a directory: {}", path.display()).into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_vault_is_selected() {
        let dir = tempfile::tempdir().expect("tempdir");
        let args = Args {
            vault: Some(dir.path().to_path_buf()),
            project: false,
            args: Vec::new(),
        };
        assert_eq!(selected_vault(&args).unwrap(), dir.path());
    }

    #[test]
    fn missing_explicit_vault_is_rejected() {
        let dir = tempfile::tempdir().expect("tempdir");
        let missing = dir.path().join("missing");
        let args = Args {
            vault: Some(missing.clone()),
            project: false,
            args: Vec::new(),
        };
        let error = selected_vault(&args).unwrap_err().to_string();
        assert!(error.contains("memory vault is not a directory"));
        assert!(error.contains(&missing.display().to_string()));
    }
}
