//! File copies under the copy policy: instructions, templates, checklists.

use std::fs;
use std::path::Path;

use crate::common::log::Logger;

use super::config::{instruction_lists, instruction_name};
use super::{dir_has_files, file_exists, Outcome, Policy, ScaffoldError};

/// Copies `src` to `dst` under `policy`. Returns `true` when `dst` was written.
///
/// An existing destination is skipped unless `policy.overwrite` is set. Even
/// then, identical contents are reported as unchanged, so a reload log shows
/// only real changes. Protected destinations are never overwritten.
///
/// # Errors
///
/// [`ScaffoldError::Io`] when a required write fails.
pub fn copy_file(
    src: &Path,
    dst: &Path,
    policy: &mut Policy,
    log: &Logger,
) -> Result<bool, ScaffoldError> {
    let name = file_name(dst);
    if dst.exists() {
        if policy.is_protected(dst) {
            log.warning(&format!("  ⊘ {name} (your file, left alone)"));
            policy.report.record(dst, Outcome::Protected);
            return Ok(false);
        }
        if !policy.overwrite {
            log.warning(&format!("  ⊘ {name} (already exists, skipping)"));
            policy.report.record(dst, Outcome::Unchanged);
            return Ok(false);
        }
        match same_contents(src, dst) {
            Ok(true) => {
                log.info(&format!("  = {name} (unchanged)"));
                policy.report.record(dst, Outcome::Unchanged);
                return Ok(false);
            }
            Ok(false) => {}
            Err(_) => log.warning(&format!("  ! {name} (could not compare, copying anyway)")),
        }
        if policy.dry_run {
            log.info(&format!("  ~ {name} (would update)"));
            policy.report.record(dst, Outcome::Updated);
            return Ok(true);
        }
        if let Err(e) = write_copy(src, dst) {
            policy.report.record(dst, Outcome::Failed);
            return Err(e);
        }
        log.info(&format!("  ↑ {name} (updated)"));
        policy.report.record(dst, Outcome::Updated);
        return Ok(true);
    }

    if policy.dry_run {
        log.info(&format!("  + {name} (would create)"));
        policy.report.record(dst, Outcome::Created);
        return Ok(true);
    }
    if let Err(e) = write_copy(src, dst) {
        policy.report.record(dst, Outcome::Failed);
        return Err(e);
    }
    policy.report.record(dst, Outcome::Created);
    Ok(true)
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// Compares contents. A read error on either side is an error, not "different",
/// so a permission problem is not hidden by forcing a copy.
fn same_contents(src: &Path, dst: &Path) -> std::io::Result<bool> {
    Ok(fs::read(src)? == fs::read(dst)?)
}

/// Writes `src` to `dst`, creating parent directories. `fs::copy` carries the
/// permission bits over, which keeps copied scripts runnable.
fn write_copy(src: &Path, dst: &Path) -> Result<(), ScaffoldError> {
    if let Some(parent) = dst.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| ScaffoldError::io(format!("creating {}", parent.display()), e))?;
    }
    fs::copy(src, dst)
        .map(|_| ())
        .map_err(|e| ScaffoldError::io(format!("copying to {}", dst.display()), e))
}

/// Copies the always-load and scope-load instructions into `<target>/instructions/`,
/// then the memory note. Source names have their `.instructions` segment removed.
///
/// # Errors
///
/// I/O errors creating the instruction folder or writing the memory note.
pub fn copy_instructions(
    home: &Path,
    target_dir: &Path,
    policy: &mut Policy,
    log: &Logger,
) -> Result<(), ScaffoldError> {
    if !dir_has_files(home) {
        log.warning("Skipping instruction copy - source directory issues");
        return Ok(());
    }

    log.info("Copying instructions to .plaesy/instructions/...");
    let instructions_dir = target_dir.join("instructions");
    fs::create_dir_all(&instructions_dir)
        .map_err(|e| ScaffoldError::io(format!("creating {}", instructions_dir.display()), e))?;

    log.info("  (loading always-load instructions from mapping.json)");
    let lists = match instruction_lists(home) {
        Ok(lists) => lists,
        Err(e) => {
            log.warning(&format!(
                "  ✗ could not load mapping.json always_load list: {e}"
            ));
            return Ok(());
        }
    };
    copy_listed(home, &instructions_dir, &lists.always_load, "", policy, log)?;

    // Scope-load files are available on demand to scoped commands, but are not
    // loaded into the base context.
    log.info("  (loading scope-load instructions from mapping.json)");
    copy_listed(
        home,
        &instructions_dir,
        &lists.scope_load,
        " (scope-load)",
        policy,
        log,
    )?;

    copy_memory_note(home, target_dir, policy, log)?;
    log.success("Instructions copied");
    Ok(())
}

