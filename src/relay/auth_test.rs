use super::*;

#[test]
fn shared_messaging_hmac_matches_rfc4231_vector() {
    let key = [0x0bu8; 20];
    let tag = to_hex(&hmac_sha256(&key, b"Hi There"));
    assert_eq!(
        tag,
        "b0344c61d8db38535ca8afceaf0bf12b881dc200c9833da726e9376c2e32cff7"
    );
}

#[test]
fn shared_messaging_envelope_round_trips_and_rejects_tampering() {
    let key = b"deployment-key";
    let envelope = seal(
        key,
        "k1",
        "agent-1",
        "project-a",
        "r-1",
        1000,
        "n-1",
        "hello | world",
    );
    let opened = open(key, &envelope).unwrap();
    assert_eq!(opened.body, "hello | world");
    assert_eq!(opened.project, "project-a");
    assert_eq!(opened.recipient, "r-1");
    let tampered = envelope.replace("project-a", "project-b");
    assert_eq!(open(key, &tampered), Err(AuthError::BadTag));
    assert_eq!(open(b"other-key", &envelope), Err(AuthError::BadTag));
}

#[test]
fn shared_messaging_replayed_nonce_is_rejected() {
    let key = b"deployment-key";
    let envelope = seal(key, "k1", "agent-1", "project-a", "r-1", 1000, "n-1", "hi");
    let opened = open(key, &envelope).unwrap();
    let mut guard = ReplayGuard::new(300);
    assert_eq!(guard.accept(&opened, 1000), Ok(()));
    assert_eq!(guard.accept(&opened, 1001), Err(AuthError::Replayed));
}

#[test]
fn shared_messaging_stale_envelope_is_expired() {
    let key = b"deployment-key";
    let envelope = seal(key, "k1", "agent-1", "project-a", "r-1", 1000, "n-2", "hi");
    let opened = open(key, &envelope).unwrap();
    let mut guard = ReplayGuard::new(300);
    assert_eq!(guard.accept(&opened, 1000 + 301), Err(AuthError::Expired));
}

#[test]
fn shared_messaging_recipient_is_signed() {
    let key = b"deployment-key";
    let envelope = seal(key, "k1", "agent-1", "project-a", "r-1", 1000, "n-3", "hi");
    let redirected = envelope.replace(
        "
r-1
", "
r-2
",
    );
    assert_eq!(open(key, &redirected), Err(AuthError::BadTag));
}

#[test]
fn shared_messaging_peek_key_id_reads_key_before_verifying() {
    let envelope = seal(b"k", "k9", "agent-1", "project-a", "r-1", 1000, "n-4", "hi");
    assert_eq!(peek_key_id(&envelope), Some("k9"));
}
