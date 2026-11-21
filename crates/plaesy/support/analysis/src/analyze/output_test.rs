//! Unit tests for `tests` split out of `output.rs`.
//! Kept as a separate file so they are easy to find; included via `#[path]`.

use super::*;

#[test]
fn a_file_counts_once_in_the_first_matching_bucket() {
    let files = vec![
        FileInfo {
            rel_path: "src/main.rs".into(),
            size: 0,
            mod_unix: 0,
        },
        FileInfo {
            rel_path: "README.MD".into(),
            size: 0,
            mod_unix: 0,
        },
        FileInfo {
            rel_path: "Cargo.toml".into(),
            size: 0,
            mod_unix: 0,
        },
    ];
    let fc = count_file_types(&files);
    assert_eq!(
        (fc.source_code, fc.documentation, fc.configuration),
        (1, 1, 1)
    );
}

#[test]
fn classification_needs_frontend_and_backend_for_full_stack() {
    let hit = |n: &str| FrameworkHit {
        framework: n.to_owned(),
        confidence: "high".to_owned(),
    };
    assert_eq!(
        project_classification(&[hit("react"), hit("django")]),
        "full-stack"
    );
    assert_eq!(
        project_classification(&[hit("react"), hit("go")]),
        "multi-framework"
    );
}
