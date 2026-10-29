//! Layer 1: compresses command output. Output is deduplicated, filtered, and
//! truncated to the rule's line cap. Test output keeps only failures and the summary.

use std::fs;
use std::io::{self, Write};
use std::path::Path;
use std::process::Command;

use thiserror::Error;

use super::filters;
use super::rules::{Layer1Rule, Rules, DEFAULT_MAX_LINES};
use super::stats;

/// Errors from running a command through Layer 1.
#[derive(Debug, Error)]
pub enum RunError {
    #[error("no command given")]
    NoCommand,
    #[error("cannot start {program}: {source}")]
    Spawn {
        program: String,
        #[source]
        source: io::Error,
    },
    #[error("stats: {0}")]
    Stats(#[source] io::Error),
}

/// Result of a Layer 1 run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunOutcome {
    pub output: String,
    pub before: usize,
    pub after: usize,
    /// Exit status line when the command failed. The output is still returned.
    pub failure: Option<String>,
}

/// The rule for a command line: the longest key that prefixes it, else `_default`, else built-in.
#[must_use]
pub fn rule_for(rules: &Rules, command: &str) -> Layer1Rule {
    let longest = rules
        .layer1_commands
        .keys()
        .filter(|key| key.as_str() != "_default" && command.starts_with(key.as_str()))
        .max_by_key(|key| key.len());
    longest
        .and_then(|key| rules.layer1_commands.get(key))
        .or_else(|| rules.layer1_commands.get("_default"))
        .cloned()
        .unwrap_or_default()
}

/// Compresses lines: optional dedupe, then a head and tail cut at `max_lines`, or
/// the failure lines and summary in `test-results` mode.
#[must_use]
pub fn compress_lines(lines: &[String], max_lines: usize, dedupe: bool, mode: &str) -> Vec<String> {
    if mode == "test-results" {
        return compress_test_results(lines);
    }
    let collapsed: Vec<String> = if dedupe {
        dedupe_runs(lines)
    } else {
        lines.to_vec()
    };
    let total = collapsed.len();
    if total <= max_lines {
        return collapsed;
    }
    let head = max_lines.div_ceil(2);
    let tail = max_lines - head;
    let omitted = total - head - tail;
    let mut out = Vec::with_capacity(max_lines + 1);
    out.extend_from_slice(&collapsed[..head]);
    out.push(format!("... {omitted} lines omitted ..."));
    out.extend_from_slice(&collapsed[total - tail..]);
    out
}

fn dedupe_runs(lines: &[String]) -> Vec<String> {
    let mut out = Vec::new();
    let mut iter = lines.iter();
    let Some(first) = iter.next() else {
        return out;
    };
    let (mut prev, mut count) = (first.as_str(), 1usize);
    for line in iter {
        if line == prev {
            count += 1;
        } else {
            out.push(collapse(prev, count));
            prev = line;
            count = 1;
        }
    }
    out.push(collapse(prev, count));
    out
}

fn collapse(line: &str, count: usize) -> String {
    if count > 1 {
        format!("{line} (x{count})")
    } else {
        line.to_owned()
    }
}

fn compress_test_results(lines: &[String]) -> Vec<String> {
    let fail = |l: &str| {
        let lower = l.to_lowercase();
        lower.contains("fail") || lower.contains("error") || l.contains('✗') || l.contains('✘')
    };
    let done = |l: &str| {
        let lower = l.to_lowercase();
        lower.contains("passed")
            || lower.contains("failed")
            || lower.contains("tests run")
            || lower.contains("test run")
            || lower.contains("ok.")
    };
    let mut kept: Vec<String> = lines.iter().filter(|l| fail(l)).cloned().collect();
    let tail_start = lines.len().saturating_sub(5);
    kept.extend(lines[tail_start..].iter().filter(|l| done(l)).cloned());
    if kept.is_empty() {
        return vec!["(no failures - all tests passed, output suppressed)".into()];
    }
    let mut seen = std::collections::BTreeSet::new();
    kept.retain(|l| seen.insert(l.clone()));
    kept
}

/// Splits output on newlines, dropping one trailing newline.
fn split_lines(text: &str) -> Vec<String> {
    if text.is_empty() {
        return Vec::new();
    }
    let trimmed = text.strip_suffix('\n').unwrap_or(text);
    if trimmed.is_empty() {
        return vec![String::new()];
    }
    trimmed.split('\n').map(str::to_owned).collect()
}

