//! Unit tests for `tasks_board.rs`: column grouping, title truncation, and rendering.
//! Kept as a separate file so they are easy to find; included via `#[path]`.

use super::*;

fn task(status: Status, name: &str, title: &str) -> TaskRef {
    TaskRef {
        spec: "spec".into(),
        status,
        name: name.into(),
        title: title.into(),
        phase: String::new(),
    }
}

#[test]
fn every_status_gets_a_column_in_lifecycle_order_even_when_empty() {
    let columns = group_by_status(&[]);
    let order: Vec<Status> = columns.iter().map(|(status, _)| *status).collect();
    assert_eq!(order, STATUSES.to_vec());
    assert!(columns.iter().all(|(_, tasks)| tasks.is_empty()));
}

#[test]
fn tasks_go_to_their_own_status_column() {
    let refs = vec![
        task(Status::Backlog, "high_feature-a", "A"),
        task(Status::Doing, "high_feature-b", "B"),
        task(Status::Doing, "low_chore-c", "C"),
        task(Status::Blocked, "medium_chore-d", "D"),
    ];
    let columns = group_by_status(&refs);
    let counts: Vec<(Status, usize)> = columns
        .iter()
        .map(|(status, tasks)| (*status, tasks.len()))
        .collect();
    assert_eq!(
        counts,
        vec![
            (Status::Backlog, 1),
            (Status::Todo, 0),
            (Status::Doing, 2),
            (Status::Review, 0),
            (Status::Done, 0),
            (Status::Blocked, 1),
        ]
    );
}

#[test]
fn column_orders_by_priority_and_keeps_name_order_on_ties() {
    let refs = vec![
        task(Status::Todo, "low_chore-z", "Low"),
        task(Status::Todo, "critical_chore-b", "Critical B"),
        task(Status::Todo, "high_chore-a", "High"),
        task(Status::Todo, "critical_chore-a", "Critical A"),
    ];
    let columns = group_by_status(&refs);
    let todo: Vec<&str> = columns[1].1.iter().map(|r| r.name.as_str()).collect();
    assert_eq!(
        todo,
        vec![
            "critical_chore-a",
            "critical_chore-b",
            "high_chore-a",
            "low_chore-z",
        ]
    );
}

#[test]
fn short_titles_are_returned_trimmed_and_unchanged() {
    assert_eq!(truncate_title("  Short title  ", 60), "Short title");
    assert_eq!(truncate_title("exactly", 7), "exactly");
    assert_eq!(truncate_title("", 5), "");
}

#[test]
fn long_titles_are_cut_to_the_limit_with_an_ellipsis() {
    let cut = truncate_title("abcdefghijklmnop", 10);
    assert_eq!(cut, "abcdefghi…");
    assert_eq!(cut.chars().count(), 10);
}

#[test]
fn truncate_collapses_whitespace_so_a_title_stays_on_one_line() {
    assert_eq!(truncate_title("a\n  b\tc", 10), "a b c");
}

#[test]
fn truncate_does_not_leave_a_space_before_the_ellipsis() {
    // Character 17 is a space; the cut must not print "gamma …".
    assert_eq!(
        truncate_title("alpha beta gamma delta", 18),
        "alpha beta gamma…"
    );
}

#[test]
fn truncation_counts_characters_not_bytes() {
    // Each `é` is two bytes; a byte-based cut would split a character and panic.
    let cut = truncate_title("ééééé", 3);
    assert_eq!(cut, "éé…");
    assert_eq!(cut.chars().count(), 3);
}

#[test]
fn zero_limit_yields_empty_text() {
    assert_eq!(truncate_title("anything", 0), "");
}

#[test]
fn card_shows_the_name_when_the_title_is_empty() {
    let refs = vec![task(Status::Backlog, "high_chore-no-title", "   ")];
    let lines = render_board(&refs);
    assert!(lines.iter().any(|line| line == "  - high_chore-no-title"));
}

#[test]
fn board_header_and_column_counts_are_rendered() {
    let refs = vec![
        task(Status::Backlog, "high_feature-a", "Alpha"),
        task(Status::Done, "low_chore-b", "Beta"),
    ];
    let lines = render_board(&refs);
    assert_eq!(lines[0], "📋 Task Board (2 tasks)");
    assert!(lines.contains(&"backlog (1)".to_owned()));
    assert!(lines.contains(&"todo (0)".to_owned()));
    assert!(lines.contains(&"done (1)".to_owned()));
    assert!(lines.contains(&"  - Alpha".to_owned()));
    assert!(lines.contains(&"    high_feature-a".to_owned()));
}

#[test]
fn long_card_titles_are_truncated_in_the_board() {
    let long = "x".repeat(MAX_TITLE_CHARS + 20);
    let refs = vec![task(Status::Doing, "high_feature-long", &long)];
    let lines = render_board(&refs);
    let expected = format!("  - {}…", "x".repeat(MAX_TITLE_CHARS - 1));
    assert!(lines.contains(&expected));
    assert!(!lines.iter().any(|line| line.contains(&long)));
}

#[test]
fn missing_tasks_folder_runs_without_error() {
    let root = tempfile::tempdir().unwrap();
    let missing = root.path().join(".plaesy").join("tasks");
    assert!(run(&missing).is_ok());
}
