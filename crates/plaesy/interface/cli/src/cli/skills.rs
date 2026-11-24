//! `plaesy skills`: list and copy skills between agent providers (Claude, Codex, agents, Hermes,
//! and Plaesy's own store).
//!
//! The Plaesy store defaults to `~/.plaesy/skills`. Override it with `--dir`, `PLAESY_SKILLS_DIR`,
//! or `skills.dir` in `~/.plaesy/settings.json`. Copying never overwrites: a skill is copied
//! only when the target does not have it. `sync` is a dry run unless `--apply` is passed.

use std::fs;
use std::path::{Path, PathBuf};

use super::CliResult;

/// External providers and their skills directory, relative to the home directory.
const PROVIDERS: &[(&str, &str)] = &[
    ("claude", ".claude/skills"),
    ("codex", ".codex/skills"),
    ("agents", ".agents/skills"),
    ("hermes", ".hermes/skills"),
];

#[derive(Debug, clap::Args)]
pub struct Args {
    #[command(subcommand)]
    pub command: Option<SkillsCommand>,
}

#[derive(Debug, clap::Subcommand)]
pub enum SkillsCommand {
    /// List skills per provider
    List {
        /// Plaesy skills directory (overrides PLAESY_SKILLS_DIR)
        #[arg(long)]
        dir: Option<PathBuf>,
    },
    /// Copy skills that exist in one provider but not in another
    Sync {
        /// Source provider: claude, codex, agents, or hermes
        #[arg(long)]
        from: String,
        /// Target provider: claude, codex, agents, or hermes
        #[arg(long)]
        to: String,
        /// Copy the skills; without this flag only the plan is printed
        #[arg(long)]
        apply: bool,
        /// Plaesy skills directory (overrides PLAESY_SKILLS_DIR)
        #[arg(long)]
        dir: Option<PathBuf>,
    },
}

pub fn run(args: &Args) -> CliResult {
    let Some(command) = &args.command else {
        return super::print_group_help("skills");
    };
    let home = dirs::home_dir().ok_or("home directory is not available")?;
    match command {
        SkillsCommand::List { dir } => list(&home, &plaesy_dir(&home, dir.as_deref())),
        SkillsCommand::Sync { from, to, apply, dir } => {
            sync(&home, &plaesy_dir(&home, dir.as_deref()), from, to, *apply)
        }
    }
}

/// Plaesy's store, first match wins: `--dir`, `PLAESY_SKILLS_DIR`, `skills.dir` in
/// `~/.plaesy/settings.json`, then `~/.plaesy/skills`.
fn plaesy_dir(home: &Path, flag: Option<&Path>) -> PathBuf {
    if let Some(dir) = flag {
        return dir.to_path_buf();
    }
    if let Some(dir) = std::env::var_os("PLAESY_SKILLS_DIR").filter(|d| !d.is_empty()) {
        return PathBuf::from(dir);
    }
    if let Some(dir) = settings_skills_dir(&home.join(".plaesy").join("settings.json")) {
        return dir;
    }
    home.join(".plaesy").join("skills")
}

/// Reads `skills.dir` from a settings file. A missing or unreadable file falls through to the
/// default, so skill commands still work without a settings file.
fn settings_skills_dir(path: &Path) -> Option<PathBuf> {
    let raw = fs::read_to_string(path).ok()?;
    let value: serde_json::Value = serde_json::from_str(&raw).ok()?;
    let dir = value.get("skills")?.get("dir")?.as_str()?;
    (!dir.is_empty()).then(|| PathBuf::from(dir))
}

fn provider_dir(home: &Path, plaesy: &Path, name: &str) -> Result<PathBuf, Box<dyn std::error::Error>> {
    if name == "plaesy" {
        return Ok(plaesy.to_path_buf());
    }
    let (_, rel) = PROVIDERS
        .iter()
        .find(|(provider, _)| *provider == name)
        .ok_or_else(|| format!("unknown provider `{name}`; use one of: plaesy, claude, codex, agents, hermes"))?;
    Ok(home.join(rel))
}

/// A skill is a directory entry. Symlinks are reported but never copied, so a link into another
/// provider cannot be duplicated by accident.
struct Entry {
    name: String,
    is_link: bool,
}

fn entries(dir: &Path) -> Result<Vec<Entry>, Box<dyn std::error::Error>> {
    if !dir.is_dir() {
        return Ok(Vec::new());
    }
    let mut out = Vec::new();
    for item in fs::read_dir(dir)? {
        let item = item?;
        let meta = fs::symlink_metadata(item.path())?;
        let is_link = meta.file_type().is_symlink();
        if !is_link && !meta.is_dir() {
            continue;
        }
        out.push(Entry {
            name: item.file_name().to_string_lossy().into_owned(),
            is_link,
        });
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(out)
}

fn list(home: &Path, plaesy: &Path) -> CliResult {
    let mut all: Vec<(&str, PathBuf)> = vec![("plaesy", plaesy.to_path_buf())];
    all.extend(PROVIDERS.iter().map(|(name, rel)| (*name, home.join(rel))));
    for (name, dir) in all {
        let items = entries(&dir)?;
        println!("{name} ({}): {} entries", dir.display(), items.len());
        for item in items {
            let marker = if item.is_link { " (link)" } else { "" };
            println!("  {}{marker}", item.name);
        }
    }
    Ok(())
}

fn sync(home: &Path, plaesy: &Path, from: &str, to: &str, apply: bool) -> CliResult {
    if from == to {
        return Err("--from and --to must differ".into());
    }
    let source = provider_dir(home, plaesy, from)?;
    let target = provider_dir(home, plaesy, to)?;
    let target_names: Vec<String> = entries(&target)?.into_iter().map(|e| e.name).collect();

    let mut planned = Vec::new();
    let mut skipped_links = Vec::new();
    for item in entries(&source)? {
        if target_names.contains(&item.name) {
            continue;
        }
        if item.is_link {
            skipped_links.push(item.name);
        } else {
            planned.push(item.name);
        }
    }

    let mode = if apply { "apply" } else { "dry run" };
    println!("sync {from} -> {to} ({mode}): {} to copy", planned.len());
    for name in &planned {
        println!("  copy {name}");
    }
    for name in &skipped_links {
        println!("  skip {name} (symlink)");
    }
    if !apply {
        println!("re-run with --apply to copy");
        return Ok(());
    }

    fs::create_dir_all(&target)?;
    for name in &planned {
        copy_dir(&source.join(name), &target.join(name))?;
    }
    println!("copied {} skill(s)", planned.len());
    Ok(())
}

fn copy_dir(from: &Path, to: &Path) -> Result<(), Box<dyn std::error::Error>> {
    fs::create_dir_all(to)?;
    for item in fs::read_dir(from)? {
        let item = item?;
        let meta = fs::symlink_metadata(item.path())?;
        let dest = to.join(item.file_name());
        if meta.file_type().is_symlink() {
            continue;
        }
        if meta.is_dir() {
            copy_dir(&item.path(), &dest)?;
        } else {
            fs::copy(item.path(), dest)?;
        }
    }
    Ok(())
}