/// Runs `args`, compresses the output, and records the savings under `stats_path` when given.
///
/// stdout and stderr are shown one after the other. The Go version merged them as they were
/// written, so their interleaving can differ.
///
/// # Errors
///
/// No command, a command that cannot start, or a stats write failure. A command that exits
/// non-zero is not an error here: its compressed output comes back with `failure` set.
pub fn run(
    rules: &Rules,
    stats_path: Option<&Path>,
    args: &[String],
) -> Result<RunOutcome, RunError> {
    let (program, rest) = args.split_first().ok_or(RunError::NoCommand)?;
    let command = args.join(" ");
    let rule = rule_for(rules, &command);
    let max_lines = if rule.max_lines == 0 {
        DEFAULT_MAX_LINES
    } else {
        rule.max_lines
    };

    let child = Command::new(program)
        .args(rest)
        .output()
        .map_err(|source| RunError::Spawn {
            program: program.clone(),
            source,
        })?;
    let mut raw = String::from_utf8_lossy(&child.stdout).into_owned();
    raw.push_str(&String::from_utf8_lossy(&child.stderr));

    let (lines, label) = match filters::detect(args) {
        Some((name, filter)) => {
            let raw_lines = split_lines(&raw);
            let filtered = match filter(&raw_lines) {
                Ok(lines) => lines,
                Err(e) => {
                    eprintln!(
                        "plaesy trim: filter {name} failed ({e}); passing raw output through"
                    );
                    raw_lines
                }
            };
            (filtered, format!("layer1:{name}"))
        }
        None => (split_lines(&raw), "layer1".to_owned()),
    };
    let compressed_lines = compress_lines(&lines, max_lines, rule.dedupe_consecutive, &rule.mode);
    let mut output = compressed_lines.join("\n");
    if !compressed_lines.is_empty() {
        output.push('\n');
    }

    let before = stats::estimate_tokens(&raw);
    let after = stats::estimate_tokens(&output);
    if let Some(path) = stats_path {
        stats::add_record(path, &label, &command, before, after).map_err(RunError::Stats)?;
    }

    let failure = (!child.status.success()).then(|| format!("command failed: {}", child.status));
    Ok(RunOutcome {
        output,
        before,
        after,
        failure,
    })
}

/// Writes `output` to `out`, for callers that print the compressed text.
///
/// # Errors
///
/// Write failures on `out`.
pub fn print(out: &mut impl Write, outcome: &RunOutcome) -> io::Result<()> {
    out.write_all(outcome.output.as_bytes())
}

/// Reads a file's text for the Layer 2 commands.
///
/// # Errors
///
/// The file cannot be read.
pub fn read_text(path: &Path) -> io::Result<String> {
    fs::read_to_string(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn strings(items: &[&str]) -> Vec<String> {
        items.iter().map(|s| (*s).to_owned()).collect()
    }

    #[test]
    fn repeats_collapse_and_long_output_keeps_head_and_tail() {
        assert_eq!(
            compress_lines(&strings(&["a", "a", "a", "b"]), 10, true, ""),
            ["a (x3)", "b"]
        );
        let long: Vec<String> = (0..10).map(|i| i.to_string()).collect();
        let out = compress_lines(&long, 4, false, "");
        assert_eq!(
            out,
            ["0", "1", "... 6 lines omitted ...", "8", "9"].map(String::from)
        );
    }

    #[test]
    fn test_results_keep_failures_and_the_summary() {
        let out = compress_lines(
            &strings(&["ok a", "FAIL b", "5 passed"]),
            10,
            true,
            "test-results",
        );
        assert_eq!(out, ["FAIL b", "5 passed"]);
        let none = compress_lines(&strings(&["ok a"]), 10, true, "test-results");
        assert!(none[0].contains("all tests passed"));
    }

    #[test]
    fn the_longest_matching_rule_wins() {
        let rules = Rules::default();
        assert_eq!(rule_for(&rules, "cargo test --all").max_lines, 40);
        assert_eq!(rule_for(&rules, "make").max_lines, DEFAULT_MAX_LINES);
    }

    #[test]
    fn a_failing_command_keeps_its_output_and_reports_the_failure() {
        let rules = Rules::default();
        let args = if cfg!(windows) {
            strings(&["cmd", "/C", "echo boom & exit /B 3"])
        } else {
            strings(&["sh", "-c", "echo boom; exit 3"])
        };
        let outcome = run(&rules, None, &args).unwrap();
        assert!(outcome.output.contains("boom"));
        assert!(outcome.failure.is_some());
    }
}
