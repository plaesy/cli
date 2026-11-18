//! Unit tests for `tests` split out of `constitution.rs`.
//! Kept as a separate file so they are easy to find; included via `#[path]`.

use super::*;

const VALID: &str = "---
title: Plaesy Constitution
version: 1.2.0
ratified: 2026-10-01
last_amended: 2026-10-20
active_dimensions: [business, technical]
---

## 1. Active Dimensions

| Dimension | Active |
|---|---|
| Technical | yes |
| Design | no |
| Business | yes |

## 4. Universal Rules

| ID | Rule |
|---|---|
| EV-01 | Evidence first |
| QG-01 | Gates first |

## 5. Non-Negotiables

| ID | Rule |
|---|---|
| NN-01 | Never skip tests |

## Amendment Log

| Version | Date | Change |
|---|---|---|
| 1.2.0 | 2026-10-20 | add operations |
";

fn write(dir: &tempfile::TempDir, body: &str) -> std::path::PathBuf {
    let path = dir.path().join("constitution.md");
    fs::write(&path, body).unwrap();
    path
}

fn message(err: ValidateError) -> String {
    err.to_string()
}

#[test]
fn valid_constitution_reports_scope_and_rule_counts() {
    let dir = tempfile::tempdir().unwrap();
    let report = validate(&write(&dir, VALID)).unwrap();
    assert_eq!(report.version, "1.2.0");
    assert_eq!(report.active_dimensions, ["business", "technical"]);
    assert_eq!(report.rules, 2);
    assert_eq!(report.non_negotiables, 1);
}

#[test]
fn absent_file_is_unratified_not_a_pass() {
    let dir = tempfile::tempdir().unwrap();
    let err = validate(&dir.path().join("missing.md")).unwrap_err();
    assert!(message(err).contains("not ratified yet"));
}

#[test]
fn drift_between_frontmatter_and_table_and_log_is_reported_together() {
    let dir = tempfile::tempdir().unwrap();
    let drifted = VALID
        .replace("[business, technical]", "[design]")
        .replace("version: 1.2.0", "version: 1.3.0");
    let msg = message(validate(&write(&dir, &drifted)).unwrap_err());
    assert!(msg.contains("does not match the §1 rows"), "{msg}");
    assert!(msg.contains("version drift"), "{msg}");
}

#[test]
fn unknown_dimension_and_bad_flag_fail() {
    let dir = tempfile::tempdir().unwrap();
    let unknown = VALID.replace("| Design | no |", "| Gardening | no |");
    assert!(message(validate(&write(&dir, &unknown)).unwrap_err()).contains("unknown dimension"));
    let bad_flag = VALID.replace("| Design | no |", "| Design | maybe |");
    assert!(message(validate(&write(&dir, &bad_flag)).unwrap_err()).contains("must be yes or no"));
}

#[test]
fn leftover_placeholders_and_duplicate_ids_fail() {
    let dir = tempfile::tempdir().unwrap();
    let leftover = VALID.replace("Evidence first", "{{RULE_TEXT}}");
    assert!(message(validate(&write(&dir, &leftover)).unwrap_err())
        .contains("unfilled template placeholders"));
    let dup = VALID.replace("QG-01", "EV-01");
    assert!(message(validate(&write(&dir, &dup)).unwrap_err()).contains("duplicate rule ID"));
}

#[test]
fn escaped_pipe_stays_inside_its_cell() {
    assert_eq!(split_row(r"| a \| b | c |"), ["a | b", "c"]);
}

#[test]
fn dimension_key_normalizes_labels() {
    assert_eq!(dimension_key("Internet  Access"), "internet_access");
}
