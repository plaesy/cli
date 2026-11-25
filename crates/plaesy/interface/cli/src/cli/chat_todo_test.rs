//! Unit tests for `chat_todo.rs`, included via `#[path]`.

use super::*;

#[test]
fn an_empty_list_says_so_and_add_appends_numbered_items() {
    let mut todos = Todos::default();
    assert_eq!(todos.apply(""), "daftar kosong");
    assert_eq!(todos.apply("add tulis tes"), "1. [ ] tulis tes");
    assert_eq!(
        todos.apply("add jalankan clippy"),
        "1. [ ] tulis tes\n2. [ ] jalankan clippy"
    );
    assert_eq!(todos.len(), 2);
}

#[test]
fn done_and_undone_toggle_an_item_by_its_number() {
    let mut todos = Todos::default();
    todos.apply("add satu");
    todos.apply("add dua");
    assert_eq!(todos.apply("done 2"), "1. [ ] satu\n2. [x] dua");
    assert_eq!(todos.apply("undone 2"), "1. [ ] satu\n2. [ ] dua");
}

#[test]
fn numbers_outside_the_list_are_refused() {
    let mut todos = Todos::default();
    todos.apply("add satu");
    for arg in ["done 0", "done 2", "done abc", "rm 9"] {
        assert!(todos.apply(arg).starts_with("nomor tidak dikenal"), "{arg}");
    }
    assert_eq!(todos.len(), 1);
}

#[test]
fn rm_removes_an_item_and_clear_empties_the_list() {
    let mut todos = Todos::default();
    todos.apply("add satu");
    todos.apply("add dua");
    assert!(todos.apply("rm 1").starts_with("dihapus: satu"));
    assert_eq!(todos.render(), "1. [ ] dua");
    assert_eq!(todos.apply("clear"), "daftar dikosongkan");
    assert!(todos.is_empty());
}

#[test]
fn add_without_text_shows_usage() {
    let mut todos = Todos::default();
    assert!(todos.apply("add").starts_with("pakai:"));
    assert!(todos.is_empty());
}
