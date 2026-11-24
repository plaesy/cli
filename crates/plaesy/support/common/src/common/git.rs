//! Git repository queries.
//!
//! Port of `scripts/internal/common/gitpaths.go`. Under Git Bash on Windows,
//! git reports paths as MSYS2 mounts (`/c/Users/...`), which must be mapped to
//! native paths before the filesystem can be used.

use std::path::{Path, PathBuf};
use std::process::Command;

use thiserror::Error;

/// Failure to run a git query.
#[derive(Debug, Error)]
pub enum GitError {
    /// git could not be started or exited non-zero.
    #[error("git {args}: {message}")]
    Failed { args: String, message: String },
    /// An argument was rejected before git ran (empty, or it would be read as an option).
    #[error("invalid argument: {0}")]
    InvalidArgument(String),
}

/// Converts a git-reported path into a native path.
///
/// On Windows, `/c/Users/x` becomes `C:\Users\x`. Other platforms are unchanged.
#[must_use]
pub fn to_native_path(raw: &str) -> PathBuf {
    if cfg!(windows) {
        if let Some(native) = msys_to_windows(raw) {
            return PathBuf::from(native);
        }
        return PathBuf::from(raw.replace('/', "\\"));
    }
    PathBuf::from(raw)
}

fn msys_to_windows(raw: &str) -> Option<String> {
    let bytes = raw.as_bytes();
    if bytes.len() < 3 || bytes[0] != b'/' || bytes[2] != b'/' || !bytes[1].is_ascii_alphabetic() {
        return None;
    }
    let drive = (bytes[1] as char).to_ascii_uppercase();
    Some(format!("{drive}:\\{}", raw[3..].replace('/', "\\")))
}

/// Runs git, optionally inside `repo` (`git -C <repo>`), and returns stdout untrimmed.
///
/// Output is captured, never inherited, so nothing reaches this process's stdout.
/// Stdin is not inherited either, so git cannot open an editor or wait on a prompt.
fn run_git(repo: Option<&Path>, args: &[&str]) -> Result<String, GitError> {
    let mut command = Command::new("git");
    if let Some(dir) = repo {
        command.arg("-C").arg(dir);
    }
    let output = command.args(args).output().map_err(|e| GitError::Failed {
        args: args.join(" "),
        message: e.to_string(),
    })?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_owned();
        // Some failures (for example "nothing to commit") are reported on stdout.
        let message = if !stderr.is_empty() {
            stderr
        } else if !stdout.is_empty() {
            stdout
        } else {
            output.status.to_string()
        };
        return Err(GitError::Failed {
            args: args.join(" "),
            message,
        });
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

fn git_output(args: &[&str]) -> Result<String, GitError> {
    run_git(None, args).map(|out| out.trim().to_owned())
}

/// Returns the top-level directory of the current repository.
///
/// # Errors
///
/// Returns [`GitError`] when the working directory is not inside a repository.
pub fn repo_root() -> Result<PathBuf, GitError> {
    let raw = git_output(&["rev-parse", "--show-toplevel"])?;
    let root = to_native_path(&raw);
    if root.exists() {
        return Ok(root);
    }
    // Git Bash can print a POSIX mount that Windows cannot resolve. The same root
    // is available relative to the working directory, which is a native path.
    if let Some(repaired) = root_from_cwd() {
        return Ok(repaired);
    }
    Ok(root)
}

fn root_from_cwd() -> Option<PathBuf> {
    let cwd = std::env::current_dir().ok()?;
    let cdup = git_output(&["rev-parse", "--show-cdup"]).ok()?;
    let candidate = if cdup.is_empty() { cwd } else { cwd.join(cdup) };
    candidate.exists().then_some(candidate)
}

/// Returns the current branch name.
///
/// Falls back to `git branch --show-current` on an unborn branch, where
/// `rev-parse` has no revision to resolve.
///
/// # Errors
///
/// Returns [`GitError`] when neither form succeeds.
pub fn current_branch() -> Result<String, GitError> {
    match git_output(&["rev-parse", "--abbrev-ref", "HEAD"]) {
        Ok(branch) => Ok(branch),
        Err(_) => git_output(&["branch", "--show-current"]),
    }
}

/// Returns whether `path` is inside a git work tree. Used for non-fatal warnings.
#[must_use]
pub fn is_repo(path: &Path) -> bool {
    Command::new("git")
        .arg("-C")
        .arg(path)
        .args(["rev-parse", "--is-inside-work-tree"])
        .output()
        .is_ok_and(|o| o.status.success())
}

/// A local branch and whether it is checked out.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BranchInfo {
    /// Short branch name, such as `main` or `feature/x`.
    pub name: String,
    /// True for the branch checked out in this work tree.
    pub current: bool,
}

