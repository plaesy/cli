use super::*;

#[test]
fn shared_messaging_static_key_source_returns_known_keys_only() {
    let mut source = StaticKeySource::new();
    source.insert("k1", b"secret-1");
    assert_eq!(source.key("k1"), Some(b"secret-1".to_vec()));
    assert_eq!(source.key("k2"), None);
}

#[test]
fn shared_messaging_file_key_source_reads_one_file_per_id() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("k1"),
        b"deployment-key
",
    )
    .unwrap();
    let source = FileKeySource::new(dir.path());
    assert_eq!(source.key("k1"), Some(b"deployment-key".to_vec()));
    assert_eq!(source.key("missing"), None);
}

#[test]
fn shared_messaging_file_key_source_refuses_path_traversal() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("secret"), b"x").unwrap();
    let source = FileKeySource::new(dir.path().join("sub"));
    assert_eq!(source.key("../secret"), None);
    assert_eq!(source.key(".hidden"), None);
    assert_eq!(source.key(""), None);
}
