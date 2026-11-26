//! Unit tests for the project selector order in `projects.rs`.

use super::pick_selector;

#[test]
fn projects_flag_wins_over_environment() {
    assert_eq!(
        pick_selector(Some("p-flag"), Some("p-env")).as_deref(),
        Some("p-flag")
    );
}

#[test]
fn projects_environment_is_used_when_the_flag_is_absent() {
    assert_eq!(pick_selector(None, Some("p-env")).as_deref(), Some("p-env"));
}

#[test]
fn projects_blank_values_are_absent() {
    assert_eq!(
        pick_selector(Some("  "), Some("p-env")).as_deref(),
        Some("p-env")
    );
    assert_eq!(pick_selector(Some(""), Some(" ")), None);
}

#[test]
fn projects_no_selector_falls_through_to_the_working_directory() {
    assert_eq!(pick_selector(None, None), None);
}

#[test]
fn projects_values_are_trimmed() {
    assert_eq!(
        pick_selector(Some(" p-flag "), None).as_deref(),
        Some("p-flag")
    );
}
