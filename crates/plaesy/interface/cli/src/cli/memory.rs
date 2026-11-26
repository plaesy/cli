//! `plaesy memory`: run the memory engine in-process.

use clap::Parser;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

use super::CliResult;
#[derive(Debug, Parser)]
#[command(disable_help_flag = true)]
pub struct Args {
    /// Shared vault to index instead of the project-local `.plaesy` directory.
    #[arg(long, conflicts_with = "project")]
    pub vault: Option<PathBuf>,
    /// Explicitly use the current project's `.plaesy` directory.
    #[arg(long, conflicts_with = "vault")]
    pub project: bool,
    /// Memory-engine arguments (for example `search "query"` or `--help`).
    #[arg(trailing_var_arg = true, allow_hyphen_values = true, num_args = 0..)]
    pub args: Vec<String>,
}

/// Runs the memory engine directly, using the same vault selection as other Plaesy commands.
///
/// By default, Plaesy uses the shared `~/_` vault when it exists. `--project` opts into the
/// project-local `.plaesy` tree, while `--vault` and `PLAESY_MEMORY_VAULT` select another vault.
/// Explicit CLI vault selectors win over environment and config vault paths. Without a CLI
/// selector, `MEMORY_WATCH_DIR` wins over `PLAESY_MEMORY_VAULT`. Explicit database paths from
/// settings, TOML config, `MEMORY_DB_PATH`, or `--db` are honored; otherwise the database is
/// isolated per vault.
///
/// # Errors
///
/// The selected vault cannot be resolved, the runtime cannot start, or the memory engine fails.
pub fn run(args: &Args) -> CliResult {
    if is_onboard_command(&args.args) {
        return Err("onboarding moved to `plaesy onboard`".into());
    }
    let displays_help_or_version = args
        .args
        .iter()
        .any(|arg| matches!(arg.as_str(), "-h" | "--help" | "-V" | "--version"));
    if !displays_help_or_version && !is_schedule_uninstall(&args.args) {
        let cli_selected_vault = args.vault.is_some() || args.project;
        let has_config = has_config_arg(&args.args);
        let config_path = config_path_arg(&args.args);
        let has_db = has_db_arg(&args.args);
        let defer_to_config = has_config
            && !cli_selected_vault
            && std::env::var_os("MEMORY_WATCH_DIR").is_none()
            && std::env::var_os("PLAESY_MEMORY_VAULT").is_none();

        let selected = if defer_to_config {
            None
        } else {
            Some(selected_vault(args)?)
        };
        if let Some(vault) = &selected {
            if cli_selected_vault || std::env::var_os("MEMORY_WATCH_DIR").is_none() {
                std::env::set_var("MEMORY_WATCH_DIR", vault);
            }
        }

        if !has_db && std::env::var_os("MEMORY_DB_PATH").is_none() {
            let config_defaults = config_path
                .as_deref()
                .map(read_config_vault_defaults)
                .transpose()?;
            let has_config_db = config_defaults
                .as_ref()
                .is_some_and(|defaults| defaults.database_is_explicit)
                || (!has_config && has_standard_config_database_override()?);
            if !has_config_db {
                let vault = selected
                    .or_else(|| config_defaults.map(|defaults| defaults.watch_dir))
                    .unwrap_or_else(|| PathBuf::from(".plaesy"));
                let db_path = default_db_path(&vault)?;
                if let Some(parent) = db_path.parent() {
                    std::fs::create_dir_all(parent).map_err(|e| {
                        format!(
                            "cannot create memory data directory {}: {e}",
                            parent.display()
                        )
                    })?;
                }
                std::env::set_var("MEMORY_DB_PATH", db_path);
            }
        }
    } else if args
        .args
        .iter()
        .any(|arg| matches!(arg.as_str(), "-h" | "--help"))
    {
        println!("Vault selection (place before the memory subcommand):");
        println!("  --vault <PATH>   Use a specific shared vault");
        println!("  --project        Use the current repository's .plaesy directory");
        println!("Example: plaesy memory --vault C:/vault search \"query\"");
        println!();
    }
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    runtime.block_on(crate::memory::cli::run_args(args.args.clone()))?;
    Ok(())
}

/// Arguments for the top-level `plaesy onboard`, forwarded to the memory engine's onboarding flow.
#[derive(Debug, Parser)]
#[command(disable_help_flag = true)]
pub struct OnboardArgs {
    /// Onboarding options (for example `-y --vault C:/notes`, or `--help`).
    #[arg(trailing_var_arg = true, allow_hyphen_values = true, num_args = 0..)]
    pub args: Vec<String>,
}

/// `plaesy onboard`: first-run setup for the memory engine. Selects the user config root;
/// Configuration is stored in `config.toml`.
///
/// # Errors
///
/// The memory engine's onboarding fails (for example the vault folder is missing).
pub fn run_onboard(args: &OnboardArgs) -> CliResult {
    let mut forwarded = vec!["onboard".to_string()];
    forwarded.extend(args.args.iter().cloned());
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    runtime.block_on(crate::memory::cli::run_args(forwarded))?;
    Ok(())
}

