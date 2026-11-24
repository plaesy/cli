use super::*;
use crate::secrets::SecretStore;

#[test]
fn redact_url_password_keeps_host_and_masks_password() {
    let out = redact_text("postgres://app:s3cret-pass@db.local:5432/x");
    assert_eq!(out, "postgres://app:[REDACTED:url_password]@db.local:5432/x");
}

#[test]
fn redact_query_token_keeps_other_parameters() {
    let out = redact_text("https://api.example.com/v1?token=abc123XYZ&page=2");
    assert_eq!(
        out,
        "https://api.example.com/v1?token=[REDACTED:token]&page=2"
    );
}

#[test]
fn redact_authorization_header_with_bearer_scheme() {
    let out = redact_text("Authorization: Bearer eyJhbGciOi.payload");
    assert_eq!(out, "Authorization: Bearer [REDACTED:authorization]");
}

#[test]
fn redact_authorization_header_with_basic_scheme() {
    let out = redact_text("Authorization: Basic dXNlcjpwYXNz");
    assert_eq!(out, "Authorization: Basic [REDACTED:authorization]");
}

#[test]
fn redact_api_key_header_value() {
    let out = redact_text("X-Api-Key: plainvalue123");
    assert_eq!(out, "X-Api-Key: [REDACTED:api-key]");
}

#[test]
fn redact_json_password_field() {
    let out = redact_text(r#"{"password": "hunter2hunter", "user": "budi"}"#);
    assert!(!out.contains("hunter2hunter"));
    assert!(out.contains("[REDACTED:password]"));
    assert!(out.contains(r#""user": "budi""#));
}

#[test]
fn redact_standalone_bearer_token() {
    let out = redact_text("curl with bearer abcdefghijklmnopqrstu123");
    assert_eq!(out, "curl with bearer [REDACTED:bearer_token]");
}

#[test]
fn redact_github_token_by_prefix() {
    let out = redact_text("key ghp_abcdefghijklmnopqrstuvwxyz0123");
    assert_eq!(out, "key [REDACTED:github_token]");
}

#[test]
fn redact_anthropic_key_prefers_specific_kind() {
    let text = format!("pakai sk-ant-api03-{}", "abcdefghijklmnopqrstuvwxyz0123");
    let out = redact_text(&text);
    assert_eq!(out, "pakai [REDACTED:anthropic_api_key]");
}

#[test]
fn redact_keeps_line_breaks() {
    let out = redact_text("line one\npassword=abc123\nline three\n");
    assert_eq!(out, "line one\npassword=[REDACTED:password]\nline three\n");
}

#[test]
fn redact_plain_text_is_unchanged() {
    let text = "Ringkasan rapat: jadwal dipindah ke Kamis, lihat dokumen untuk detail.";
    assert_eq!(redact_text(text), text);
}

#[test]
fn redact_plain_words_near_key_names_are_unchanged() {
    let text = "The tokenizer splits words. password reset flow dijelaskan di dokumen.";
    assert_eq!(redact_text(text), text);
}

#[test]
fn redact_key_without_wordspace_value_is_unchanged() {
    let text = "Set API key rotation every 90 days";
    assert_eq!(redact_text(text), text);
}

#[test]
fn redact_url_without_credentials_is_unchanged() {
    let text = "Lihat https://example.com/docs/page?lang=id untuk panduan.";
    assert_eq!(redact_text(text), text);
}

#[test]
fn redact_short_bearer_word_is_unchanged() {
    let text = "bearer of good news";
    assert_eq!(redact_text(text), text);
}

#[test]
fn redact_placeholders_are_unchanged() {
    for text in [
        "password=<PASSWORD>",
        "token=${TOKEN}",
        "password=[REDACTED:password]",
        "api_key=changeme",
    ] {
        assert_eq!(redact_text(text), text, "input: {text}");
    }
}

#[test]
fn redact_multibyte_text_does_not_split_characters() {
    let out = redact_text("catatan: café password=rahasiaküa selesai");
    assert_eq!(out, "catatan: café password=[REDACTED:password] selesai");
}

#[test]
fn redact_custom_rule_extends_default_set() {
    struct LiteralRule;
    impl RedactionRule for LiteralRule {
        fn find(&self, line: &str) -> Vec<RedactionSpan> {
            line.match_indices("RAHASIA-X")
                .map(|(start, needle)| RedactionSpan {
                    range: start..start + needle.len(),
                    kind: "custom".to_owned(),
                })
                .collect()
        }
    }
    let mut redactor = Redactor::with_defaults();
    redactor.push(Box::new(LiteralRule));
    assert_eq!(
        redactor.redact("abc RAHASIA-X def password=xyz"),
        "abc [REDACTED:custom] def password=[REDACTED:password]"
    );
}

#[test]
fn redact_custom_token_pattern_is_used() {
    let mut redactor = Redactor::new();
    redactor.push(Box::new(ProviderTokenRule::new(vec![TokenPattern::new(
        "internal_token",
        "ITK_",
        8,
    )])));
    assert_eq!(
        redactor.redact("ITK_abcdefgh12 dan ITK_short"),
        "[REDACTED:internal_token] dan ITK_short"
    );
}

#[test]
fn redact_overlapping_spans_do_not_leak_suffix() {
    struct FixedSpans;
    impl RedactionRule for FixedSpans {
        fn find(&self, _line: &str) -> Vec<RedactionSpan> {
            vec![
                RedactionSpan {
                    range: 0..5,
                    kind: "first".to_owned(),
                },
                RedactionSpan {
                    range: 3..9,
                    kind: "second".to_owned(),
                },
            ]
        }
    }
    let mut redactor = Redactor::new();
    redactor.push(Box::new(FixedSpans));
    // Rentang kedua melebar sampai indeks 9, jadi hanya "j" yang tersisa di luar penanda.
    assert_eq!(redactor.redact("abcdefghij"), "[REDACTED:first]j");
}

#[test]
fn redact_all_applies_stored_values_and_patterns() {
    let mut store = SecretStore::new();
    store.insert("api_key", "value-xyz-123");
    assert_eq!(
        store.redact_all("value-xyz-123 and password=hunter2"),
        "[REDACTED:api_key] and password=[REDACTED:password]"
    );
}
