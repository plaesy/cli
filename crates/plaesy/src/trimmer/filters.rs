//! Per-command filters that restructure output before the generic Layer 1 pass.
//!
//! A filter returns an error when its input has none of the structure it parses.
//! The caller then keeps the raw lines, and a panic is treated the same way.

use std::path::Path;

/// Why a filter gave up on its input.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("input not recognised")]
pub struct NotRecognised;

/// A filter over output lines.
pub type Filter = fn(&[String]) -> Result<Vec<String>, NotRecognised>;

/// Chooses a filter from the command's first word and, for git, its subcommand.
/// Returns the filter's name, which is used in the stats layer label.
#[must_use]
pub fn detect(args: &[String]) -> Option<(&'static str, Filter)> {
    let program = Path::new(args.first()?)
        .file_stem()?
        .to_string_lossy()
        .into_owned();
    match program.as_str() {
        "git" => match args.get(1)?.as_str() {
            "diff" => Some(("git-diff", git_diff)),
            "status" => Some(("git-status", git_status)),
            "log" => Some(("git-log", git_log)),
            _ => None,
        },
        "grep" | "rg" => Some(("grep", grep)),
        "ls" | "find" | "tree" => Some(("listing", listing)),
        _ => None,
    }
}

/// Keeps file headers, hunk headers, and every +/- line. Drops index lines and context lines.
pub fn git_diff(lines: &[String]) -> Result<Vec<String>, NotRecognised> {
    if !lines.is_empty() && !lines.iter().any(|l| l.starts_with("diff --git")) {
        return Err(NotRecognised);
    }
    Ok(lines
        .iter()
        .filter(|l| !l.starts_with("index ") && !l.starts_with(' '))
        .cloned()
        .collect())
}

/// Drops hint lines and blank lines. State lines are kept as they are.
pub fn git_status(lines: &[String]) -> Result<Vec<String>, NotRecognised> {
    Ok(lines
        .iter()
        .filter(|l| {
            let t = l.trim();
            !t.is_empty() && !t.starts_with("(use ")
        })
        .cloned()
        .collect())
}

/// One line per commit (short sha and decoration), then its first non-empty message line.
pub fn git_log(lines: &[String]) -> Result<Vec<String>, NotRecognised> {
    if !lines.is_empty() && !lines.iter().any(|l| l.starts_with("commit ")) {
        return Err(NotRecognised);
    }
    let mut out = Vec::new();
    let mut awaiting_subject = false;
    for line in lines {
        if let Some(rest) = line.strip_prefix("commit ") {
            let mut fields = rest.split_whitespace();
            let sha = fields.next().unwrap_or("");
            let sha: String = sha.chars().take(7).collect();
            let decoration: Vec<&str> = fields.collect();
            let mut head = format!("commit {sha}");
            if !decoration.is_empty() {
                head.push(' ');
                head.push_str(&decoration.join(" "));
            }
            out.push(head);
            awaiting_subject = true;
        } else if awaiting_subject && !line.trim().is_empty() && !is_header(line) {
            out.push(format!("    {}", line.trim()));
            awaiting_subject = false;
        }
    }
    Ok(out)
}

/// Drops blank lines and collapses runs of `--` separators. Match lines are kept exactly,
/// so file:line references stay correct.
pub fn grep(lines: &[String]) -> Result<Vec<String>, NotRecognised> {
    let mut out: Vec<String> = Vec::new();
    for line in lines {
        if line.trim().is_empty() {
            continue;
        }
        if line == "--" && out.last().is_some_and(|l| l == "--") {
            continue;
        }
        out.push(line.clone());
    }
    Ok(out)
}

/// Drops the `total N` line of `ls -l` and blank lines.
pub fn listing(lines: &[String]) -> Result<Vec<String>, NotRecognised> {
    Ok(lines
        .iter()
        .filter(|l| {
            let t = l.trim();
            !t.is_empty() && !t.starts_with("total ")
        })
        .cloned()
        .collect())
}

/// Commit header lines that are not the message: author, date, and merge parents.
fn is_header(line: &str) -> bool {
    ["Author:", "Date:", "Merge:"]
        .iter()
        .any(|prefix| line.starts_with(prefix))
}

#[cfg(test)]
mod tests {
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
        let input = lines("commit abcdef1234 (HEAD -> main)\nAuthor: me\nDate: today\n\n    Fix the parser\n    body text");
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
}
