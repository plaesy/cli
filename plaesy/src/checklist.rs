//! A task's `contracts.json` checklist: each item has a command that proves it
//! and an expected value its output must reach.
//!
//! An item is `pending` until it first passes, `passed` while its last run
//! passed, and `failed` while its last run failed. Every failed run is kept in
//! the item's history. A task is done when every item is `passed`.
//! Port of `scripts/internal/checklist` (Go).

use std::fs;
use std::io;
use std::path::Path;

use chrono::{DateTime, SecondsFormat, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::expected::{self, Outcome};

/// Errors while loading or saving a checklist.
#[derive(Debug, Error)]
pub enum ChecklistError {
    #[error("{path}: {source}")]
    Read {
        path: String,
        #[source]
        source: io::Error,
    },
    #[error("{path}: {source}")]
    Parse {
        path: String,
        #[source]
        source: serde_json::Error,
    },
    #[error("{path}: item {id:?} needs id, run and expected")]
    Incomplete { path: String, id: String },
    #[error("writing {path}: {source}")]
    Write {
        path: String,
        #[source]
        source: io::Error,
    },
}

/// Status of one item.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    #[default]
    Pending,
    Passed,
    Failed,
}

/// One failed run of an item.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct History {
    pub at: String,
    pub actual: String,
    pub run: String,
}

/// Evidence a passed item stores. `output` is relative to the project root.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Receipt {
    pub output: String,
    pub exit_code: i32,
    pub sha256: String,
}

/// One condition of the task.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Item {
    pub id: String,
    pub check: String,
    pub run: String,
    pub expected: String,
    pub status: Status,
    #[serde(default)]
    pub histories: Vec<History>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub receipt: Option<Receipt>,
    /// What kind of evidence the check gives: `build` (compiler, test runner, grep, file test)
    /// or `doc` (a written document). Such items are exempt from the rule that every item must
    /// invoke the plaesy binary. Any other value, or none, leaves the item subject to it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub evidence: Option<String>,
}

/// Contents of a task's `contracts.json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Checklist {
    pub id: String,
    pub checklist: Vec<Item>,
}

/// Captured result of one command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandOutput {
    pub exit_code: i32,
    pub stdout: String,
}

/// Loads and validates a `contracts.json`. Unknown fields are an error, so a
/// misspelt key cannot silently drop a check.
///
/// # Errors
///
/// Read or parse failures, or an item missing `id`, `run`, or `expected`.
pub fn load(path: &Path) -> Result<Checklist, ChecklistError> {
    let shown = path.display().to_string();
    let text = fs::read_to_string(path).map_err(|source| ChecklistError::Read {
        path: shown.clone(),
        source,
    })?;
    let checklist: Checklist =
        serde_json::from_str(&text).map_err(|source| ChecklistError::Parse {
            path: shown.clone(),
            source,
        })?;
    if let Some(bad) = checklist
        .checklist
        .iter()
        .find(|it| it.id.is_empty() || it.run.is_empty() || it.expected.is_empty())
    {
        return Err(ChecklistError::Incomplete {
            path: shown,
            id: bad.id.clone(),
        });
    }
    Ok(checklist)
}

/// Writes the checklist back, replacing the file in one step.
///
/// # Errors
///
/// Serialization or filesystem failures.
pub fn save(path: &Path, checklist: &Checklist) -> Result<(), ChecklistError> {
    let shown = path.display().to_string();
    let write_err = |source| ChecklistError::Write {
        path: shown.clone(),
        source,
    };
    let mut json =
        serde_json::to_string_pretty(checklist).map_err(|source| ChecklistError::Parse {
            path: shown.clone(),
            source,
        })?;
    json.push('\n');
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(".tmp");
    let tmp = std::path::PathBuf::from(tmp);
    fs::write(&tmp, json).map_err(write_err)?;
    fs::rename(&tmp, path).map_err(write_err)
}

/// Runs every item's command and sets its status.
///
/// `run_command` executes one command line. An `Err` means the command could
/// not be started, and it counts as a failed run.
pub fn run<F>(checklist: &mut Checklist, mut run_command: F, now: DateTime<Utc>)
where
    F: FnMut(&str) -> Result<CommandOutput, String>,
{
    for item in &mut checklist.checklist {
        match run_one(item, &mut run_command) {
            None => item.status = Status::Passed,
            Some(actual) => {
                item.status = Status::Failed;
                item.histories.push(History {
                    at: now.to_rfc3339_opts(SecondsFormat::Secs, true),
                    actual,
                    run: item.run.clone(),
                });
            }
        }
    }
}

/// Returns `None` when the item passes, or the failure message.
fn run_one<F>(item: &Item, run_command: &mut F) -> Option<String>
where
    F: FnMut(&str) -> Result<CommandOutput, String>,
{
    let output = match run_command(&item.run) {
        Ok(output) => output,
        Err(message) => return Some(message),
    };
    if output.exit_code != 0 {
        return Some(format!("exit code {}", output.exit_code));
    }
    let judged = expected::judge(&item.expected, last_line(&output.stdout));
    if judged.outcome == Outcome::Met {
        return None;
    }
    if judged.reason.is_empty() {
        Some(format!(
            "result {}: actual {:?}, expected {:?} was not met. Accepted expected values: {}",
            outcome_label(judged.outcome),
            judged.actual,
            item.expected,
            expected::ACCEPTED_VALUES
        ))
    } else {
        Some(judged.reason)
    }
}

