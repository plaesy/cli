//! Unit tests for `chat_learn.rs`, included via `#[path]`.

use super::*;

#[test]
fn a_reply_with_text_around_the_object_is_parsed() {
    let reply =
        "Berikut hasilnya:\n{\"lessons\":[\"pakai tes unit\",\"\"],\"skill\":null}\nSelesai.";
    let proposal = parse(reply).expect("parsed");
    assert_eq!(proposal.lessons, vec!["pakai tes unit".to_owned()]);
    assert!(proposal.skill.is_none());
}

#[test]
fn a_skill_is_read_with_its_fields() {
    let reply = r#"{"lessons":[],"skill":{"name":"tes-parser","description":"cara uji parser","body":"1. tulis tes"}}"#;
    let proposal = parse(reply).expect("parsed");
    assert_eq!(
        proposal.skill,
        Some(SkillDraft {
            name: "tes-parser".to_owned(),
            description: "cara uji parser".to_owned(),
            body: "1. tulis tes".to_owned(),
        })
    );
}

#[test]
fn a_reply_without_json_is_an_error() {
    assert!(parse("tidak ada apa-apa").is_err());
    assert!(parse("} sebelum {").is_err());
}

#[test]
fn write_skill_creates_then_replaces_the_file() {
    let temp = tempfile::tempdir().expect("tempdir");
    let draft = SkillDraft {
        name: "tes-parser".to_owned(),
        description: "cara uji".to_owned(),
        body: "langkah".to_owned(),
    };
    assert_eq!(write_skill(temp.path(), &draft), Ok(false));
    let text =
        std::fs::read_to_string(temp.path().join("tes-parser").join("SKILL.md")).expect("read");
    assert!(
        text.contains("name: tes-parser") && text.contains("langkah"),
        "{text}"
    );
    assert_eq!(write_skill(temp.path(), &draft), Ok(true));
}

#[test]
fn write_skill_refuses_a_bad_name_or_an_empty_body() {
    let temp = tempfile::tempdir().expect("tempdir");
    let bad = SkillDraft {
        name: "../luar".to_owned(),
        description: String::new(),
        body: "x".to_owned(),
    };
    assert!(write_skill(temp.path(), &bad).is_err());
    let empty = SkillDraft {
        name: "ok-name".to_owned(),
        description: String::new(),
        body: String::new(),
    };
    assert!(write_skill(temp.path(), &empty).is_err());
}
