//! Runs the checks declared in `evals/cases`.
//!
//! This is not a model-behaviour test unless a runner is configured. A case file
//! names one instruction file and holds three kinds of case:
//!
//! - **static**: the instruction text must contain some phrases and must not
//!   contain others. This checks wording, not meaning.
//! - **outputs**: a fixture directory of hand-written frontend files is linted
//!   with `designlint`, and must pass or fail as declared.
//! - **briefs**: a model writes output for each brief. This needs a runner set in
//!   `PLAESY_EVAL_RUNNER`. Without one, every brief is `SKIP`, and a skip is never
//!   a pass.
//!
//! Port of `scripts/internal/evals` (Go).

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde::Deserialize;
use thiserror::Error;

use crate::designlint::{self, Report, Severity};

/// Case files, relative to the repository root.
pub const CASES_DIR: &str = "evals/cases";

/// Outcome status.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Pass,
    Fail,
    Skip,
}

impl Status {
    /// Label printed in reports.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Pass => "PASS",
            Self::Fail => "FAIL",
            Self::Skip => "SKIP",
        }
    }
}

/// One case file.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CaseFile {
    pub instruction: String,
    #[serde(default, rename = "static")]
    pub statics: Vec<Static>,
    #[serde(default)]
    pub outputs: Vec<Output>,
    #[serde(default)]
    pub briefs: Vec<Brief>,
}

/// A phrase check on the instruction text. Matching ignores case.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Static {
    pub id: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub requires: Vec<String>,
    #[serde(default)]
    pub forbids: Vec<String>,
}

/// A lint expectation on a fixture directory.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Output {
    pub id: String,
    #[serde(default)]
    pub description: String,
    pub fixture: String,
    /// `pass` (no FAIL) or `fail` (at least one FAIL).
    pub expect: String,
    #[serde(default)]
    pub rules: Vec<String>,
}

/// One prompt for the optional runner.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Brief {
    pub id: String,
    pub brief: String,
}

/// Result of one case.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Outcome {
    pub file: String,
    pub kind: &'static str,
    pub id: String,
    pub status: Status,
    pub detail: String,
}

/// Errors from loading or running cases.
#[derive(Debug, Error)]
pub enum EvalError {
    #[error("{file}: {message}")]
    Case { file: String, message: String },
    #[error("PLAESY_EVAL_RUNNER: {0}")]
    Runner(String),
    #[error("{context}: {source}")]
    Io {
        context: String,
        #[source]
        source: std::io::Error,
    },
}

/// Loads and validates every case file under `root/evals/cases`, keyed by file name.
///
/// # Errors
///
/// I/O failures, JSON with unknown keys, or a case that is missing required fields.
pub fn load(root: &Path) -> Result<BTreeMap<String, CaseFile>, EvalError> {
    let dir = root.join(CASES_DIR);
    let entries = fs::read_dir(&dir).map_err(|e| EvalError::Io {
        context: format!("reading {}", dir.display()),
        source: e,
    })?;
    let mut out = BTreeMap::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() || path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        let text = fs::read_to_string(&path).map_err(|e| EvalError::Io {
            context: format!("reading {name}"),
            source: e,
        })?;
        let case: CaseFile = serde_json::from_str(&text).map_err(|e| EvalError::Case {
            file: name.clone(),
            message: e.to_string(),
        })?;
        validate(&case).map_err(|message| EvalError::Case {
            file: name.clone(),
            message,
        })?;
        out.insert(name, case);
    }
    Ok(out)
}

fn validate(case: &CaseFile) -> Result<(), String> {
    if case.instruction.is_empty() {
        return Err("instruction is required".into());
    }
    for s in &case.statics {
        if s.id.is_empty() {
            return Err("static case without id".into());
        }
        if s.requires.is_empty() && s.forbids.is_empty() {
            return Err(format!(
                "static case {:?} has neither requires nor forbids",
                s.id
            ));
        }
    }
    for o in &case.outputs {
        if o.id.is_empty() || o.fixture.is_empty() {
            return Err("output case needs id and fixture".into());
        }
        if o.expect != "pass" && o.expect != "fail" {
            return Err(format!(
                "output case {:?}: expect must be pass or fail, got {:?}",
                o.id, o.expect
            ));
        }
    }
    for b in &case.briefs {
        if b.id.is_empty() || b.brief.is_empty() {
            return Err("brief needs id and brief text".into());
        }
    }
    Ok(())
}

/// Evaluates every case file. Order is file name, then kind, then declaration order.
///
/// # Errors
///
/// Loading errors from [`load`].
pub fn run(root: &Path, runner: Option<&str>) -> Result<Vec<Outcome>, EvalError> {
    let files = load(root)?;
    let mut out = Vec::new();
    for (name, case) in &files {
        out.extend(run_static(root, name, case));
        out.extend(run_outputs(root, name, case));
        out.extend(run_briefs(name, case, runner));
    }
    Ok(out)
}