fn copy_listed(
    home: &Path,
    dest_dir: &Path,
    names: &[String],
    label: &str,
    policy: &mut Policy,
    log: &Logger,
) -> Result<(), ScaffoldError> {
    for name in names {
        let src = home.join(name);
        if !file_exists(&src) {
            log.warning(&format!("  ? {name} (source not found, skipping)"));
            continue;
        }
        let base = instruction_name(name).trim_end_matches(".md").to_owned();
        let dst = dest_dir.join(format!("{base}.md"));
        match copy_file(&src, &dst, policy, log) {
            Ok(true) if !policy.dry_run => log.info(&format!("  ✓ {base}.md{label}")),
            Ok(_) => {}
            Err(_) => log.warning(&format!("  ✗ {base}.md (failed to copy)")),
        }
    }
    Ok(())
}

/// Creates `memory.md` from its template. An existing note is never blanked: the
/// empty scaffold is written only when the template is absent and nothing exists.
fn copy_memory_note(
    home: &Path,
    target_dir: &Path,
    policy: &mut Policy,
    log: &Logger,
) -> Result<(), ScaffoldError> {
    let template = home.join("skills/plaesy/assets/memory.template.md");
    let target = target_dir.join("memory.md");
    if file_exists(&target) && policy.is_protected(&target) {
        log.warning("  ⊘ memory.md (your file, left alone)");
        policy.report.record(&target, Outcome::Protected);
    } else if file_exists(&target) && !policy.overwrite {
        log.warning("  ⊘ memory.md (already exists, skipping)");
        policy.report.record(&target, Outcome::Unchanged);
    } else if !file_exists(&template) {
        log.warning("  ! memory.template.md not found, creating empty file");
        match fs::write(&target, b"") {
            Ok(()) => log.info("  ✓ memory.md (empty)"),
            Err(e) => log.warning(&format!("  ✗ memory.md (could not create: {e})")),
        }
    } else {
        match copy_file(&template, &target, policy, log) {
            Ok(_) if !policy.dry_run => log.info("  ✓ memory.md"),
            Ok(_) => {}
            Err(e) => log.warning(&format!("  ✗ memory.md (failed to copy: {e})")),
        }
    }
    Ok(())
}

/// Copies every file directly under `home/<source_sub>` into `target/<label>`,
/// optionally restricted to the given extensions (with leading dots).
///
/// # Errors
///
/// I/O errors creating the destination or reading the source directory.
pub fn copy_flat_dir(
    home: &Path,
    target_dir: &Path,
    source_sub: &str,
    label: &str,
    extensions: &[&str],
    policy: &mut Policy,
    log: &Logger,
) -> Result<(), ScaffoldError> {
    let source = home.join(source_sub);
    if !dir_has_files(&source) {
        log.warning(&format!("Skipping {label} copy - source directory issues"));
        return Ok(());
    }

    log.info(&format!("Copying {label} to .plaesy/{label}/..."));
    let dest = target_dir.join(label);
    fs::create_dir_all(&dest)
        .map_err(|e| ScaffoldError::io(format!("creating {}", dest.display()), e))?;

    let entries = fs::read_dir(&source)
        .map_err(|e| ScaffoldError::io(format!("reading {}", source.display()), e))?;
    let mut copied = 0usize;
    for entry in entries.flatten() {
        if entry.path().is_dir() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        if !extensions.is_empty() && !has_any_extension(&name, extensions) {
            continue;
        }
        match copy_file(&entry.path(), &dest.join(&name), policy, log) {
            Ok(true) => copied += 1,
            Ok(false) => {}
            Err(_) => log.warning(&format!("  ✗ {name} (failed to copy)")),
        }
    }
    if copied > 0 && !policy.dry_run {
        log.info(&format!("  ✓ {label} copied ({copied} files)"));
    }
    log.success(&format!("{} copied", capitalize(label)));
    Ok(())
}

