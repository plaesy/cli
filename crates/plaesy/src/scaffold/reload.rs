//! `plaesy reload`: refreshes the generated parts of an existing `.plaesy/` tree.
//!
//! `init` skips anything that already exists, so a project that was initialised
//! before an instruction changed keeps the stale copy. `reload` overwrites the
//! generated files and keeps user state: `memory.md`, `context.md`, and
//! `state.json` are created when missing but never overwritten.
//! Port of `scripts/internal/scaffold/reload.go` (Go).

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::assets;
use crate::common::log::Logger;

use super::config as scaffold_config;
use super::copy::{copy_checklists, copy_instructions, copy_templates};
use super::home::{find_home, is_home};
use super::init::normalize_ai_platform;
use super::mcp::install_mcp_server;
use super::platform::{copy_agents, setup_platform_config};
use super::prune::{apply as apply_prune, find_candidates, prune_roots};
use super::structure::{create_loop_state, create_memory_structure};
use super::{Policy, ScaffoldError};

/// Module path of this repository's `scripts/go.mod`.
const REPOSITORY_MODULE: &str = "github.com/plaesy/spec-kit";

/// Directories reload makes sure exist before copying.
const GENERATED_DIRS: [&str; 4] = ["instructions", "templates", "checklists", "roles"];

/// Options for [`reload`].
#[derive(Debug, Clone, Default)]
pub struct Options {
    /// Project to refresh. Empty means the working directory.
    pub target_dir: PathBuf,
    /// Also refresh the platform's core file, prompts, and roles. Empty or `none` skips them.
    pub ai_platform: String,
    /// Source home. Overrides the default.
    pub plaesy_home: Option<PathBuf>,
    /// Read the sources embedded in this binary when no home is given.
    pub use_embedded: bool,
    /// Report without writing.
    pub dry_run: bool,
    /// List orphans in the tool-owned trees.
    pub prune: bool,
    /// Delete the listed orphans. Requires `prune`.
    pub prune_apply: bool,
}

/// Per-file outcome of a reload.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Report {
    pub dry_run: bool,
    pub created: Vec<String>,
    pub updated: Vec<String>,
    pub unchanged: Vec<String>,
    pub protected: Vec<String>,
    pub failed: Vec<String>,
    /// Owned-tree files with no source.
    pub stale: Vec<String>,
    /// Stale files actually deleted.
    pub pruned: Vec<String>,
}

impl Report {
    /// Reports whether reload altered anything, or would.
    #[must_use]
    pub fn changed(&self) -> bool {
        !self.created.is_empty() || !self.updated.is_empty() || !self.pruned.is_empty()
    }

    /// One-line summary of the counts.
    #[must_use]
    pub fn summary(&self) -> String {
        let prefix = if self.dry_run { "Would " } else { "" };
        let mut parts = vec![
            format!("{prefix}{} created", self.created.len()),
            format!("{} updated", self.updated.len()),
            format!("{} unchanged", self.unchanged.len()),
        ];
        if !self.protected.is_empty() {
            parts.push(format!("{} preserved (your data)", self.protected.len()));
        }
        if !self.stale.is_empty() {
            parts.push(format!("{} stale (no source)", self.stale.len()));
        }
        if !self.pruned.is_empty() {
            parts.push(format!("{} pruned", self.pruned.len()));
        }
        let mut out = parts.join(", ");
        if !self.failed.is_empty() {
            out.push_str(&format!(", {} FAILED", self.failed.len()));
        }
        out
    }
}

/// A Plaesy home extracted from the embedded sources. Removed when dropped.
struct TempHome {
    path: PathBuf,
}

