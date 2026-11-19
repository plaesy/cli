use super::*;

#[test]
fn approval_secret_redacts_values_from_logs() {
    let mut store = SecretStore::new();
    store.insert("api_key", "sk-synthetic-123456");
    let log_line = "call provider with sk-synthetic-123456 and retry";
    let redacted = store.redact(log_line);
    assert!(!redacted.contains("sk-synthetic-123456"));
    assert!(redacted.contains("[REDACTED:api_key]"));
}

#[test]
fn approval_secret_debug_output_hides_values() {
    let mut store = SecretStore::new();
    store.insert("api_key", "sk-synthetic-123456");
    let debug = format!("{store:?}");
    assert!(debug.contains("api_key"));
    assert!(!debug.contains("sk-synthetic-123456"));
}

#[test]
fn approval_secret_overlapping_values_do_not_leak_suffix() {
    let mut store = SecretStore::new();
    store.insert("short", "abc");
    store.insert("long", "abcdef");
    let redacted = store.redact("token=abcdef");
    assert!(!redacted.contains("def"));
}
