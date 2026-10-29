//! Per-AI-platform files: agents, the core instruction file, prompts, and hooks.
//!
//! Every destination comes from `platform.json`, so adding a platform is a
//! config change, not a code change.

use std::fs;
use std::path::{Path, PathBuf};

use crate::common::log::Logger;
use crate::config::platform::PlatformConfig;

use super::copy::copy_file;
use super::hooks::install_platform_hooks;
use super::{file_exists, Policy, ScaffoldError};

/// Sets up one platform: agents, core file, prompts, and the design-lint hook.
///
/// # Errors
///
/// I/O failures while writing files.
pub fn setup_platform_config(
    home: &Path,
    target_dir: &Path,
    platform: &str,
    cfg: &PlatformConfig,
    policy: &mut Policy,
    log: &Logger,
) -> Result<(), ScaffoldError> {
    log.info(&format!(
        "Setting up configuration for: {} based on platform.json mapping...",
        cfg.display_name(platform)
    ));

    // Agents go to the platform's agents mapping, or to .plaesy/roles/ when it has none.
    let entry = cfg.platforms.get(platform);
    let roles_dir = match entry
        .and_then(|e| e.mapping.get("agents"))
        .filter(|a| !a.is_empty())
    {
        Some(agents) => target_dir.join(agents),
        None => target_dir.join(&cfg.plaesy.base_directory).join("roles"),
    };
    copy_agents(home, &roles_dir, platform, policy, log)?;

    let Some(entry) = entry else {
        log.warning(&format!(
            "Unknown platform, skipping platform-specific file setup: {platform}"
        ));
        return Ok(());
    };

    if let Some(core) = entry.mapping.get("core").filter(|c| !c.is_empty()) {
        copy_platform_core(home, target_dir, core, cfg, policy, log)?;
    }
    if let Some(prompts) = entry.mapping.get("prompts").filter(|p| !p.is_empty()) {
        copy_platform_prompts(home, target_dir, prompts, platform, cfg, policy, log)?;
    }

    install_platform_hooks(target_dir, platform, cfg, policy, log)?;
    log.success("AI-specific configuration completed based on platform.json mapping");
    Ok(())
}

/// Copies `home/agents/*.agents.md` into `roles_dir` as `<name><ext>`.
///
/// # Errors
///
/// I/O errors creating the destination directory.
pub fn copy_agents(
    home: &Path,
    roles_dir: &Path,
    platform: &str,
    policy: &mut Policy,
    log: &Logger,
) -> Result<(), ScaffoldError> {
    let label = roles_dir
        .strip_prefix(roles_dir.parent().unwrap_or(roles_dir))
        .unwrap_or(roles_dir)
        .display()
        .to_string();
    log.info(&format!("Copying agents to {label}..."));

    let source = home.join("agents");
    fs::create_dir_all(roles_dir)
        .map_err(|e| ScaffoldError::io(format!("creating {}", roles_dir.display()), e))?;
    let Ok(entries) = fs::read_dir(&source) else {
        log.warning(&format!(
            "Source agents directory not found: {}",
            source.display()
        ));
        return Ok(());
    };

    let ext = agent_extension(platform);
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        let Some(base) = name.strip_suffix(".agents.md") else {
            continue;
        };
        if entry.path().is_dir() {
            continue;
        }
        let dst = roles_dir.join(format!("{base}{ext}"));
        match copy_file(&entry.path(), &dst, policy, log) {
            Ok(true) => log.info(&format!("  ✓ {base}{ext}")),
            Ok(false) => {}
            Err(_) => log.warning(&format!("  ✗ {base}{ext} (failed to copy)")),
        }
    }
    Ok(())
}

