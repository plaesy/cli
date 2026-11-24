//! Unit tests for `tasks_board.rs`: column grouping, title truncation, and rendering.

use crate::tasks::{Status, TaskRef};

fn task(status: Status, name: &str, title: &str) -> TaskRef {
    TaskRef {
        spec: "demo".to_owned(),
        status,
        name: name.to_owned(),
        title: title.to_owned(),
        phase: String::new(),
    }
}

#[test]
fn columns_keep_every_status_in_lifecycle_order_even_when_empty() {
    let refs = vec![task(Status::Doing, "medium_feat-a", "A")];
    let columns = super::columns(&refs);
    let order: Vec<Status> = columns.iter().map(|(status, _)| *status).collect();
    assert_eq!(order, crate::tasks::STATUSES.to_vec());
    assert!(columns
        .iter()
        .filter(|(status, _)| *status != Status::Doing)
        .all(|(_, column)| column.is_empty()));
}

#[test]
fn columns_sort_by_priority_then_name_within_a_status() {
    let refs = vec![
        task(Status::Todo, "low_chore-z", ""),
        task(Status::Todo, "high_bug-b", ""),
        task(Status::Todo, "critical_bug-c", ""),
        task(Status::Todo, "high_bug-a", ""),
        task(Status::Done, "critical_bug-d", ""),
    ];
    let columns = super::columns(&refs);
    let todo: Vec<&str> = columns
        .iter()
        .find(|(status, _)| *status == Status::Todo)
        .map(|(_, column)| column.iter().map(|t| t.name.as_str()).collect::<Vec<&str>>())
        .unwrap_or_default();
    assert_eq!(
        todo,
        ["critical_bug-c", "high_bug-a", "high_bug-b", "low_chore-z"]
    );
}

#[test]
fn truncate_keeps_short_and_exact_length_titles_unchanged() {
    assert_eq!(super::truncate_title("Short", 10), "Short");
    assert_eq!(super::truncate_title("Exactly ten", 11), "Exactly ten");
}

#[test]
fn truncate_cuts_long_titles_to_the_width_with_an_ellipsis() {
    let cut = super::truncate_title("Build the kanban board for every spec", 21);
    assert_eq!(cut, "Build the kanban b...");
    assert_eq!(cut.chars().count(), 21);
}

#[test]
fn truncate_does_not_leave_a_space_before_the_ellipsis() {
    // Characters 1..=17 end in a space; the cut must not print "gamma ...".
    assert_eq!(
        super::truncate_title("alpha beta gamma delta", 20),
        "alpha beta gamma..."
    );
}

#[test]
fn truncate_counts_characters_not_bytes() {
    let title = "日本語のタイトルはとても長い";
    let cut = super::truncate_title(title, 8);
    assert_eq!(cut, "日本語のタ...");
    assert_eq!(cut.chars().count(), 8);
}

#[test]
fn truncate_handles_widths_that_cannot_hold_an_ellipsis() {
    assert_eq!(super::truncate_title("abcdef", 3), "abc");
    assert_eq!(super::truncate_title("abcdef", 1), "a");
    assert_eq!(super::truncate_title("abcdef", 0), "");
}

#[test]
fn truncate_collapses_whitespace_so_a_title_stays_on_one_line() {
    assert_eq!(super::truncate_title("a\n  b\tc", 10), "a b c");
}

#[test]
fn render_shows_counts_per_column_and_marks_empty_columns() {
    let refs = vec![
        task(Status::Backlog, "medium_feat-x", "Plan the release"),
        task(Status::Doing, "high_bug-y", "Fix the parser"),
        task(Status::Doing, "low_chore-z", "Tidy imports"),
    ];
    let out = super::render(&refs, 60);
    assert!(out.starts_with("Task Board (3 tasks)\n"));
    assert!(out.contains("\nbacklog (1)\n  - Plan the release (medium_feat-x)\n"));
    assert!(out.contains(
        "\ndoing (2)\n  - Fix the parser (high_bug-y)\n  - Tidy imports (low_chore-z)\n"
    ));
    assert!(out.contains("\ntodo (0)\n  (none)\n"));
    assert!(out.contains("\nblocked (0)\n  (none)\n"));
}

#[test]
fn render_falls_back_to_the_name_when_the_title_is_empty() {
    let refs = vec![task(Status::Todo, "medium_feat-untitled", "   ")];
    let out = super::render(&refs, 60);
    assert!(out.contains("  - medium_feat-untitled (medium_feat-untitled)\n"));
}

#[test]
fn render_cuts_long_titles_and_keeps_the_task_name() {
    let refs = vec![task(
        Status::Review,
        "medium_feat-long",
        "A very long title that goes past the width",
    )];
    let out = super::render(&refs, 15);
    assert!(out.contains("  - A very long... (medium_feat-long)\n"));
}

#[test]
fn render_empty_board_still_lists_all_columns() {
    let out = super::render(&[], 60);
    assert!(out.starts_with("Task Board (0 tasks)\n"));
    for status in crate::tasks::STATUSES {
        assert!(out.contains(&format!("\n{status} (0)\n")), "missing {status}");
    }
}
