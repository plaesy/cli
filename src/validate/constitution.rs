//! Validation of the generated project constitution
//! (`.plaesy/memory/constitution.md`).
//!
//! The constitution is the document that governs every later phase. A
//! half-filled or drifted copy must not be read as authority, so this check
//! enforces the contract the template states about itself. Every problem is
//! reported in one run.

use std::collections::HashSet;
use std::fs;
use std::path::Path;
use std::sync::OnceLock;

use regex::Regex;

use super::{jinja_tag_regex, ValidateError};

/// The closed set of dimensions the framework routes. A row outside this set
/// would be invisible to every prompt, so it fails validation.
pub const KNOWN_DIMENSIONS: [&str; 9] = [
    "technical",
    "design",
    "business",
    "product",
    "marketing",
    "legal",
    "financial",
    "management",
    "operations",
];

/// What a valid constitution declares.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ConstitutionReport {
    pub path: String,
    pub version: String,
    /// Dimensions marked `yes` in §1, sorted.
    pub active_dimensions: Vec<String>,
    /// Rows in the Universal Rules table (§4).
    pub rules: usize,
    /// Rows in the Non-Negotiables table (§5).
    pub non_negotiables: usize,
}

/// Validates the constitution at `path`.
///
/// # Errors
///
/// [`ValidateError::Invalid`] when the file is absent, has no frontmatter, or
/// fails any rule. The message lists every problem found.
pub fn validate(path: &Path) -> Result<ConstitutionReport, ValidateError> {
    let shown = path.display().to_string();
    let body = fs::read_to_string(path).map_err(|_| {
        ValidateError::invalid(format!(
            "constitution not found at {shown}: not ratified yet — run /start (Phase 0) to generate it from skills/plaesy/assets/constitution.template.md"
        ))
    })?;

    let (front, rest) = split_frontmatter(&body).ok_or_else(|| {
        ValidateError::invalid(format!(
            "constitution {shown} has no YAML frontmatter (expected a leading '---' block with title, version, ratified, last_amended, active_dimensions)"
        ))
    })?;
    let fields = parse_frontmatter(front);
    let mut problems = Vec::new();

    for key in [
        "title",
        "version",
        "ratified",
        "last_amended",
        "active_dimensions",
    ] {
        if fields.get(key).is_none_or(|v| v.trim().is_empty()) {
            problems.push(format!("frontmatter key {key:?} is missing or empty"));
        }
    }
    for key in ["ratified", "last_amended"] {
        let value = fields.get(key).map_or("", |v| v.trim_matches(['"', '\'']));
        if !value.is_empty() && !is_iso_date(value) {
            problems.push(format!(
                "frontmatter {key} = {value:?} is not an ISO 8601 date (YYYY-MM-DD)"
            ));
        }
    }
    let version = fields
        .get("version")
        .map_or("", |v| v.trim_matches(['"', '\''])) // unquoted or quoted both accepted
        .to_owned();

    let leftover: Vec<String> = jinja_tag_regex()
        .find_iter(&body)
        .map(|m| m.as_str().to_owned())
        .collect();
    if !leftover.is_empty() {
        problems.push(format!(
            "unfilled template placeholders remain: {}",
            summarize(&leftover)
        ));
    }

    let table = parse_active_dimensions(rest);
    let active = match &table {
        Ok(active) => active.clone(),
        Err(message) => {
            problems.push(message.clone());
            Vec::new()
        }
    };
    if table.is_ok() && active.is_empty() {
        problems.push("no active dimension: mark at least one §1 row 'yes' (a constitution that governs nothing is not a constitution)".into());
    }

    let raw_declared = fields.get("active_dimensions").map_or("", String::as_str);
    let declared = parse_dimension_list(raw_declared);
    if !(declared.is_empty() && raw_declared.trim().is_empty())
        && table.is_ok()
        && declared != active
    {
        problems.push(format!(
            "frontmatter active_dimensions [{}] does not match the §1 rows marked yes [{}] — the table is the source; make them agree",
            declared.join(", "),
            active.join(", ")
        ));
    }

    if let Some(logged) = last_logged_version(rest) {
        if !version.is_empty() && logged != version {
            problems.push(format!(
                "version drift: frontmatter says {version:?} but the last amendment-log row says {logged:?} — the frontmatter must match the log"
            ));
        }
    }

    let (rules, non_negotiables) = count_rule_rows(rest, &mut problems);

    if !problems.is_empty() {
        problems.sort();
        return Err(ValidateError::invalid(format!(
            "constitution {shown} failed validation:\n  - {}",
            problems.join("\n  - ")
        )));
    }
    Ok(ConstitutionReport {
        path: shown,
        version,
        active_dimensions: active,
        rules,
        non_negotiables,
    })
}