fn run_static(root: &Path, name: &str, case: &CaseFile) -> Vec<Outcome> {
    let text = match fs::read_to_string(root.join(&case.instruction)) {
        Ok(text) => text.to_lowercase(),
        Err(e) => {
            return vec![Outcome {
                file: name.to_owned(),
                kind: "static",
                id: "(instruction)".into(),
                status: Status::Fail,
                detail: e.to_string(),
            }]
        }
    };
    case.statics
        .iter()
        .map(|s| {
            let mut problems: Vec<String> = s
                .requires
                .iter()
                .filter(|want| !text.contains(&want.to_lowercase()))
                .map(|want| format!("missing {want:?}"))
                .collect();
            problems.extend(
                s.forbids
                    .iter()
                    .filter(|bad| text.contains(&bad.to_lowercase()))
                    .map(|bad| format!("contains forbidden {bad:?}")),
            );
            let (status, detail) = if problems.is_empty() {
                (Status::Pass, s.description.clone())
            } else {
                (Status::Fail, problems.join("; "))
            };
            Outcome {
                file: name.to_owned(),
                kind: "static",
                id: s.id.clone(),
                status,
                detail,
            }
        })
        .collect()
}

fn run_outputs(root: &Path, name: &str, case: &CaseFile) -> Vec<Outcome> {
    case.outputs
        .iter()
        .map(|c| {
            let fixture = root.join(&c.fixture);
            let (status, detail) =
                match designlint::lint(&[fixture], &designlint::Options::default()) {
                    Ok(report) => judge(&c.expect, &c.rules, &report),
                    Err(e) => (Status::Fail, e.to_string()),
                };
            let detail = if detail.is_empty() {
                c.description.clone()
            } else {
                detail
            };
            Outcome {
                file: name.to_owned(),
                kind: "output",
                id: c.id.clone(),
                status,
                detail,
            }
        })
        .collect()
}

/// Applies one expectation to a lint result. `pass` allows WARN but not FAIL.
/// `fail` needs a FAIL. Every named rule must appear at any severity.
fn judge(expect: &str, rules: &[String], report: &Report) -> (Status, String) {
    let mut problems = Vec::new();
    if expect == "pass" && report.failed() {
        problems.push(format!("expected no FAIL, got {}", fail_rules(report)));
    }
    if expect == "fail" && !report.failed() {
        problems.push("expected a FAIL, got none".to_owned());
    }
    for rule in rules {
        if !report.findings.iter().any(|f| f.rule == rule) {
            problems.push(format!("rule {rule} did not appear"));
        }
    }
    if problems.is_empty() {
        (Status::Pass, String::new())
    } else {
        (Status::Fail, problems.join("; "))
    }
}

fn fail_rules(report: &Report) -> String {
    let mut rules: Vec<&str> = Vec::new();
    for f in report
        .findings
        .iter()
        .filter(|f| f.severity == Severity::Fail)
    {
        if !rules.contains(&f.rule) {
            rules.push(f.rule);
        }
    }
    rules.join(", ")
}

fn run_briefs(name: &str, case: &CaseFile, runner: Option<&str>) -> Vec<Outcome> {
    case.briefs
        .iter()
        .map(|b| {
            let (status, detail) = match runner {
                None => (
                    Status::Skip,
                    "no PLAESY_EVAL_RUNNER set; brief not run".to_owned(),
                ),
                Some(runner) => run_one(runner, b),
            };
            Outcome {
                file: name.to_owned(),
                kind: "brief",
                id: b.id.clone(),
                status,
                detail,
            }
        })
        .collect()
}

/// Calls the runner for one brief, then lints what it wrote. A runner that writes
/// no lintable file fails, because an empty output would otherwise lint clean.
fn run_one(runner: &str, brief: &Brief) -> (Status, String) {
    let argv = match parse_runner(runner) {
        Ok(argv) => argv,
        Err(e) => return (Status::Fail, e.to_string()),
    };
    let dir = std::env::temp_dir().join(format!("plaesy-eval-{}-{}", std::process::id(), brief.id));
    if let Err(e) = fs::create_dir_all(&dir) {
        return (Status::Fail, e.to_string());
    }
    let outcome = run_in(&argv, brief, &dir);
    let _ = fs::remove_dir_all(&dir);
    outcome
}

fn run_in(argv: &[String], brief: &Brief, dir: &Path) -> (Status, String) {
    let output = Command::new(&argv[0])
        .args(&argv[1..])
        .arg(&brief.id)
        .arg(dir)
        .env("PLAESY_EVAL_BRIEF", &brief.brief)
        .output();
    let output = match output {
        Ok(output) => output,
        Err(e) => return (Status::Fail, format!("runner failed to start: {e}")),
    };
    if !output.status.success() {
        let mut text = String::from_utf8_lossy(&output.stdout).into_owned();
        text.push_str(&String::from_utf8_lossy(&output.stderr));
        return (
            Status::Fail,
            format!("runner failed: {}: {}", output.status, text.trim()),
        );
    }
    let report = match designlint::lint(&[dir.to_path_buf()], &designlint::Options::default()) {
        Ok(report) => report,
        Err(e) => return (Status::Fail, e.to_string()),
    };
    if report.files == 0 {
        return (
            Status::Fail,
            format!("runner wrote no lintable files to {}", dir.display()),
        );
    }
    if report.failed() {
        return (
            Status::Fail,
            format!("output has FAIL findings: {}", fail_rules(&report)),
        );
    }
    (
        Status::Pass,
        format!("output lints clean ({} file(s))", report.files),
    )
}

