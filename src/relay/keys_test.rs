use super::*;

#[test]
fn shared_messaging_static_key_source_returns_known_keys_only() {
    let mut source = StaticKeySource::new();
    source.insert("k1", b"secret-1");
    assert_eq!(source.key("k1"), Some(b"secret-1".to_vec()));
    assert_eq!(source.key("k2"), None);
}
