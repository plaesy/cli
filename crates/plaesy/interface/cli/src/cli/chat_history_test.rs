//! Unit tests for `chat_history.rs`, included via `#[path]`.

use super::*;

fn turns() -> Vec<(String, String)> {
    vec![
        ("user".to_owned(), "Apa ibukota Indonesia?".to_owned()),
        ("assistant".to_owned(), "Jakarta.".to_owned()),
    ]
}

#[test]
fn a_saved_session_loads_back_as_the_same_turns() {
    let temp = tempfile::tempdir().expect("tempdir");
    let path = save(temp.path(), "chat-1.jsonl", &turns()).expect("save");
    assert_eq!(load(&path).expect("load"), turns());
}

#[test]
fn load_skips_lines_that_are_not_turns() {
    let temp = tempfile::tempdir().expect("tempdir");
    let path = temp.path().join("rusak.jsonl");
    fs::write(
        &path,
        "bukan json\n{\"role\":\"system\",\"text\":\"x\"}\n{\"role\":\"user\",\"text\":\"ok\"}\n",
    )
    .expect("write");
    assert_eq!(
        load(&path).expect("load"),
        vec![("user".to_owned(), "ok".to_owned())]
    );
}

#[test]
fn search_finds_turns_case_insensitively_with_their_line_number() {
    let temp = tempfile::tempdir().expect("tempdir");
    save(temp.path(), "chat-1.jsonl", &turns()).expect("save");
    let hits = search(temp.path(), "JAKARTA");
    assert_eq!(
        hits,
        vec![Hit {
            file: "chat-1.jsonl".to_owned(),
            line: 2,
            snippet: "Jakarta.".to_owned(),
        }]
    );
    assert!(search(temp.path(), "surabaya").is_empty());
    assert!(search(temp.path(), "  ").is_empty());
}

#[test]
fn search_of_a_missing_folder_is_empty_not_an_error() {
    let temp = tempfile::tempdir().expect("tempdir");
    assert!(search(&temp.path().join("tidak-ada"), "x").is_empty());
}

#[test]
fn new_names_are_chat_files() {
    let name = new_name();
    assert!(
        name.starts_with("chat-") && name.ends_with(".jsonl"),
        "{name}"
    );
}

#[test]
fn list_is_newest_first_with_turn_counts_and_the_first_line() {
    let temp = tempfile::tempdir().expect("tempdir");
    save(temp.path(), "chat-1.jsonl", &turns()).expect("save");
    save(temp.path(), "chat-2.jsonl", &turns()[..1]).expect("save");
    let saved = list(temp.path());
    assert_eq!(saved.len(), 2);
    assert_eq!(saved[0].name, "chat-2.jsonl");
    assert_eq!(saved[0].turns, 1);
    assert_eq!(saved[1].turns, 2);
    assert_eq!(saved[1].first, "Apa ibukota Indonesia?");
}

#[test]
fn delete_removes_one_file_and_refuses_paths() {
    let temp = tempfile::tempdir().expect("tempdir");
    save(temp.path(), "chat-1.jsonl", &turns()).expect("save");
    assert!(delete(temp.path(), "../chat-1.jsonl").is_err());
    assert!(delete(temp.path(), "catatan.txt").is_err());
    delete(temp.path(), "chat-1.jsonl").expect("delete");
    assert!(list(temp.path()).is_empty());
}

#[test]
fn clear_removes_every_saved_session_and_counts_them() {
    let temp = tempfile::tempdir().expect("tempdir");
    save(temp.path(), "chat-1.jsonl", &turns()).expect("save");
    save(temp.path(), "chat-2.jsonl", &turns()).expect("save");
    fs::write(temp.path().join("lain.txt"), "x").expect("write");
    assert_eq!(clear(temp.path()), Ok(2));
    assert!(temp.path().join("lain.txt").exists());
    assert_eq!(clear(&temp.path().join("tidak-ada")), Ok(0));
}
