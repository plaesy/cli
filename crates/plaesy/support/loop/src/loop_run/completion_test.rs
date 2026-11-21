//! Unit tests for `tests` split out of `completion.rs`.
//! Kept as a separate file so they are easy to find; included via `#[path]`.

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