impl Drop for TempHome {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

/// Where the sources come from.
enum Source {
    Embedded(TempHome),
    Path(PathBuf),
}

impl Source {
    fn path(&self) -> &Path {
        match self {
            Self::Embedded(temp) => &temp.path,
            Self::Path(path) => path,
        }
    }
}

/// Extracts the embedded sources into a fresh temporary directory.
fn extract_embedded(log: &Logger) -> Result<TempHome, ScaffoldError> {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let path = env::temp_dir().join(format!("plaesy-embedded-{}-{stamp}", std::process::id()));
    assets::extract(&path, log).map_err(|e| ScaffoldError::Home(e.to_string()))?;
    let temp = TempHome { path };
    if !is_home(&temp.path) {
        return Err(ScaffoldError::Home(
            "the embedded tree is not a usable Plaesy home".into(),
        ));
    }
    Ok(temp)
}

fn resolve_source(opts: &Options, log: &Logger) -> Result<Source, ScaffoldError> {
    if opts.plaesy_home.is_none() && opts.use_embedded {
        match extract_embedded(log) {
            Ok(temp) => return Ok(Source::Embedded(temp)),
            Err(e) => log.warning(&format!("{e}; falling back to the installed Plaesy home")),
        }
    }
    find_home(opts.plaesy_home.as_deref(), log).map(Source::Path)
}

/// Reports whether `home/scripts/go.mod` declares this repository's module.
fn is_inside_repository(home: &Path) -> bool {
    let Ok(text) = fs::read_to_string(home.join("scripts").join("go.mod")) else {
        return false;
    };
    text.lines()
        .map(|line| line.split_whitespace().collect::<Vec<_>>())
        .find(|fields| fields.len() == 2 && fields[0] == "module")
        .is_some_and(|fields| fields[1] == REPOSITORY_MODULE)
}

/// `memory.md`, `context.md`, and `state.json` hold user state. They are created
/// when missing but never overwritten.
fn protected_paths(base: &Path) -> std::collections::HashSet<PathBuf> {
    ["memory.md", "context.md", "state.json"]
        .iter()
        .map(|name| base.join(name))
        .collect()
}

/// Refreshes `<target>/.plaesy` from the sources.
///
/// # Errors
///
/// [`ScaffoldError::Config`] when the tree is absent or the platform is unknown, and
/// I/O failures.
pub fn reload(opts: &Options, log: &Logger) -> Result<Report, ScaffoldError> {
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

    let source = resolve_source(opts, log)?;
    let home = source.path().to_path_buf();
    let cfg = scaffold_config::load(&home)?;

    let base = target.join(&cfg.plaesy.base_directory);
    if !base.is_dir() {
        return Err(ScaffoldError::Config(if base.exists() {
            format!(
                "{} is not a directory: {}",
                cfg.plaesy.base_directory,
                base.display()
            )
        } else {
            format!(
                "no {} directory in {} — nothing to reload; run `plaesy init` first",
                cfg.plaesy.base_directory,
                target.display()
            )
        }));
    }

    let mut policy = Policy {
        overwrite: true,
        dry_run: opts.dry_run,
        protected: protected_paths(&base),
        report: super::FileReport {
            base: Some(base.clone()),
            ..super::FileReport::default()
        },
    };

    if opts.dry_run {
        log.info("Dry run — no files will be written.");
    }
    let source_label = if matches!(source, Source::Embedded(_)) {
        "embedded in this binary".to_owned()
    } else {
        home.display().to_string()
    };
    log.info(&format!("source: {source_label}"));
    if !matches!(source, Source::Embedded(_)) && !is_inside_repository(&home) {
        log.warning(&format!(
            "source {} is not inside the repository {REPOSITORY_MODULE} (no scripts/go.mod with that module path); pass --plaesy-home or set PLAESY_HOME to the repository you intend",
            home.display()
        ));
    }
    log.info(&format!(
        "Refreshing generated files in {}/ from {source_label}...",
        cfg.plaesy.base_directory
    ));

    for dir in GENERATED_DIRS {
        let path = base.join(dir);
        fs::create_dir_all(&path)
            .map_err(|e| ScaffoldError::io(format!("creating {}", path.display()), e))?;
    }

    copy_instructions(&home, &base, &mut policy, log)?;
    copy_templates(&home, &base, &mut policy, log)?;
    copy_checklists(&home, &base, &mut policy, log)?;
    // The roles mirror refreshes on every run, because the instructions delegate to it.
    copy_agents(&home, &base.join("roles"), "", &mut policy, log)?;

    create_memory_structure(&home, &base, &mut policy, log)?;
    create_loop_state(&home, &base, &mut policy, log)?;
    install_mcp_server(&target, &policy, log)?;

    let platform = opts.ai_platform.trim();
    let mut platform_id = String::new();
    if !platform.is_empty() && platform != "none" {
        platform_id = normalize_ai_platform(platform, Some(&cfg));
        if !cfg.has_platform(&platform_id) {
            return Err(ScaffoldError::InvalidPlatform {
                platform: opts.ai_platform.clone(),
                available: cfg.platform_ids().into_iter().map(str::to_owned).collect(),
            });
        }
        log.success(&format!("Selected: {}", cfg.display_name(&platform_id)));
        setup_platform_config(&home, &target, &platform_id, &cfg, &mut policy, log)?;
    } else {
        log.info("Skipping platform files — pass --ai <platform> to refresh the core file, prompts and roles.");
    }

    let mut report = Report {
        dry_run: opts.dry_run,
        ..Report::default()
    };
    if opts.prune {
        // Candidates come from the same sources the mirror was just written from,
        // so an unreadable source surfaces as a failed copy rather than a deletion.
        let roots = prune_roots(&home, &target, &base, &platform_id, &cfg);
        report.stale = find_candidates(&roots, &target, log);
        if !report.stale.is_empty() && opts.prune_apply && !opts.dry_run {
            report.pruned = apply_prune(&roots, &target, &report.stale, log);
        }
    }

    let recorded = &policy.report;
    report.created = recorded.created.clone();
    report.updated = recorded.updated.clone();
    report.unchanged = recorded.unchanged.clone();
    report.protected = recorded.protected.clone();
    report.failed = recorded.failed.clone();
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summary_names_each_nonzero_category() {
        let report = Report {
            created: vec!["a".into()],
            protected: vec!["memory.md".into()],
            failed: vec!["x".into()],
            ..Report::default()
        };
        assert_eq!(
            report.summary(),
            "1 created, 0 updated, 0 unchanged, 1 preserved (your data), 1 FAILED"
        );
        assert!(report.changed());
    }

    #[test]
    fn dry_run_summary_says_would() {
        let report = Report {
            dry_run: true,
            ..Report::default()
        };
        assert!(report.summary().starts_with("Would 0 created"));
        assert!(!report.changed());
    }

    #[test]
    fn go_mod_marks_the_repository() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir_all(dir.path().join("scripts")).unwrap();
        fs::write(
            dir.path().join("scripts/go.mod"),
            "module github.com/plaesy/spec-kit\n\ngo 1.26\n",
        )
        .unwrap();
        assert!(is_inside_repository(dir.path()));
        assert!(!is_inside_repository(&dir.path().join("elsewhere")));
    }

    #[test]
    fn reload_refuses_a_target_without_the_plaesy_directory() {
        let project = tempfile::tempdir().unwrap();
        let options = Options {
            target_dir: project.path().to_path_buf(),
            use_embedded: true,
            ..Options::default()
        };
        let err = reload(&options, &Logger::new("t")).unwrap_err();
        assert!(err.to_string().contains("run `plaesy init` first"), "{err}");
    }
}
