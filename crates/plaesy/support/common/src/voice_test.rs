//! Unit tests for `voice.rs`. Kept as a separate file; included via `#[path]`.

use super::*;

fn create(title: &str) -> VoiceCommand {
    VoiceCommand::CreateTask {
        title: title.to_string(),
    }
}

fn complete(title: &str) -> VoiceCommand {
    VoiceCommand::CompleteTask {
        title: title.to_string(),
    }
}

#[test]
fn create_task_indonesian_and_english() {
    assert_eq!(parse("buat tugas beli susu"), Ok(create("beli susu")));
    assert_eq!(parse("tambahkan tugas kirim laporan"), Ok(create("kirim laporan")));
    assert_eq!(parse("add task buy milk"), Ok(create("buy milk")));
    assert_eq!(parse("new task write tests"), Ok(create("write tests")));
}

#[test]
fn create_task_keeps_case_and_inner_punctuation() {
    assert_eq!(
        parse("Buat tugas: Kirim laporan, besok."),
        Ok(create("Kirim laporan, besok"))
    );
    assert_eq!(parse("BUAT TUGAS Beli Susu"), Ok(create("Beli Susu")));
}

#[test]
fn create_task_skips_filler_and_connectors() {
    assert_eq!(parse("tolong buat tugas beli roti"), Ok(create("beli roti")));
    assert_eq!(parse("Plaesy, buat tugas beli roti"), Ok(create("beli roti")));
    assert_eq!(parse("buat tugas untuk membeli susu"), Ok(create("membeli susu")));
    assert_eq!(parse("add task to buy milk"), Ok(create("buy milk")));
    assert_eq!(parse("buat tugas baru beli kopi"), Ok(create("beli kopi")));
}

#[test]
fn create_task_title_may_contain_command_words() {
    assert_eq!(parse("buat tugas tandai selesai"), Ok(create("tandai selesai")));
}

#[test]
fn complete_task_prefix_forms() {
    assert_eq!(parse("selesaikan tugas beli susu"), Ok(complete("beli susu")));
    assert_eq!(parse("tandai selesai beli susu"), Ok(complete("beli susu")));
    assert_eq!(parse("complete task buy milk"), Ok(complete("buy milk")));
    assert_eq!(parse("finish task write tests"), Ok(complete("write tests")));
}

#[test]
fn complete_task_suffix_forms() {
    assert_eq!(
        parse("tandai tugas beli susu sebagai selesai"),
        Ok(complete("beli susu"))
    );
    assert_eq!(parse("tandai beli susu selesai"), Ok(complete("beli susu")));
    assert_eq!(parse("mark task buy milk as done"), Ok(complete("buy milk")));
    assert_eq!(parse("mark buy milk done"), Ok(complete("buy milk")));
}

#[test]
fn list_tasks_variants() {
    for text in [
        "daftar tugas",
        "lihat tugas",
        "tampilkan tugas",
        "tampilkan semua tugas",
        "tunjukkan semua tugas",
        "list tasks",
        "show all tasks",
        "Show tasks",
    ] {
        assert_eq!(parse(text), Ok(VoiceCommand::ListTasks), "input: {text}");
    }
}

#[test]
fn list_rejects_trailing_words() {
    assert_eq!(
        parse("daftar tugas hari ini"),
        Err(VoiceError::Unrecognized("daftar tugas hari ini".into()))
    );
}

#[test]
fn empty_and_filler_only_input_is_empty() {
    assert_eq!(parse(""), Err(VoiceError::Empty));
    assert_eq!(parse("   "), Err(VoiceError::Empty));
    assert_eq!(parse("tolong"), Err(VoiceError::Empty));
    assert_eq!(parse("... ?"), Err(VoiceError::Empty));
}

#[test]
fn unrelated_sentence_is_unrecognized() {
    assert_eq!(
        parse("  cuaca hari ini  "),
        Err(VoiceError::Unrecognized("cuaca hari ini".into()))
    );
}

#[test]
fn verb_without_task_noun_is_unrecognized() {
    assert_eq!(
        parse("add salt to the soup"),
        Err(VoiceError::Unrecognized("add salt to the soup".into()))
    );
}

#[test]
fn recognized_verb_without_title_reports_missing_title() {
    assert_eq!(
        parse("buat tugas"),
        Err(VoiceError::MissingTitle {
            action: "buat tugas"
        })
    );
    assert_eq!(
        parse("buat tugas untuk"),
        Err(VoiceError::MissingTitle {
            action: "buat tugas"
        })
    );
    assert_eq!(
        parse("tandai selesai"),
        Err(VoiceError::MissingTitle {
            action: "tandai selesai"
        })
    );
    assert_eq!(
        parse("mark task as done"),
        Err(VoiceError::MissingTitle {
            action: "tandai selesai"
        })
    );
}

#[test]
fn command_serializes_with_action_tag() {
    let json = serde_json::to_string(&create("beli susu")).expect("serialize command");
    assert_eq!(json, r#"{"action":"create_task","title":"beli susu"}"#);
    let json = serde_json::to_string(&VoiceCommand::ListTasks).expect("serialize command");
    assert_eq!(json, r#"{"action":"list_tasks"}"#);
}

#[test]
fn error_messages_are_clear() {
    assert_eq!(
        VoiceError::Unrecognized("cuaca".into()).to_string(),
        "perintah tidak dikenali: \"cuaca\""
    );
    assert_eq!(
        VoiceError::MissingTitle {
            action: "buat tugas"
        }
        .to_string(),
        "perintah buat tugas membutuhkan judul tugas"
    );
}
