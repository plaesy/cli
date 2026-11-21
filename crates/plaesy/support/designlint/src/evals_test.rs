//! Unit tests for `tests` split out of `evals.rs`.
//! Kept as a separate file so they are easy to find; included via `#[path]`.

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
