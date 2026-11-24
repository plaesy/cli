//! Unit tests for `tests` split out of `git.rs`.
//! Kept as a separate file so they are easy to find; included via `#[path]`.

use super::*;

#[test]
fn msys_drive_prefix_maps_to_a_drive_letter() {
    assert_eq!(
        msys_to_windows("/c/Users/me/repo").as_deref(),
        Some("C:\\Users\\me\\repo")
    );
    assert_eq!(msys_to_windows("/D/x").as_deref(), Some("D:\\x"));
}

#[test]
fn non_drive_paths_are_not_rewritten() {
    assert_eq!(msys_to_windows("/tmp/repo"), None);
    assert_eq!(msys_to_windows("C:\\already\\native"), None);
    assert_eq!(msys_to_windows("/c"), None);
}

/// Runs git in `dir` and panics with stderr when it fails. Test setup only.
fn git_ok(dir: &Path, args: &[&str]) {
    let out = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .output()
        .expect("git must be on PATH for these tests");
    assert!(
        out.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

/// Creates a repository on `main` with a local identity and signing off.
fn init_repo() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("temp dir");
    git_ok(dir.path(), &["init", "-q", "-b", "main"]);
    git_ok(dir.path(), &["config", "user.name", "Tes"]);
    git_ok(dir.path(), &["config", "user.email", "tes@example.invalid"]);
    git_ok(dir.path(), &["config", "commit.gpgsign", "false"]);
    dir
}

/// Writes `name` and commits it with `message`, so the repository has history.
fn commit_file(dir: &Path, name: &str, message: &str) {
    std::fs::write(dir.join(name), message).expect("write test file");
    git_ok(dir, &["add", "--", name]);
    git_ok(dir, &["commit", "-q", "-m", message]);
}

#[test]
fn create_branch_adds_branch_without_switching() {
    let dir = init_repo();
    commit_file(dir.path(), "a.txt", "awal");
    create_branch(dir.path(), "fitur/baru").expect("create branch");
    let branches = list_branches(dir.path()).expect("list branches");
    let current: Vec<&str> = branches
        .iter()
        .filter(|b| b.current)
        .map(|b| b.name.as_str())
        .collect();
    assert_eq!(current, ["main"]);
    assert!(branches
        .iter()
        .any(|b| b.name == "fitur/baru" && !b.current));
}

#[test]
fn create_branch_rejects_invalid_or_option_like_names() {
    let dir = init_repo();
    commit_file(dir.path(), "a.txt", "awal");
    for name in ["", "-x", "a..b", "dua kata", "a~b"] {
        assert!(
            matches!(
                create_branch(dir.path(), name),
                Err(GitError::InvalidArgument(_))
            ),
            "expected InvalidArgument for {name:?}"
        );
    }
}

#[test]
fn commit_returns_new_head_after_staging() {
    let dir = init_repo();
    std::fs::write(dir.path().join("b.txt"), "isi").expect("write");
    stage(dir.path(), &["b.txt"]).expect("stage");
    let hash = commit(dir.path(), "tambah b").expect("commit");
    assert_eq!(hash.len(), 40);
    assert!(hash.chars().all(|c| c.is_ascii_hexdigit()));
    assert!(status(dir.path()).expect("status").is_clean());
}

#[test]
fn commit_fails_without_staged_changes_and_rejects_blank_message() {
    let dir = init_repo();
    commit_file(dir.path(), "a.txt", "awal");
    assert!(matches!(
        commit(dir.path(), "kosong"),
        Err(GitError::Failed { .. })
    ));
    assert!(matches!(
        commit(dir.path(), "   "),
        Err(GitError::InvalidArgument(_))
    ));
}

#[test]
fn status_reports_branch_untracked_and_staged_paths() {
    let dir = init_repo();
    std::fs::write(dir.path().join("c.txt"), "x").expect("write");
    let before = status(dir.path()).expect("status");
    assert_eq!(before.branch.as_deref(), Some("main"));
    assert_eq!(
        before.entries,
        vec![StatusEntry {
            index: '?',
            worktree: '?',
            path: "c.txt".to_owned(),
        }]
    );
    stage(dir.path(), &["c.txt"]).expect("stage");
    let after = status(dir.path()).expect("status");
    assert_eq!(after.entries[0].index, 'A');
    assert_eq!(after.entries[0].worktree, ' ');
}

#[test]
fn status_fails_outside_a_repository() {
    let dir = tempfile::tempdir().expect("temp dir");
    assert!(matches!(status(dir.path()), Err(GitError::Failed { .. })));
}

#[test]
fn parse_status_handles_rename_and_detached_head() {
    let st = parse_status("## HEAD (no branch)\0R  new.txt\0old.txt\0 M edit.txt\0");
    assert_eq!(st.branch, None);
    assert_eq!(
        st.entries,
        vec![
            StatusEntry {
                index: 'R',
                worktree: ' ',
                path: "new.txt".to_owned(),
            },
            StatusEntry {
                index: ' ',
                worktree: 'M',
                path: "edit.txt".to_owned(),
            },
        ]
    );
}

#[test]
fn branch_header_variants_are_parsed() {
    assert_eq!(
        parse_branch_header("main...origin/main [ahead 2]").as_deref(),
        Some("main")
    );
    assert_eq!(
        parse_branch_header("No commits yet on trunk").as_deref(),
        Some("trunk")
    );
    assert_eq!(parse_branch_header("HEAD (no branch)"), None);
}

#[test]
fn pr_draft_lists_commits_and_builds_gh_command() {
    let dir = init_repo();
    commit_file(dir.path(), "a.txt", "dasar");
    create_branch(dir.path(), "fitur").expect("create branch");
    git_ok(dir.path(), &["checkout", "-q", "fitur"]);
    commit_file(dir.path(), "b.txt", "tambah b");
    commit_file(dir.path(), "c.txt", "tambah c");

    let draft = pr_draft(dir.path(), "main", "fitur", "  Tambah B dan C  ").expect("draft");
    assert_eq!(draft.title, "Tambah B dan C");
    assert!(draft.body.contains("- tambah b\n"));
    assert!(draft.body.contains("- tambah c\n"));
    assert!(!draft.body.contains("dasar"));
    assert_eq!(draft.argv[0], "gh");
    assert_eq!(draft.argv[1], "pr");
    assert_eq!(draft.argv[2], "create");
    assert!(draft
        .argv
        .windows(2)
        .any(|w| w[0] == "--head" && w[1] == "fitur"));
}

#[test]
fn pr_draft_rejects_option_like_refs_and_blank_title() {
    let dir = init_repo();
    commit_file(dir.path(), "a.txt", "awal");
    assert!(matches!(
        pr_draft(dir.path(), "-x", "main", "Judul"),
        Err(GitError::InvalidArgument(_))
    ));
    assert!(matches!(
        pr_draft(dir.path(), "main", "main", "  "),
        Err(GitError::InvalidArgument(_))
    ));
}

#[test]
fn pr_body_states_when_range_is_empty() {
    assert!(build_pr_body(&[]).contains("tidak ada commit"));
    assert!(build_pr_body(&["satu"]).contains("- satu\n"));
}