fn is_iso_date(value: &str) -> bool {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^[0-9]{4}-[0-9]{2}-[0-9]{2}$").expect("static regex"))
        .is_match(value)
}

/// Returns the frontmatter block and the document body after it.
fn split_frontmatter(body: &str) -> Option<(&str, &str)> {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| Regex::new(r"(?m)^---\r?\n").expect("static regex"));
    if !body.starts_with("---") {
        return None;
    }
    let mut parts = re.splitn(body, 3);
    let _before = parts.next()?;
    let front = parts.next()?;
    let rest = parts.next()?;
    Some((front, rest))
}

fn parse_frontmatter(front: &str) -> std::collections::HashMap<String, String> {
    front
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .filter_map(|line| {
            let (key, value) = line.split_once(':')?;
            Some((key.trim().to_owned(), value.trim().to_owned()))
        })
        .collect()
}

/// Reads `active_dimensions: [technical, business]` into a sorted list.
fn parse_dimension_list(raw: &str) -> Vec<String> {
    let inner = raw.trim().trim_start_matches('[').trim_end_matches(']');
    let mut out: Vec<String> = inner
        .split(',')
        .map(|part| part.trim().trim_matches(['"', '\'']).to_lowercase())
        .filter(|part| !part.is_empty())
        .collect();
    out.sort();
    out
}

/// Returns the dimensions marked `yes` in the §1 table, sorted. An `Err` means the
/// table itself is unusable.
fn parse_active_dimensions(body: &str) -> Result<Vec<String>, String> {
    let (_, rows) = table_after_heading(body, "Active Dimensions")?;
    let mut active = Vec::new();
    for cells in &rows {
        if cells.len() < 2 {
            return Err(format!(
                "§1 Active Dimensions row has {} cell(s), need at least a dimension and an Active cell: {:?}",
                cells.len(),
                cells.join(" | ")
            ));
        }
        let name = dimension_key(&cells[0]);
        let flag = cells[1].to_lowercase();
        if !KNOWN_DIMENSIONS.contains(&name.as_str()) {
            return Err(format!(
                "§1 Active Dimensions row {:?} is an unknown dimension (known: {}) — a row outside the closed set is invisible to every prompt",
                cells[0],
                KNOWN_DIMENSIONS.join(", ")
            ));
        }
        match flag.as_str() {
            "yes" => active.push(name),
            "no" => {}
            _ => {
                return Err(format!(
                    "§1 Active Dimensions cell for {:?} must be yes or no, got {:?}",
                    cells[0], cells[1]
                ))
            }
        }
    }
    active.sort();
    Ok(active)
}

/// Turns a §1 label such as "Internet Access" into `internet_access`.
fn dimension_key(label: &str) -> String {
    label
        .split_whitespace()
        .collect::<Vec<_>>()
        .join("_")
        .to_lowercase()
}

/// Version in the last data row of the amendment log, if the log has a Version column.
fn last_logged_version(body: &str) -> Option<String> {
    let (header, rows) = table_after_heading(body, "Amendment Log").ok()?;
    let index = header
        .iter()
        .position(|cell| cell.eq_ignore_ascii_case("version"))?;
    rows.last()?.get(index).cloned()
}

/// Counts §4 and §5 rule rows and reports duplicate IDs.
fn count_rule_rows(body: &str, problems: &mut Vec<String>) -> (usize, usize) {
    static RE: OnceLock<Regex> = OnceLock::new();
    let rule_id =
        RE.get_or_init(|| Regex::new(r"^(EV|QG|DOC|SC|RTM|NN)-[0-9]+$").expect("static regex"));
    let mut seen = HashSet::new();
    let (mut rules, mut non_negotiables) = (0, 0);
    for cells in all_table_rows(body) {
        let Some(first) = cells.first() else {
            continue;
        };
        let id = first.trim();
        if !rule_id.is_match(id) {
            continue;
        }
        if !seen.insert(id.to_owned()) {
            problems.push(format!(
                "duplicate rule ID {id:?} — IDs are cited by phases and findings, so they must be unique"
            ));
            continue;
        }
        if id.starts_with("NN-") {
            non_negotiables += 1;
        } else {
            rules += 1;
        }
    }
    (rules, non_negotiables)
}

