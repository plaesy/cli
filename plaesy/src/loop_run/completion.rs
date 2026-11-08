//! Completion gates and the report files a run ends with.

use std::fs;
use std::io;
use std::path::Path;
use std::sync::OnceLock;

use regex::Regex;

use super::config::Threshold;
use super::measure::{Measurement, UNMEASURED_TEXT};

/// Thresholds that are not met, and one note for each measurement that could not be
/// taken. An unmeasured mandatory threshold counts as unmet.
pub fn unmet_thresholds(
    thresholds: &[Threshold],
    measure: &mut dyn FnMut(&str) -> Measurement,
) -> (Vec<String>, Vec<String>) {
    let mut unmet = Vec::new();
    let mut notes = Vec::new();
    for th in thresholds.iter().filter(|t| t.mandatory) {
        let m = measure(&th.name);
        if !m.known {
            notes.push(format!("unmeasured threshold: {} ({})", th.name, m.note));
            unmet.push(format!("{} is {UNMEASURED_TEXT}", th.name));
            continue;
        }
        if m.value < th.min {
            unmet.push(format!(
                "{} {:.2} is below the mandatory {:.2}",
                th.name, m.value, th.min
            ));
        }
    }
    (unmet, notes)
}

fn candidate_status() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)status\W{0,6}candidate\b").expect("static regex"))
}

/// Signatures of learnings entries whose status is `candidate`. A missing file has none.
#[must_use]
pub fn read_candidates(path: Option<&Path>) -> Vec<String> {
    let Some(path) = path else {
        return Vec::new();
    };
    let Ok(text) = fs::read_to_string(path) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    let mut signature = String::new();
    for line in text.replace("\r\n", "\n").lines() {
        if let Some(i) = line.find("**signature:**") {
            signature = line[i + "**signature:**".len()..].trim().to_owned();
            continue;
        }
        if !signature.is_empty() && candidate_status().is_match(line) {
            out.push(std::mem::take(&mut signature));
        }
    }
    out
}

/// Writes a report into `dir`, replacing any earlier file of the same name.
///
/// # Errors
///
/// I/O failures creating the folder or writing the file.
pub fn write_report(dir: &Path, name: &str, lines: &[String]) -> io::Result<()> {
    fs::create_dir_all(dir)?;
    let mut body = lines.join("\n");
    body.push('\n');
    fs::write(dir.join(name), body)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::loop_run::measure::Measurement;

    #[test]
    fn unmeasured_mandatory_threshold_blocks_completion() {
        let thresholds = [Threshold {
            name: "coverage".into(),
            min: 90.0,
            mandatory: true,
        }];
        let (unmet, notes) = unmet_thresholds(&thresholds, &mut |name| Measurement {
            name: name.to_owned(),
            value: 0.0,
            known: false,
            note: "no command".into(),
        });
        assert_eq!(unmet.len(), 1);
        assert_eq!(notes.len(), 1);
    }

    #[test]
    fn candidate_entries_are_listed_by_signature() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("learnings.md");
        fs::write(
            &path,
            "**signature:** retry on 429\n**status:** candidate\n\n**signature:** old rule\n**status:** approved\n",
        )
        .unwrap();
        assert_eq!(read_candidates(Some(&path)), ["retry on 429"]);
    }
}