/// One changed or untracked path from `git status`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatusEntry {
    /// Staged state (porcelain index column). `' '` means unchanged, `'?'` untracked.
    pub index: char,
    /// Unstaged state (porcelain work-tree column).
    pub worktree: char,
    /// Path relative to the repository root. For renames, the new path.
    pub path: String,
}

/// Parsed `git status --porcelain=v1 -z --branch`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RepoStatus {
    /// Current branch. `None` when HEAD is detached.
    pub branch: Option<String>,
    /// Changed and untracked paths, in git's order.
    pub entries: Vec<StatusEntry>,
}

impl RepoStatus {
    /// True when no tracked or untracked path differs from HEAD.
    #[must_use]
    pub fn is_clean(&self) -> bool {
        self.entries.is_empty()
    }
}

/// A pull request prepared as plain text. Nothing here touches the network.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrDraft {
    /// Title, trimmed.
    pub title: String,
    /// Markdown body listing the commits in `base..head`.
    pub body: String,
    /// Command to run by hand, for example `gh pr create ... --body-file -`.
    /// Pipe [`PrDraft::body`] to its stdin.
    pub argv: Vec<String>,
}

/// Lists local branches. The checked-out branch has `current` set.
///
/// # Errors
///
/// Returns [`GitError`] when `repo` is not a repository.
pub fn list_branches(repo: &Path) -> Result<Vec<BranchInfo>, GitError> {
    let raw = run_git(
        Some(repo),
        &[
            "for-each-ref",
            "--format=%(HEAD)\t%(refname:short)",
            "refs/heads/",
        ],
    )?;
    Ok(raw
        .lines()
        .filter_map(|line| {
            let (head, name) = line.split_once('\t')?;
            Some(BranchInfo {
                name: name.to_owned(),
                current: head == "*",
            })
        })
        .collect())
}

/// Creates a local branch at HEAD. The current branch is not switched.
///
/// # Errors
///
/// Returns [`GitError::InvalidArgument`] for a name git would refuse, and
/// [`GitError::Failed`] when git itself fails (for example, the branch exists).
pub fn create_branch(repo: &Path, name: &str) -> Result<(), GitError> {
    check_branch_name(repo, name)?;
    run_git(Some(repo), &["branch", name]).map(|_| ())
}

/// Returns the porcelain status of `repo`, with branch and paths parsed.
///
/// # Errors
///
/// Returns [`GitError`] when `repo` is not a repository.
pub fn status(repo: &Path) -> Result<RepoStatus, GitError> {
    let raw = run_git(Some(repo), &["status", "--porcelain=v1", "--branch", "-z"])?;
    Ok(parse_status(&raw))
}

/// Stages `paths` for the next commit (`git add -- <paths>`).
///
/// # Errors
///
/// Returns [`GitError::InvalidArgument`] when `paths` is empty, and
/// [`GitError::Failed`] when git rejects a path.
pub fn stage(repo: &Path, paths: &[&str]) -> Result<(), GitError> {
    if paths.is_empty() {
        return Err(GitError::InvalidArgument("no paths to stage".to_owned()));
    }
    let mut args = vec!["add", "--"];
    args.extend_from_slice(paths);
    run_git(Some(repo), &args).map(|_| ())
}

/// Commits the staged changes with `message` and returns the new HEAD hash.
///
/// Only what is already staged is committed. Hooks run as usual.
///
/// # Errors
///
/// Returns [`GitError::InvalidArgument`] for a blank message, and
/// [`GitError::Failed`] when nothing is staged or git refuses the commit.
pub fn commit(repo: &Path, message: &str) -> Result<String, GitError> {
    if message.trim().is_empty() {
        return Err(GitError::InvalidArgument(
            "commit message is empty".to_owned(),
        ));
    }
    run_git(Some(repo), &["commit", "-m", message])?;
    run_git(Some(repo), &["rev-parse", "HEAD"]).map(|out| out.trim().to_owned())
}