/// Every table row in the document, including header rows, which callers filter by content.
fn all_table_rows(body: &str) -> Vec<Vec<String>> {
    let mut out = Vec::new();
    for line in body.split('\n') {
        let trimmed = line.trim();
        if !trimmed.starts_with('|') || is_separator_row(trimmed) {
            continue;
        }
        out.push(split_row(trimmed));
    }
    out
}

/// Header and data rows of the first table after a heading containing `name`.
fn table_after_heading(body: &str, name: &str) -> Result<(Vec<String>, Vec<Vec<String>>), String> {
    let needle = name.to_lowercase();
    let mut found = false;
    let mut header: Option<Vec<String>> = None;
    let mut data = Vec::new();
    for line in body.split('\n') {
        let trimmed = line.trim();
        if !found {
            if trimmed.starts_with('#') && trimmed.to_lowercase().contains(&needle) {
                found = true;
            }
            continue;
        }
        if trimmed.starts_with('|') {
            if header.is_none() {
                header = Some(split_row(trimmed));
            } else if !is_separator_row(trimmed) {
                data.push(split_row(trimmed));
            }
            continue;
        }
        if header.is_some() {
            break; // the first table after the heading is the one meant
        }
    }
    if !found {
        return Err(format!(
            "no heading containing {name:?} found — the constitution must keep its '## 1. Active Dimensions' section"
        ));
    }
    let header = header.ok_or_else(|| format!("no table under {name:?}"))?;
    if data.is_empty() {
        return Err(format!("no data rows in the table under {name:?}"));
    }
    Ok((header, data))
}

/// Splits one markdown table row into trimmed cells. An escaped `\|` stays a
/// literal pipe inside its cell, because placeholder defaults such as
/// `{{VAR|default}}` are escaped in table cells.
fn split_row(line: &str) -> Vec<String> {
    let line = line.trim();
    let line = line.strip_prefix('|').unwrap_or(line);
    let line = line.strip_suffix('|').unwrap_or(line);

    let mut cells = Vec::new();
    let mut current = String::new();
    let mut escaped = false;
    for ch in line.chars() {
        if escaped {
            if ch != '|' {
                current.push('\\');
            }
            current.push(ch);
            escaped = false;
        } else if ch == '\\' {
            escaped = true;
        } else if ch == '|' {
            cells.push(current.trim().to_owned());
            current.clear();
        } else {
            current.push(ch);
        }
    }
    if escaped {
        current.push('\\');
    }
    cells.push(current.trim().to_owned());
    cells
}

fn is_separator_row(line: &str) -> bool {
    split_row(line)
        .iter()
        .all(|cell| cell.is_empty() || cell.trim_matches(['-', ':', ' ']).is_empty())
}

/// Dedupes and caps a placeholder list so a raw template still prints a readable error.
fn summarize(items: &[String]) -> String {
    const MAX: usize = 10;
    let mut seen = HashSet::new();
    let unique: Vec<&String> = items.iter().filter(|s| seen.insert(s.as_str())).collect();
    if unique.len() <= MAX {
        return unique
            .iter()
            .map(|s| s.as_str())
            .collect::<Vec<_>>()
            .join(", ");
    }
    let head: Vec<&str> = unique[..MAX].iter().map(|s| s.as_str()).collect();
    format!("{}, … and {} more", head.join(", "), unique.len() - MAX)
}

#[cfg(test)]
mod tests {
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
        assert!(
            message(validate(&write(&dir, &unknown)).unwrap_err()).contains("unknown dimension")
        );
        let bad_flag = VALID.replace("| Design | no |", "| Design | maybe |");
        assert!(
            message(validate(&write(&dir, &bad_flag)).unwrap_err()).contains("must be yes or no")
        );
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
}