fn outcome_label(outcome: Outcome) -> &'static str {
    match outcome {
        Outcome::Met => "met",
        Outcome::NotMet => "not_met",
        Outcome::NotMeasurable => "not_measurable",
    }
}

/// Reports whether every item is passed. An empty checklist is not done.
#[must_use]
pub fn is_done(checklist: &Checklist) -> bool {
    !checklist.checklist.is_empty()
        && checklist
            .checklist
            .iter()
            .all(|it| it.status == Status::Passed)
}

/// Passed items and total items.
#[must_use]
pub fn progress(checklist: &Checklist) -> (usize, usize) {
    let passed = checklist
        .checklist
        .iter()
        .filter(|it| it.status == Status::Passed)
        .count();
    (passed, checklist.checklist.len())
}

fn last_line(out: &str) -> &str {
    out.lines()
        .rev()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or("")
}

#[cfg(test)]
mod tests {
    use chrono::TimeZone;

    use super::*;

    fn now() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 10, 22, 12, 0, 0).unwrap()
    }

    fn item(id: &str, run: &str, expected: &str) -> Item {
        Item {
            id: id.into(),
            check: String::new(),
            run: run.into(),
            expected: expected.into(),
            status: Status::Pending,
            histories: Vec::new(),
            receipt: None,
            evidence: None,
        }
    }

    fn ok(stdout: &str) -> Result<CommandOutput, String> {
        Ok(CommandOutput {
            exit_code: 0,
            stdout: stdout.into(),
        })
    }

    #[test]
    fn failures_are_kept_in_histories() {
        let mut list = Checklist {
            id: "t".into(),
            checklist: vec![
                item("C1", "ok", "pass"),
                item("C2", "count", ">= 3"),
                item("C3", "broken", "pass"),
            ],
        };
        run(
            &mut list,
            |cmd| match cmd {
                "ok" => ok(""),
                "count" => ok("5\n"),
                _ => Err("no shell".into()),
            },
            now(),
        );

        assert_eq!(list.checklist[0].status, Status::Passed);
        assert_eq!(list.checklist[1].status, Status::Passed);
        let c3 = &list.checklist[2];
        assert_eq!(c3.status, Status::Failed);
        assert_eq!(
            c3.histories,
            [History {
                at: "2026-10-22T12:00:00Z".into(),
                actual: "no shell".into(),
                run: "broken".into(),
            }]
        );
        assert!(!is_done(&list));
    }

    #[test]
    fn failures_accumulate_and_a_later_pass_keeps_them() {
        let mut list = Checklist {
            id: "t".into(),
            checklist: vec![item("C1", "x", "pass")],
        };
        let fail = |_: &str| ok_code(1);
        run(&mut list, fail, now());
        run(&mut list, |_| ok_code(1), now());
        assert_eq!(list.checklist[0].histories.len(), 2);

        run(&mut list, |_| ok(""), now());
        assert!(is_done(&list));
        assert_eq!(list.checklist[0].histories.len(), 2);
        assert_eq!(progress(&list), (1, 1));
    }

    fn ok_code(exit_code: i32) -> Result<CommandOutput, String> {
        Ok(CommandOutput {
            exit_code,
            stdout: String::new(),
        })
    }

    #[test]
    fn sentence_expectation_names_the_accepted_values() {
        let mut list = Checklist {
            id: "t".into(),
            checklist: vec![item(
                "R10",
                "go test ./internal/x/ -count=1",
                "exit code 2 while running; turn ends when complete",
            )],
        };
        run(&mut list, |_| ok("ok example 0.012s\n"), now());

        assert_eq!(list.checklist[0].status, Status::Failed);
        let msg = &list.checklist[0].histories[0].actual;
        for want in ["ok example 0.012s", "pass", "zero", "all", ">= 9"] {
            assert!(msg.contains(want), "history {msg:?} must contain {want:?}");
        }
    }

    #[test]
    fn load_rejects_unknown_fields_and_incomplete_items() {
        let dir = tempfile::tempdir().unwrap();
        let typo = dir.path().join("typo.json");
        fs::write(
            &typo,
            r#"{"id":"t","checklist":[{"id":"C1","check":"","run":"x","expected":"pass","status":"pending","histroies":[]}]}"#,
        )
        .unwrap();
        assert!(matches!(load(&typo), Err(ChecklistError::Parse { .. })));

        let empty_run = dir.path().join("empty.json");
        fs::write(
            &empty_run,
            r#"{"id":"t","checklist":[{"id":"C1","check":"","run":"","expected":"pass","status":"pending"}]}"#,
        )
        .unwrap();
        assert!(matches!(
            load(&empty_run),
            Err(ChecklistError::Incomplete { .. })
        ));
    }

    #[test]
    fn save_then_load_round_trips() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("contracts.json");
        let list = Checklist {
            id: "t".into(),
            checklist: vec![item("C1", "x", "pass")],
        };
        save(&path, &list).unwrap();
        assert_eq!(load(&path).unwrap(), list);
    }

    #[test]
    fn shipped_contract_template_loads() {
        let template = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../spec-kit/skills/plaesy/assets/contract.json");
        load(&template).expect("contract.json template loads");
    }
}