/// Copies the skill templates (`skills/plaesy/assets`, markdown, JSON, YAML).
///
/// # Errors
///
/// As [`copy_flat_dir`].
pub fn copy_templates(
    home: &Path,
    target_dir: &Path,
    policy: &mut Policy,
    log: &Logger,
) -> Result<(), ScaffoldError> {
    copy_flat_dir(
        home,
        target_dir,
        "skills/plaesy/assets",
        "templates",
        &[".md", ".json", ".yaml"],
        policy,
        log,
    )
}

/// Copies the checklists (`checklists/*.md`).
///
/// # Errors
///
/// As [`copy_flat_dir`].
pub fn copy_checklists(
    home: &Path,
    target_dir: &Path,
    policy: &mut Policy,
    log: &Logger,
) -> Result<(), ScaffoldError> {
    copy_flat_dir(
        home,
        target_dir,
        "checklists",
        "checklists",
        &[".md"],
        policy,
        log,
    )
}

fn has_any_extension(name: &str, extensions: &[&str]) -> bool {
    let ext = Path::new(name)
        .extension()
        .map(|e| format!(".{}", e.to_string_lossy().to_lowercase()))
        .unwrap_or_default();
    extensions.iter().any(|want| want.to_lowercase() == ext)
}

fn capitalize(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn quiet() -> Logger {
        Logger::new("test")
    }

    #[test]
    fn copy_skips_existing_by_default_and_overwrites_on_request() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("src.md");
        let dst = dir.path().join("dst.md");
        fs::write(&src, "new").unwrap();
        fs::write(&dst, "old").unwrap();

        let mut skip = Policy::default();
        assert!(!copy_file(&src, &dst, &mut skip, &quiet()).unwrap());
        assert_eq!(fs::read_to_string(&dst).unwrap(), "old");

        let mut over = Policy {
            overwrite: true,
            ..Policy::default()
        };
        assert!(copy_file(&src, &dst, &mut over, &quiet()).unwrap());
        assert_eq!(fs::read_to_string(&dst).unwrap(), "new");
        assert_eq!(over.report.updated.len(), 1);
    }

    #[test]
    fn protected_destination_is_never_overwritten() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("src.md");
        let dst = dir.path().join("memory.md");
        fs::write(&src, "template").unwrap();
        fs::write(&dst, "my notes").unwrap();
        let mut policy = Policy {
            overwrite: true,
            ..Policy::default()
        };
        policy.protected.insert(dst.clone());
        assert!(!copy_file(&src, &dst, &mut policy, &quiet()).unwrap());
        assert_eq!(fs::read_to_string(&dst).unwrap(), "my notes");
        assert_eq!(policy.report.protected.len(), 1);
    }

    #[test]
    fn dry_run_writes_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("src.md");
        let dst = dir.path().join("out/dst.md");
        fs::write(&src, "x").unwrap();
        let mut policy = Policy {
            dry_run: true,
            ..Policy::default()
        };
        assert!(copy_file(&src, &dst, &mut policy, &quiet()).unwrap());
        assert!(!dst.exists());
        assert_eq!(policy.report.created.len(), 1);
    }

    #[test]
    fn extension_filter_is_case_insensitive() {
        assert!(has_any_extension("README.MD", &[".md"]));
        assert!(!has_any_extension("image.png", &[".md", ".json"]));
    }
}
