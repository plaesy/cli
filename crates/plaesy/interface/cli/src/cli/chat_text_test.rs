use super::*;

#[test]
fn lang_parse_accepts_codes_and_names_and_rejects_others() {
    assert_eq!(Lang::parse("en"), Ok(Lang::En));
    assert_eq!(Lang::parse(" ID "), Ok(Lang::Id));
    assert_eq!(Lang::parse("Indonesia"), Ok(Lang::Id));
    assert!(Lang::parse("fr").is_err());
}

#[test]
fn resolve_prefers_flag_then_settings_then_english() {
    let dir = tempfile::tempdir().expect("tempdir");
    let settings = dir.path().join("settings.json");

    assert_eq!(
        resolve(None, dir.path()),
        Ok(Lang::En),
        "no file means English"
    );

    std::fs::write(&settings, r#"{"language":"id"}"#).expect("write");
    assert_eq!(resolve(None, dir.path()), Ok(Lang::Id));
    assert_eq!(resolve(Some("en"), dir.path()), Ok(Lang::En), "flag wins");

    std::fs::write(&settings, r#"{"language":"klingon"}"#).expect("write");
    assert!(
        resolve(None, dir.path()).is_err(),
        "unknown value is an error"
    );
}

#[test]
fn usage_follows_the_language() {
    assert!(usage(Lang::En, Usage::Ask).starts_with("usage: /ask <question>"));
    assert!(usage(Lang::Id, Usage::Ask).starts_with("usage: /ask <pertanyaan>"));
    assert_eq!(
        usage(Lang::En, Usage::Provider),
        usage(Lang::Id, Usage::Provider)
    );
}

#[test]
fn key_messages_are_translated_for_both_languages() {
    assert_eq!(history_cleared(Lang::En), "history cleared");
    assert_eq!(history_cleared(Lang::Id), "riwayat dihapus");
    assert_eq!(no_pending(Lang::En), "no pending questions");
    assert_eq!(no_pending(Lang::Id), "tidak ada pertanyaan terbuka");
    assert_eq!(answered(Lang::En, "d1", "b"), "d1 answered: b");
    assert_eq!(answered(Lang::Id, "d1", "b"), "d1 dijawab: b");
    assert_eq!(action_archive(Lang::En), "Archive");
    assert_eq!(action_archive(Lang::Id), "Arsipkan");
}

#[test]
fn an_empty_reply_gets_a_visible_notice_in_both_languages() {
    assert!(empty_reply(Lang::En).contains("no text"));
    assert!(empty_reply(Lang::Id).contains("tidak mengembalikan teks"));
}