/// Prepares a pull request from `head` into `base` as text, without network access.
///
/// The body lists the non-merge commit subjects in `base..head`, newest first.
///
/// # Errors
///
/// Returns [`GitError::InvalidArgument`] for a blank title or a ref name that
/// starts with `-`, and [`GitError::Failed`] when git cannot resolve the range.
pub fn pr_draft(repo: &Path, base: &str, head: &str, title: &str) -> Result<PrDraft, GitError> {
    check_ref_arg(base)?;
    check_ref_arg(head)?;
    let title = title.trim();
    if title.is_empty() {
        return Err(GitError::InvalidArgument(
            "pull request title is empty".to_owned(),
        ));
    }
    let range = format!("{base}..{head}");
    let log = run_git(
        Some(repo),
        &["log", "--no-merges", "--format=%s", range.as_str(), "--"],
    )?;
    let subjects: Vec<&str> = log
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect();
    let argv: Vec<String> = [
        "gh",
        "pr",
        "create",
        "--base",
        base,
        "--head",
        head,
        "--title",
        title,
        "--body-file",
        "-",
    ]
    .iter()
    .map(|part| (*part).to_owned())
    .collect();
    Ok(PrDraft {
        title: title.to_owned(),
        body: build_pr_body(&subjects),
        argv,
    })
}

/// Rejects an empty branch name or one that git would read as an option.
fn check_ref_arg(name: &str) -> Result<(), GitError> {
    if name.is_empty() || name.starts_with('-') {
        return Err(GitError::InvalidArgument(format!("ref name {name:?}")));
    }
    Ok(())
}

/// Checks a new branch name with `git check-ref-format` before it reaches `git branch`.
fn check_branch_name(repo: &Path, name: &str) -> Result<(), GitError> {
    check_ref_arg(name)?;
    let refname = format!("refs/heads/{name}");
    run_git(Some(repo), &["check-ref-format", refname.as_str()])
        .map(|_| ())
        .map_err(|_| GitError::InvalidArgument(format!("branch name {name:?}")))
}

/// Parses NUL-separated `git status --porcelain=v1 -z --branch` output.
///
/// The `-z` form keeps paths unquoted. A rename or copy is followed by its
/// source path as a separate token, which is skipped.
fn parse_status(raw: &str) -> RepoStatus {
    let mut status = RepoStatus::default();
    let mut tokens = raw.split('\0');
    while let Some(token) = tokens.next() {
        if token.is_empty() {
            continue;
        }
        if let Some(header) = token.strip_prefix("## ") {
            status.branch = parse_branch_header(header);
            continue;
        }
        let mut codes = token.chars();
        let (Some(index), Some(worktree), Some(' ')) = (codes.next(), codes.next(), codes.next())
        else {
            continue;
        };
        let Some(path) = token.get(3..) else {
            continue;
        };
        status.entries.push(StatusEntry {
            index,
            worktree,
            path: path.to_owned(),
        });
        if matches!(index, 'R' | 'C') {
            tokens.next();
        }
    }
    status
}

/// Extracts the branch name from a `## ...` header. `None` means detached HEAD.
fn parse_branch_header(header: &str) -> Option<String> {
    if header.starts_with("HEAD (no branch)") {
        return None;
    }
    let rest = header.strip_prefix("No commits yet on ").unwrap_or(header);
    let before_upstream = rest.split("...").next().unwrap_or(rest);
    let name = before_upstream.split(' ').next().unwrap_or(before_upstream);
    if name.is_empty() {
        None
    } else {
        Some(name.to_owned())
    }
}

/// Builds the PR body text from commit subjects.
fn build_pr_body(subjects: &[&str]) -> String {
    let mut body = String::from("## Perubahan\n\n");
    if subjects.is_empty() {
        body.push_str("- (tidak ada commit di rentang ini)\n");
    }
    for subject in subjects {
        body.push_str("- ");
        body.push_str(subject);
        body.push('\n');
    }
    body
}

#[cfg(test)]
#[path = "git_test.rs"]
mod git_test;