/// Splits the runner setting into argv. Two forms are accepted: a JSON array of
/// strings, which is unambiguous for paths with spaces, or a command line where
/// quoted groups stay one word and backslashes are kept, so a Windows path works.
///
/// # Errors
///
/// An empty setting, malformed JSON, or an unclosed quote.
pub fn parse_runner(setting: &str) -> Result<Vec<String>, EvalError> {
    let setting = setting.trim();
    if setting.is_empty() {
        return Err(EvalError::Runner("is empty".into()));
    }
    if setting.starts_with('[') {
        let argv: Vec<String> = serde_json::from_str(setting).map_err(|e| {
            EvalError::Runner(format!("looks like a JSON array but does not parse: {e}"))
        })?;
        if argv.is_empty() {
            return Err(EvalError::Runner("JSON array is empty".into()));
        }
        return Ok(argv);
    }

    let mut argv = Vec::new();
    let mut word = String::new();
    let mut in_word = false;
    let mut quote: Option<char> = None;
    for c in setting.chars() {
        match quote {
            Some(q) if c == q => quote = None,
            Some(_) => word.push(c),
            None if c == '"' || c == '\'' => {
                quote = Some(c);
                in_word = true;
            }
            None if c == ' ' || c == '\t' => {
                if in_word {
                    argv.push(std::mem::take(&mut word));
                    in_word = false;
                }
            }
            None => {
                word.push(c);
                in_word = true;
            }
        }
    }
    if let Some(q) = quote {
        return Err(EvalError::Runner(format!("has an unclosed {q} quote")));
    }
    if in_word {
        argv.push(word);
    }
    Ok(argv)
}

/// Counts outcomes by status: `(pass, fail, skip)`.
#[must_use]
pub fn summary(outcomes: &[Outcome]) -> (usize, usize, usize) {
    outcomes
        .iter()
        .fold((0, 0, 0), |(p, f, s), o| match o.status {
            Status::Pass => (p + 1, f, s),
            Status::Fail => (p, f + 1, s),
            Status::Skip => (p, f, s + 1),
        })
}

/// Path of the case directory under a root.
#[must_use]
pub fn cases_dir(root: &Path) -> PathBuf {
    root.join(CASES_DIR)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runner_accepts_quoted_windows_paths_and_json_arrays() {
        assert_eq!(
            parse_runner(r#""C:\tools\my runner.exe" --fast"#).unwrap(),
            [r"C:\tools\my runner.exe", "--fast"]
        );
        assert_eq!(
            parse_runner(r#"["C:\\tools\\my runner.exe", "--fast"]"#).unwrap(),
            [r"C:\tools\my runner.exe", "--fast"]
        );
        assert!(parse_runner(r#""unclosed"#).is_err());
        assert!(parse_runner("   ").is_err());
    }

    #[test]
    fn judge_applies_pass_and_fail_expectations() {
        let clean = Report::default();
        assert_eq!(judge("pass", &[], &clean).0, Status::Pass);
        assert_eq!(judge("fail", &[], &clean).0, Status::Fail);
        assert_eq!(
            judge("pass", &["DL-EMOJI".into()], &clean).1,
            "rule DL-EMOJI did not appear"
        );
    }

    #[test]
    fn briefs_are_skipped_without_a_runner() {
        let case = CaseFile {
            instruction: "x.md".into(),
            statics: vec![],
            outputs: vec![],
            briefs: vec![Brief {
                id: "b1".into(),
                brief: "A landing page".into(),
            }],
        };
        let outcomes = run_briefs("c.json", &case, None);
        assert_eq!(outcomes[0].status, Status::Skip);
    }

    #[test]
    fn unknown_keys_in_a_case_file_are_rejected() {
        let ok = r#"{"instruction":"x.md","static":[]}"#;
        assert!(serde_json::from_str::<CaseFile>(ok).is_ok());
        let typo = r#"{"instruction":"x.md","outptus":[]}"#;
        assert!(serde_json::from_str::<CaseFile>(typo).is_err());
    }

    #[test]
    fn summary_counts_each_status() {
        let mk = |status| Outcome {
            file: String::new(),
            kind: "static",
            id: String::new(),
            status,
            detail: String::new(),
        };
        let outcomes = [
            mk(Status::Pass),
            mk(Status::Fail),
            mk(Status::Skip),
            mk(Status::Skip),
        ];
        assert_eq!(summary(&outcomes), (1, 1, 2));
    }
}
