//! The `.plaesy/` tree: directories, seeded files, and the loop state.

use std::fs;
use std::path::Path;

use chrono::{SecondsFormat, Utc};

use crate::common::log::Logger;
use crate::config::platform::PlatformConfig;

use super::copy::{copy_checklists, copy_file, copy_instructions, copy_templates};
use super::{file_exists, Outcome, Policy, ScaffoldError};

/// Creates `<target>/memory`, `<target>/analysis`, and `<target>/context.md`.
///
/// # Errors
///
/// I/O errors creating the directories.
pub fn create_memory_structure(
    home: &Path,
    target_dir: &Path,
    policy: &mut Policy,
    log: &Logger,
) -> Result<(), ScaffoldError> {
    log.info("Creating memory and analysis directories...");
    for dir in ["memory", "analysis"] {
        let path = target_dir.join(dir);
        fs::create_dir_all(&path)
            .map_err(|e| ScaffoldError::io(format!("creating {}", path.display()), e))?;
    }

    let target = target_dir.join("context.md");
    let template = home.join("skills/plaesy/assets/context.template.md");
    if file_exists(&target) && policy.is_protected(&target) {
        log.warning("  ⊘ context.md (your file, left alone)");
        policy.report.record(&target, Outcome::Protected);
    } else if file_exists(&target) && !policy.overwrite {
        log.warning("  ⊘ context.md (already exists, skipping)");
        policy.report.record(&target, Outcome::Unchanged);
    } else if file_exists(&template) {
        if let Ok(true) = copy_file(&template, &target, policy, log) {
            if !policy.dry_run {
                log.info("  ✓ context.md");
            }
        }
    } else if policy.dry_run {
        log.info("  + context.md (would create empty)");
        policy.report.record(&target, Outcome::Created);
    } else {
        // An absent file reads as "never initialised" to every command, so an
        // empty one is still written when the template is missing.
        log.warning("  ! context.template.md not found, creating empty file");
        let _ = fs::write(&target, b"");
        log.info("  ✓ context.md (empty)");
    }

    log.success("Memory and analysis structure created");
    Ok(())
}

/// Creates `<target>/decisions/` and an empty `decisions.md` index.
///
/// # Errors
///
/// I/O errors creating the directory.
pub fn create_decision_structure(
    home: &Path,
    target_dir: &Path,
    policy: &mut Policy,
    log: &Logger,
) -> Result<(), ScaffoldError> {
    log.info("Creating decision management structure...");
    let decisions_dir = target_dir.join("decisions");
    fs::create_dir_all(&decisions_dir)
        .map_err(|e| ScaffoldError::io(format!("creating {}", decisions_dir.display()), e))?;

    let target = target_dir.join("decisions.md");
    let template = home.join("skills/plaesy/assets/decisions.template.md");
    if file_exists(&target) && policy.is_protected(&target) {
        log.warning("  ⊘ decisions.md (your file, left alone)");
        policy.report.record(&target, Outcome::Protected);
    } else if file_exists(&target) && !policy.overwrite {
        log.warning("  ⊘ decisions.md (already exists, skipping)");
        policy.report.record(&target, Outcome::Unchanged);
    } else if !file_exists(&template) {
        log.warning("  ! decisions.template.md not found, creating empty file");
        match fs::write(&target, b"") {
            Ok(()) => log.info("  ✓ decisions.md (empty)"),
            Err(e) => log.warning(&format!("  ✗ decisions.md (could not create: {e})")),
        }
    } else {
        match copy_file(&template, &target, policy, log) {
            Ok(_) if !policy.dry_run => log.info("  ✓ decisions.md"),
            Ok(_) => {}
            Err(e) => log.warning(&format!("  ✗ decisions.md (failed to copy: {e})")),
        }
    }

    log.success("Decision management structure created");
    Ok(())
}

/// Writes `state.json` from its template, replacing the timestamp placeholder with
/// the current UTC time. Both `[TIMESTAMP]` and `{{TIMESTAMP}}` are accepted.
///
/// An existing `state.json` is kept.
///
/// # Errors
///
/// I/O errors writing the file.
pub fn create_loop_state(
    home: &Path,
    target_dir: &Path,
    policy: &mut Policy,
    log: &Logger,
) -> Result<(), ScaffoldError> {
    let target = target_dir.join("state.json");
    if target.exists() {
        log.warning("  ⊘ state.json (already exists, skipping)");
        policy.report.record(&target, Outcome::Unchanged);
        return Ok(());
    }

    let template = home.join("skills/plaesy/assets/state.template.json");
    let Ok(text) = fs::read_to_string(&template) else {
        log.warning("  ! state.template.json not found, creating empty file");
        let _ = fs::write(&target, b"");
        log.info("  ✓ state.json (empty)");
        return Ok(());
    };

    let timestamp = Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true);
    let rendered = text
        .replace("[TIMESTAMP]", &timestamp)
        .replace("{{TIMESTAMP}}", &timestamp);
    if policy.dry_run {
        log.info("  + state.json (would create)");
        policy.report.record(&target, Outcome::Created);
        return Ok(());
    }
    fs::write(&target, rendered)
        .map_err(|e| ScaffoldError::io(format!("writing {}", target.display()), e))?;
    log.info("  ✓ state.json");
    policy.report.record(&target, Outcome::Created);
    Ok(())
}

/// Builds `<target>/<base_directory>` with its core directories, copies the
/// shared files into it, and creates the project directories beside it.
///
/// # Errors
///
/// I/O failures in any step.
pub fn create_structure(
    home: &Path,
    target_dir: &Path,
    cfg: &PlatformConfig,
    policy: &mut Policy,
    log: &Logger,
) -> Result<(), ScaffoldError> {
    log.info("Creating Plaesy structure...");
    let base = target_dir.join(&cfg.plaesy.base_directory);
    fs::create_dir_all(&base)
        .map_err(|e| ScaffoldError::io(format!("creating {}", base.display()), e))?;

    for dir in cfg.plaesy.core_directories.iter().filter(|d| !d.is_empty()) {
        log.info(&format!(
            "Creating core directory: '{}'",
            Path::new(&cfg.plaesy.base_directory).join(dir).display()
        ));
        let path = base.join(dir);
        fs::create_dir_all(&path)
            .map_err(|e| ScaffoldError::io(format!("creating {}", path.display()), e))?;
    }

    copy_instructions(home, &base, policy, log)?;
    copy_templates(home, &base, policy, log)?;
    copy_checklists(home, &base, policy, log)?;
    create_memory_structure(home, &base, policy, log)?;
    create_decision_structure(home, &base, policy, log)?;
    create_loop_state(home, &base, policy, log)?;

    for dir in cfg
        .plaesy
        .project_directories
        .iter()
        .filter(|d| !d.is_empty())
    {
        log.info(&format!("Creating project directory: '{dir}'"));
        let path = target_dir.join(dir);
        fs::create_dir_all(&path)
            .map_err(|e| ScaffoldError::io(format!("creating {}", path.display()), e))?;
    }

    log.success("Plaesy structure created with dynamic configuration");
    Ok(())
}
