//! Unit tests for `comments`, kept beside the module and included via `#[path]`.

use super::*;
use crate::tasks::{tasks_dir, TASK_FILE};
use std::fs;

fn fixture() -> (tempfile::TempDir, std::path::PathBuf) {
    let root = tempfile::tempdir().unwrap();
    let tasks = tasks_dir(root.path());
    let folder = tasks.join("todo").join("task-a");
    fs::create_dir_all(&folder).unwrap();
    fs::write(folder.join(TASK_FILE), "---\ntitle: test\n---\n").unwrap();
    (root, tasks)
}

fn comments_file(tasks: &std::path::Path) -> std::path::PathBuf {
    tasks.join("todo").join("task-a").join(COMMENTS_FILE)
}

#[test]
fn extract_mentions_keeps_distinct_names_in_order() {
    assert_eq!(
        extract_mentions("Halo @ana, cek @bima-k dan @ana lagi. Surel a@b.com; @ada. @. @ ujung @"),
        vec!["ana", "bima-k", "ada"]
    );
}

#[test]
fn extract_mentions_is_empty_without_names() {
    assert!(extract_mentions("tanpa mention, hanya teks").is_empty());
    assert!(extract_mentions("@").is_empty());
    assert!(extract_mentions("").is_empty());
}

#[test]
fn added_comments_round_trip_through_the_file() {
    let (_root, tasks) = fixture();
    let first = add_comment(&tasks, "task-a", "  Tolong cek @ana dan @bima.  ").unwrap();
    let second = add_comment(&tasks, "task-a", "Sudah, @ana").unwrap();

    assert_eq!(first.id, 1);
    assert_eq!(first.body, "Tolong cek @ana dan @bima.");
    assert_eq!(first.mentions, vec!["ana", "bima"]);
    assert!(!first.created_at.is_empty());
    assert_eq!(second.id, 2);
    assert_eq!(second.mentions, vec!["ana"]);

    let listed = list_comments(&tasks, "task-a").unwrap();
    assert_eq!(listed, vec![first.clone(), second.clone()]);

    let text = fs::read_to_string(comments_file(&tasks)).unwrap();
    assert_eq!(text.lines().count(), 2);
    let decoded: Vec<Comment> = text
        .lines()
        .map(|line| serde_json::from_str::<Comment>(line).unwrap())
        .collect();
    assert_eq!(decoded, vec![first, second]);
}

#[test]
fn removing_a_comment_keeps_the_others() {
    let (_root, tasks) = fixture();
    add_comment(&tasks, "task-a", "pertama").unwrap();
    let kept = add_comment(&tasks, "task-a", "kedua @rina").unwrap();

    remove_comment(&tasks, "task-a", 1).unwrap();

    assert_eq!(list_comments(&tasks, "task-a").unwrap(), vec![kept]);
    assert!(matches!(
        remove_comment(&tasks, "task-a", 1),
        Err(TasksError::CommentNotFound { id: 1, .. })
    ));
}

#[test]
fn blank_text_and_unknown_task_are_rejected() {
    let (_root, tasks) = fixture();
    assert!(matches!(
        add_comment(&tasks, "task-a", "  \n "),
        Err(TasksError::EmptyComment)
    ));
    assert!(matches!(
        add_comment(&tasks, "tidak-ada", "halo"),
        Err(TasksError::NotFound(name)) if name == "tidak-ada"
    ));
    assert!(matches!(
        list_comments(&tasks, "tidak-ada"),
        Err(TasksError::NotFound(_))
    ));
}

#[test]
fn append_starts_a_new_line_when_the_file_lacks_a_final_newline() {
    let (_root, tasks) = fixture();
    fs::write(
        comments_file(&tasks),
        r#"{"id":1,"body":"lama","mentions":[],"created_at":"2026-11-23T00:00:00Z"}"#,
    )
    .unwrap();

    let added = add_comment(&tasks, "task-a", "baru").unwrap();

    assert_eq!(added.id, 2);
    let listed = list_comments(&tasks, "task-a").unwrap();
    assert_eq!(listed.len(), 2);
    assert_eq!(listed[0].body, "lama");
    assert_eq!(listed[1], added);
}

#[test]
fn corrupt_line_is_reported_with_its_number() {
    let (_root, tasks) = fixture();
    fs::write(
        comments_file(&tasks),
        "{\"id\":1,\"body\":\"ok\",\"mentions\":[],\"created_at\":\"x\"}\nbukan json\n",
    )
    .unwrap();

    let error = list_comments(&tasks, "task-a").unwrap_err();

    assert!(error.to_string().contains("line 2"), "{error}");
}

#[test]
fn concurrent_comments_get_distinct_ids() {
    use std::sync::Arc;
    use std::thread;

    let (_root, tasks) = fixture();
    let tasks = Arc::new(tasks);
    let handles: Vec<_> = ["satu", "dua", "tiga"]
        .into_iter()
        .map(|body| {
            let tasks = Arc::clone(&tasks);
            thread::spawn(move || add_comment(&tasks, "task-a", body).unwrap())
        })
        .collect();
    for handle in handles {
        handle.join().unwrap();
    }

    let mut ids: Vec<u64> = list_comments(&tasks, "task-a")
        .unwrap()
        .into_iter()
        .map(|comment| comment.id)
        .collect();
    ids.sort_unstable();
    assert_eq!(ids, vec![1, 2, 3]);
}