fn default_db_path(vault: &Path) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let canonical = std::fs::canonicalize(vault).unwrap_or_else(|_| vault.to_path_buf());
    let digest = Sha256::digest(canonical.to_string_lossy().as_bytes());
    let vault_id = digest[..8]
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    let data_root =
        crate::memory::paths::data_root().ok_or("could not resolve the local data directory")?;
    Ok(data_root.join("memory").join(format!("{vault_id}.db")))
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
    if let Some(vault) = std::env::var_os("MEMORY_WATCH_DIR") {
        return existing_directory(Path::new(&vault));
    }
    if let Some(vault) = std::env::var_os("PLAESY_MEMORY_VAULT") {
        return existing_directory(Path::new(&vault));
    }
    if let Some(vault) = standard_config_watch_dir(args)? {
        return existing_directory(&vault);
    }
    if let Some(home) = dirs::home_dir() {
        let shared = home.join("_");
        if shared.is_dir() {
            return Ok(shared);
        }
    }
    let root =
        crate::common::git::repo_root().map_err(|e| format!("cannot find project root: {e}"))?;
    existing_directory(&root.join(".plaesy"))
}

fn has_config_arg(args: &[String]) -> bool {
    args.iter()
        .any(|arg| arg == "--config" || arg.starts_with("--config="))
}

fn is_onboard_command(args: &[String]) -> bool {
    let mut skip_value = false;
    for arg in args {
        if skip_value {
            skip_value = false;
            continue;
        }
        if arg == "--config" || arg == "--db" {
            skip_value = true;
            continue;
        }
        if arg.starts_with("--config=") || arg.starts_with("--db=") || arg.starts_with('-') {
            continue;
        }
        return arg == "onboard";
    }
    false
}

fn is_schedule_uninstall(args: &[String]) -> bool {
    args.windows(2)
        .any(|pair| pair[0] == "schedule" && pair[1] == "uninstall")
}

fn has_db_arg(args: &[String]) -> bool {
    args.iter()
        .any(|arg| arg == "--db" || arg.starts_with("--db="))
}

fn config_path_arg(args: &[String]) -> Option<PathBuf> {
    args.iter().enumerate().find_map(|(index, arg)| {
        if arg == "--config" {
            args.get(index + 1).map(PathBuf::from)
        } else {
            arg.strip_prefix("--config=").map(PathBuf::from)
        }
    })
}

struct ConfigVaultDefaults {
    watch_dir: PathBuf,
    watch_dir_is_explicit: bool,
    database_is_explicit: bool,
}

fn read_config_vault_defaults(
    path: &Path,
) -> Result<ConfigVaultDefaults, Box<dyn std::error::Error>> {
    let source = std::fs::read_to_string(path)
        .map_err(|error| format!("cannot read settings {}: {error}", path.display()))?;
    // config.toml is the current format; a legacy settings.json is still read as JSON.
    let config: serde_json::Value = if path.extension().is_some_and(|ext| ext == "toml") {
        let table: toml::Table = toml::from_str(&source)
            .map_err(|error| format!("invalid settings {}: {error}", path.display()))?;
        serde_json::to_value(table)?
    } else {
        serde_json::from_str(&source)
            .map_err(|error| format!("invalid settings {}: {error}", path.display()))?
    };
    let configured_watch_dir = config
        .get("watch_dir")
        .and_then(serde_json::Value::as_str)
        .map(PathBuf::from);
    let watch_dir_is_explicit = configured_watch_dir.is_some();
    let watch_dir = configured_watch_dir.unwrap_or_else(|| PathBuf::from(".plaesy"));
    let database_is_explicit = config
        .get("database")
        .and_then(serde_json::Value::as_object)
        .is_some_and(|database| database.contains_key("db_path"));
    Ok(ConfigVaultDefaults {
        watch_dir,
        watch_dir_is_explicit,
        database_is_explicit,
    })
}

/// The vault set by a settings file. The user file (`plaesy onboard`) always applies; the project
/// file applies only without `--config`, because a supplied `--config` replaces the standard files
/// and is handled directly by the memory loader.
fn standard_config_watch_dir(args: &Args) -> Result<Option<PathBuf>, Box<dyn std::error::Error>> {
    let user_path = crate::memory::settings::default_path();
    let mut watch_dir = None;
    for path in crate::memory::config::standard_config_paths()
        .into_iter()
        .filter(|path| path.is_file())
    {
        let is_user_file = user_path.as_deref() == Some(path.as_path());
        if !is_user_file && has_config_arg(&args.args) {
            continue;
        }
        let defaults = read_config_vault_defaults(&path)?;
        if defaults.watch_dir_is_explicit {
            watch_dir = Some(defaults.watch_dir);
        }
    }
    Ok(watch_dir)
}

fn has_standard_config_database_override() -> Result<bool, Box<dyn std::error::Error>> {
    for path in crate::memory::config::standard_config_paths()
        .into_iter()
        .filter(|path| path.is_file())
    {
        if read_config_vault_defaults(&path)?.database_is_explicit {
            return Ok(true);
        }
    }
    Ok(false)
}

fn existing_directory(path: &Path) -> Result<PathBuf, Box<dyn std::error::Error>> {
    if path.is_dir() {
        Ok(path.to_path_buf())
    } else {
        Err(format!("memory vault is not a directory: {}", path.display()).into())
    }
}

#[cfg(test)]
#[path = "memory_test.rs"]
mod memory_test;
