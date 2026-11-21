//! Unit tests for `tests` split out of `filters.rs`.
//! Kept as a separate file so they are easy to find; included via `#[path]`.

use super::*;

fn lines(text: &str) -> Vec<String> {
    text.lines().map(str::to_owned).collect()
}

#[test]
fn git_diff_keeps_changes_and_headers_only() {
    let input = lines("diff --git a/x b/x\nindex 1..2\n@@ -1 +1 @@\n context\n-old\n+new");
    let out = git_diff(&input).unwrap();
    assert_eq!(out, ["diff --git a/x b/x", "@@ -1 +1 @@", "-old", "+new"]);
}

#[test]
fn git_diff_refuses_input_that_is_not_a_diff() {
    assert_eq!(git_diff(&lines("hello")), Err(NotRecognised));
}

#[test]
fn git_log_keeps_one_line_per_commit() {
    let input = lines(
            "commit abcdef1234 (HEAD -> main)\nAuthor: me\nDate: today\n\n    Fix the parser\n    body text",
        );
    let out = git_log(&input).unwrap();
    assert_eq!(out, ["commit abcdef1 (HEAD -> main)", "    Fix the parser"]);
}

#[test]
fn grep_collapses_separators() {
    let out = grep(&lines("a.rs:1: x\n--\n--\n\nb.rs:2: y")).unwrap();
    assert_eq!(out, ["a.rs:1: x", "--", "b.rs:2: y"]);
}

#[test]
fn detection_uses_the_program_and_git_subcommand() {
    let args = vec!["git".to_owned(), "status".to_owned()];
    assert_eq!(detect(&args).map(|(name, _)| name), Some("git-status"));
    assert!(detect(&["make".to_owned()]).is_none());
}