/// Copies the shared core file to the platform's core path, such as `CLAUDE.md`.
/// An existing core file is kept.
///
/// # Errors
///
/// I/O errors writing the file.
pub fn copy_platform_core(
    home: &Path,
    target_dir: &Path,
    dest_rel: &str,
    cfg: &PlatformConfig,
    policy: &mut Policy,
    log: &Logger,
) -> Result<(), ScaffoldError> {
    let Some(core) = cfg
        .plaesy
        .mapping
        .get("core")
        .filter(|c| !c.value.is_empty())
    else {
        return Ok(());
    };
    let src = home.join(&core.value);
    if !src.exists() {
        log.warning(&format!("Source core file not found: {}", src.display()));
        return Ok(());
    }

    let dst = target_dir.join(dest_rel);
    if let Some(parent) = dst.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| ScaffoldError::io(format!("creating {}", parent.display()), e))?;
    }
    if file_exists(&dst) {
        log.info(&format!(
            "Platform-specific core exists, skipping: {dest_rel}"
        ));
        return Ok(());
    }
    copy_file(&src, &dst, policy, log)?;
    log.success(&format!("Created platform-specific core file: {dest_rel}"));
    Ok(())
}

/// Copies every markdown prompt under the shared prompts folder into the
/// platform's prompts folder, keeping subfolders and remapping the extension.
///
/// # Errors
///
/// I/O errors creating destination folders.
pub fn copy_platform_prompts(
    home: &Path,
    target_dir: &Path,
    dest_rel_dir: &str,
    platform: &str,
    cfg: &PlatformConfig,
    policy: &mut Policy,
    log: &Logger,
) -> Result<(), ScaffoldError> {
    let Some(mapping) = cfg
        .plaesy
        .mapping
        .get("prompts")
        .filter(|m| !m.value.is_empty())
    else {
        return Ok(());
    };
    let source = home.join(mapping.value.trim_end_matches("/*"));
    let dest_dir = target_dir.join(dest_rel_dir);
    fs::create_dir_all(&dest_dir)
        .map_err(|e| ScaffoldError::io(format!("creating {}", dest_dir.display()), e))?;

    let ext = prompt_extension(platform);
    let mut files = Vec::new();
    collect_markdown(&source, &mut files);
    for file in files {
        let Ok(rel) = file.strip_prefix(&source) else {
            continue;
        };
        let rel = rel.to_string_lossy().trim_end_matches(".md").to_owned();
        let dst = dest_dir.join(format!("{rel}{ext}"));
        if let Some(parent) = dst.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| ScaffoldError::io(format!("creating {}", parent.display()), e))?;
        }
        match copy_file(&file, &dst, policy, log) {
            Ok(_) => log.success(&format!(
                "Copied prompt: {}",
                PathBuf::from(dest_rel_dir)
                    .join(format!("{rel}{ext}"))
                    .display()
            )),
            Err(_) => log.warning(&format!("  ✗ {rel}{ext} (failed to copy)")),
        }
    }
    Ok(())
}

/// Lists markdown files under `dir`, recursively, in sorted order. Unreadable
/// entries are skipped.
fn collect_markdown(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    let mut paths: Vec<PathBuf> = entries.flatten().map(|e| e.path()).collect();
    paths.sort();
    for path in paths {
        if path.is_dir() {
            collect_markdown(&path, out);
        } else if path.extension().is_some_and(|e| e == "md") {
            out.push(path);
        }
    }
}

/// Prompt file extension for a platform. GitHub Copilot uses `.prompt.md`.
#[must_use]
pub fn prompt_extension(platform: &str) -> &'static str {
    match platform {
        "github_copilot" => ".prompt.md",
        _ => ".md",
    }
}

/// Agent file extension for a platform. GitHub Copilot uses `.agent.md`.
#[must_use]
pub fn agent_extension(platform: &str) -> &'static str {
    match platform {
        "github_copilot" => ".agent.md",
        _ => ".md",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn copilot_uses_its_own_extensions() {
        assert_eq!(prompt_extension("github_copilot"), ".prompt.md");
        assert_eq!(agent_extension("github_copilot"), ".agent.md");
        assert_eq!(prompt_extension("claude"), ".md");
        assert_eq!(agent_extension(""), ".md");
    }
}
