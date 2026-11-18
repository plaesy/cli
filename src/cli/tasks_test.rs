//! Unit tests for `board_tests` split out of `tasks.rs`.
//! Kept as a separate file so they are easy to find; included via `#[path]`.

#[test]
fn extracts_goal_paragraph_and_ignores_legacy_tasks_without_goal() {
    assert_eq!(
        super::goal_summary(
            "# Title\n\n## Goal\nMake the user outcome\nacross two lines.\n\n## Scope\nDetails."
        ),
        Some("Make the user outcome across two lines.".to_owned())
    );
    assert_eq!(
        super::goal_summary("# Legacy task\n\nNo goal section."),
        None
    );
    assert_eq!(
        super::goal_summary("## Goal\nDescribe the observable outcome this task should achieve."),
        None
    );
    assert_eq!(
        super::goal_summary("## Goal\nThe user can complete sign-in."),
        Some("The user can complete sign-in.".to_owned())
    );
}

#[test]
fn detects_an_unfilled_goal_template() {
    assert!(super::goal_is_unfilled(
        "## Goal\nDescribe the observable outcome this task should achieve."
    ));
    assert!(super::goal_is_unfilled(
        "## Goal\n\n## Scope\nIncluded work."
    ));
    assert!(!super::goal_is_unfilled(
        "## Goal\nThe user can complete sign-in."
    ));
}
