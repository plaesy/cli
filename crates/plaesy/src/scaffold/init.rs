//! `plaesy init`: builds `.plaesy/` in a project and sets up the chosen AI platform.

use std::fs;
use std::path::{Path, PathBuf};

use crate::common::log::Logger;
use crate::config::platform::{
    is_known_alias, normalize_platform as resolve_alias, PlatformConfig,
};

use super::config::load as load_platform_config;
use super::home::find_home;
use super::mcp::install_mcp_server;
use super::platform::{copy_agents, setup_platform_config};
use super::structure::create_structure;
use super::{Policy, ScaffoldError};

/// Options for [`init`].
#[derive(Debug, Clone, Default)]
pub struct Options {
    /// Project directory. Empty means the working directory.
    pub target_dir: PathBuf,
    /// AI platform id, display name, or alias. Empty or `none` means no platform.
    pub ai_platform: String,
    /// Overrides the Plaesy home lookup.
    pub plaesy_home: Option<PathBuf>,
}

/// Initialises a project: resolves the Plaesy home, validates the target and the
/// platform, creates `.plaesy/`, registers the MCP server, and sets up the
/// platform's files.
///
/// # Errors
///
/// Invalid target or platform, a missing Plaesy home, or I/O failures.
pub fn init(opts: &Options, log: &Logger) -> Result<(), ScaffoldError> {
    let target = if opts.target_dir.as_os_str().is_empty() {
        PathBuf::from(".")
    } else {
        opts.target_dir.clone()
    };
    let target = target.canonicalize().map_err(|e| {
        ScaffoldError::io(
            format!("resolving target directory {}", target.display()),
            e,
        )
    })?;
    validate_target_dir(&target, log)?;

    let home = find_home(opts.plaesy_home.as_deref(), log)?;
    log.info(&format!("Plaesy home: {}", home.display()));
    log.info(&format!("Target directory: {}", target.display()));

    let cfg = load_platform_config(&home)?;
    let platform = normalize_ai_platform(&opts.ai_platform, Some(&cfg));
    if platform != "none" && !cfg.has_platform(&platform) {
        return Err(ScaffoldError::InvalidPlatform {
            platform,
            available: cfg.platform_ids().into_iter().map(str::to_owned).collect(),
        });
    }
    log.success(&format!("Selected: {}", cfg.display_name(&platform)));

    let mut policy = Policy::default();
    create_structure(&home, &target, &cfg, &mut policy, log)?;
    // The MCP server is registered for every project, whatever the platform.
    install_mcp_server(&target, &policy, log)?;
    // The roles mirror is part of every project: instructions delegate to .plaesy/roles/.
    let roles = target.join(&cfg.plaesy.base_directory).join("roles");
    copy_agents(&home, &roles, "", &mut policy, log)?;

    if platform != "none" {
        setup_platform_config(&home, &target, &platform, &cfg, &mut policy, log)?;
    }

    log.success("Plaesy Spec-Kit initialization completed!");
    log.info(&format!("Project: {}", target.display()));
    log.info(&format!("Platform: {}", cfg.display_name(&platform)));
    Ok(())
}

/// The target must exist, be a directory, and accept writes. An existing `.plaesy`
/// is kept, and a warning says so.
fn validate_target_dir(target: &Path, log: &Logger) -> Result<(), ScaffoldError> {
    if !target.exists() {
        return Err(ScaffoldError::Target(format!(
            "target directory does not exist: {}",
            target.display()
        )));
    }
    if !target.is_dir() {
        return Err(ScaffoldError::Target(format!(
            "target path is not a directory: {}",
            target.display()
        )));
    }
    let probe = target.join(".plaesy-init-write-check");
    fs::write(&probe, b"").map_err(|_| {
        ScaffoldError::Target(format!(
            "target directory is not writable: {}",
            target.display()
        ))
    })?;
    let _ = fs::remove_file(&probe);
    if target.join(".plaesy").is_dir() {
        log.warning("Target directory already has .plaesy/ (existing files will be preserved, not overwritten)");
    }
    Ok(())
}

/// Resolves a user-typed platform name to a platform id, or `"none"`.
///
/// The no-platform family (`none`, `manual`, `generic`, …) maps to `none` first.
/// Then the shared alias table is consulted, with dashes read as underscores. A
/// bare id or display name is matched last.
#[must_use]
pub fn normalize_ai_platform(input: &str, cfg: Option<&PlatformConfig>) -> String {
    if input.trim().is_empty() {
        return "none".into();
    }
    let lowered = input.to_lowercase();
    if matches!(
        lowered.as_str(),
        "none" | "manual" | "generic" | "generic_ai" | "generic-ai"
    ) {
        return "none".into();
    }
    let normalized = lowered.replace('-', "_");
    let resolved = resolve_alias(&normalized, None);
    if resolved != lowered || is_known_alias(&normalized) {
        return resolved;
    }
    if let Some(cfg) = cfg {
        for id in cfg.platform_ids() {
            let name = cfg.platforms.get(id).map_or("", |p| p.name.as_str());
            if input == id || input == name {
                return id.to_owned();
            }
        }
    }
    input.to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_platform_family_maps_to_none() {
        for input in ["", "none", "Manual", "generic-ai"] {
            assert_eq!(
                normalize_ai_platform(input, None),
                "none",
                "input {input:?}"
            );
        }
    }

    #[test]
    fn aliases_resolve_to_canonical_ids() {
        assert_eq!(normalize_ai_platform("claude-code", None), "claude");
        assert_eq!(normalize_ai_platform("CLAUDE", None), "claude");
        assert_eq!(normalize_ai_platform("copilot", None), "github_copilot");
    }

    #[test]
    fn unknown_names_come_back_unchanged() {
        assert_eq!(normalize_ai_platform("Mystery Tool", None), "Mystery Tool");
    }

    #[test]
    fn missing_target_is_rejected() {
        let log = Logger::new("t");
        let missing = Path::new("definitely-not-here-plaesy");
        assert!(matches!(
            validate_target_dir(missing, &log),
            Err(ScaffoldError::Target(_))
        ));
    }
}
