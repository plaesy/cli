//! `plaesy memory`: delegates arguments to the standalone `plaesy/memory` binary.
//!
//! The binary is found through `PLAESY_MEMORY_BIN`, then `PATH`. The memory engine is an
//! independent repository and release lifecycle; this umbrella CLI only owns the subcommand.

use std::path::{Path, PathBuf};
use std::process::Command as Process;

use clap::Parser;
use sha2::{Digest, Sha256};

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
/// Resolution is explicit and profile-safe: an override wins, then the standalone `memory`
/// executable on `PATH`. The umbrella CLI does not couple its build to the memory repository.
fn binary() -> Option<PathBuf> {
    let override_path = std::env::var_os(BIN_ENV)
        .map(PathBuf::from)
        .filter(|path| path.is_file());
    if override_path.is_some() {
        return override_path;
    }

    validate::find_command("memory")
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
            "memory binary not found: install `plaesy/memory` with `cargo install --git https://github.com/plaesy/memory.git --bin memory`, set {BIN_ENV}, or install `memory` on PATH"
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
        let db_path = default_db_path(&vault)?;
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                format!(
                    "cannot create memory data directory {}: {e}",
                    parent.display()
                )
            })?;
        }
        command.env("MEMORY_DB_PATH", db_path);
    }
    let status = command
        .status()
        .map_err(|e| format!("cannot start {}: {e}", bin.display()))?;
    std::process::exit(status.code().unwrap_or(1));
}

fn default_db_path(vault: &Path) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let data_root = if cfg!(target_os = "windows") {
        std::env::var_os("LOCALAPPDATA")
            .map(PathBuf::from)
            .ok_or("LOCALAPPDATA is not set")?
    } else if cfg!(target_os = "macos") {
        std::env::var_os("HOME")
            .map(PathBuf::from)
            .ok_or("HOME is not set")?
            .join("Library")
            .join("Application Support")
    } else {
        std::env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")))
            .ok_or("XDG_DATA_HOME or HOME is not set")?
    };
    let canonical = std::fs::canonicalize(vault).unwrap_or_else(|_| vault.to_path_buf());
    let digest = Sha256::digest(canonical.to_string_lossy().as_bytes());
    let vault_id = digest[..8]
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    Ok(data_root
        .join("plaesy")
        .join("memory")
        .join(format!("{vault_id}.db")))
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
    fn default_database_is_outside_the_vault() {
        let dir = tempfile::tempdir().expect("tempdir");
        let db = default_db_path(dir.path()).expect("database path");
        assert!(!db.starts_with(dir.path()));
        assert!(db.to_string_lossy().contains("plaesy"));
        assert!(db.extension().is_some_and(|ext| ext == "db"));
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
