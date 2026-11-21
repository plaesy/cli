//! Unit tests for `tests` split out of `detect.rs`.
//! Kept as a separate file so they are easy to find; included via `#[path]`.

use super::*;

fn hit(name: &str) -> FrameworkHit {
    FrameworkHit {
        framework: name.to_owned(),
        confidence: "high".to_owned(),
    }
}

#[test]
fn the_primary_type_follows_the_priority_list() {
    let hits = vec![hit("go"), hit("react")];
    assert_eq!(detect_project_type(&hits).framework, "react");
    assert_eq!(detect_project_type(&[]).framework, "generic");
}

#[test]
fn languages_default_to_javascript() {
    assert_eq!(
        detect_all_languages(&["README.txt".to_owned()]),
        vec!["JavaScript".to_owned()]
    );
    assert_eq!(
        detect_all_languages(&["a/main.rs".to_owned()]),
        vec!["Rust".to_owned()]
    );
}
