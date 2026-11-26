//! Unit tests for `notify.rs`, included via `#[path]`.

use super::*;

fn sample(read_at: Option<u64>) -> Notification {
    Notification {
        id: "n-1".into(),
        kind: Kind::ReadyToMerge,
        session: "s-1".into(),
        message: "PR ready".into(),
        created_at: 100,
        read_at,
    }
}

#[test]
fn parse_kind_accepts_the_four_names_and_rejects_others() {
    assert_eq!(parse_kind("merged"), Ok(Kind::Merged));
    let err = parse_kind("push").unwrap_err();
    assert!(err.contains("input-needed"), "{err}");
}

#[test]
fn format_line_shows_read_state_and_message() {
    assert!(format_line(&sample(None)).contains("unread"));
    let read = format_line(&sample(Some(101)));
    assert!(read.starts_with("n-1 [ready-to-merge] read "), "{read}");
    assert!(read.ends_with("PR ready"));
}

#[test]
fn target_needs_exactly_one_of_id_or_all() {
    assert_eq!(target(Some("n-1"), false), Ok(Target::One("n-1")));
    assert_eq!(target(None, true), Ok(Target::All));
    assert!(target(None, false).is_err());
    assert!(target(Some("n-1"), true).is_err());
}
