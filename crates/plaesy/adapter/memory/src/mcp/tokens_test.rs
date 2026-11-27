use super::*;
use std::path::Path;
use std::time::{Duration, Instant};

fn two_labels() -> (TokenFile, String, String) {
    let mut file = TokenFile::default();
    let laptop = file.add("laptop", 1000).expect("add laptop");
    let server = file.add("server", 1000).expect("add server");
    (file, laptop, server)
}

fn laptop_allowed() -> Verdict {
    Verdict::Allowed("laptop".to_owned())
}

#[test]
fn env_token_alone_keeps_single_token_behaviour() {
    let env = "e".repeat(40);
    let set = TokenSet::new(Some(env.clone()), Vec::new());
    assert_eq!(
        set.authorize(&env, 0),
        Verdict::Allowed(DEFAULT_LABEL.to_owned())
    );
    assert_eq!(set.authorize("wrong", 0), Verdict::Denied);
    assert_eq!(
        TokenSet::new(None, Vec::new()).authorize(&env, 0),
        Verdict::Denied
    );
}

#[test]
fn each_added_token_matches_only_its_own_label() {
    let (file, laptop, server) = two_labels();
    let set = TokenSet::new(None, file.tokens.clone());
    assert_eq!(set.authorize(&laptop, 1000), laptop_allowed());
    assert_eq!(
        set.authorize(&server, 1000),
        Verdict::Allowed("server".to_owned())
    );
    assert!(laptop.starts_with("plsy_"));
}

#[test]
fn revoking_one_label_leaves_the_other_working() {
    let (mut file, laptop, server) = two_labels();
    file.revoke("laptop").expect("revoke laptop");
    let set = TokenSet::new(None, file.tokens.clone());
    assert_eq!(set.authorize(&laptop, 1000), Verdict::Denied);
    assert_eq!(
        set.authorize(&server, 1000),
        Verdict::Allowed("server".to_owned())
    );
}

#[test]
fn rotation_keeps_the_old_token_only_until_grace_expires() {
    let (mut file, old, _) = two_labels();
    let new = file.rotate("laptop", 1000, 100).expect("rotate");
    let set = TokenSet::new(None, file.tokens.clone());
    assert_eq!(set.authorize(&old, 1099), laptop_allowed());
    assert_eq!(set.authorize(&old, 1100), Verdict::Denied);
    assert_eq!(set.authorize(&new, 1100), laptop_allowed());
    assert_eq!(set.authorize(&new, 999_999), laptop_allowed());
}

#[test]
fn second_rotation_drops_the_token_from_the_first_rotation() {
    let (mut file, first, _) = two_labels();
    let second = file.rotate("laptop", 1000, 100).expect("rotate once");
    let third = file.rotate("laptop", 1001, 100).expect("rotate twice");
    let set = TokenSet::new(None, file.tokens.clone());
    assert_eq!(set.authorize(&first, 1002), Verdict::Denied);
    assert_eq!(set.authorize(&second, 1002), laptop_allowed());
    assert_eq!(set.authorize(&third, 1002), laptop_allowed());
}

#[test]
fn rotate_and_add_reject_unknown_and_revoked_labels() {
    let (mut file, _, _) = two_labels();
    assert!(file.rotate("missing", 1000, 10).is_err());
    file.revoke("server").expect("revoke server");
    assert!(file.rotate("server", 1000, 10).is_err());
    assert!(file.add("server", 1000).is_err());
}

#[test]
fn labels_are_restricted_and_default_is_reserved() {
    assert!(validate_label("laptop").is_ok());
    assert!(validate_label("server-2").is_ok());
    let long = "x".repeat(33);
    for bad in ["", "Laptop", "a b", "default", "a_b", long.as_str()] {
        assert!(
            validate_label(bad).is_err(),
            "label {bad:?} should be refused"
        );
    }
}

#[test]
fn token_shape_yields_a_lookup_id() {
    let (_, token, _) = two_labels();
    assert!(id_of_token(&token).is_some());
    assert!(id_of_token("plsy_short_x").is_none());
    assert!(id_of_token("plsy_0123456789ab_").is_none());
    assert!(id_of_token("Bearer abc").is_none());
}

#[test]
fn unknown_id_is_denied() {
    let set = TokenSet::new(None, Vec::new());
    assert_eq!(
        set.authorize("plsy_0123456789ab_abcdef", 1000),
        Verdict::Denied
    );
}

#[test]
fn hash_depends_on_both_salt_and_token() {
    assert_eq!(hash_token("s", "t"), hash_token("s", "t"));
    assert_ne!(hash_token("s1", "t"), hash_token("s2", "t"));
    assert_ne!(hash_token("s", "t1"), hash_token("s", "t2"));
}

#[test]
fn saved_file_holds_no_plaintext_token_and_still_authorizes() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("mcp-tokens.toml");
    let (file, token, _) = two_labels();
    save(&path, &file).expect("save");
    let text = std::fs::read_to_string(&path).expect("read saved file");
    let secret = token.rsplit('_').next().expect("secret part");
    assert!(!text.contains(&token));
    assert!(!text.contains(secret));
    let reloaded = load(&path).expect("load").expect("file present");
    let set = TokenSet::new(None, reloaded.tokens);
    assert_eq!(set.authorize(&token, 1000), laptop_allowed());
}

#[test]
fn summary_shows_state_and_no_hash_or_salt() {
    let (mut file, _, _) = two_labels();
    file.rotate("laptop", 1000, 60).expect("rotate");
    file.revoke("server").expect("revoke");
    let text = file.summary().join("\n");
    assert!(text.contains("laptop\tenabled"));
    assert!(text.contains("previous_expires=1060"));
    assert!(text.contains("server\trevoked"));
    for entry in &file.tokens {
        assert!(!text.contains(&entry.hash));
        assert!(!text.contains(&entry.salt));
    }
}

#[test]
fn audit_line_names_the_label_and_never_the_token() {
    let (_, token, _) = two_labels();
    let line = audit_line(
        "auth_ok",
        Some("laptop"),
        Some(Path::new("/tmp/mcp-tokens.toml")),
        1000,
        0,
    );
    assert!(line.contains("label=laptop"));
    assert!(line.contains("path=/tmp/mcp-tokens.toml"));
    assert!(line.contains("at=1000"));
    assert!(!line.contains(&token));
    assert!(!line.contains("plsy_"));
    let failed = audit_line("auth_failed", None, None, 1000, 3);
    assert!(failed.contains("failures_since_last_log=3"));
    assert!(!failed.contains("label="));
}

#[test]
fn token_file_inside_the_vault_is_refused() {
    let vault = tempfile::tempdir().expect("vault");
    let inside = vault.path().join(".config").join("mcp-tokens.toml");
    assert!(ensure_outside(&inside, vault.path()).is_err());
    let other = tempfile::tempdir().expect("other");
    assert!(ensure_outside(&other.path().join("mcp-tokens.toml"), vault.path()).is_ok());
    assert!(TokenAuth::new(None, Some(inside), vault.path()).is_err());
}

#[test]
fn no_token_source_returns_none_so_the_server_stays_open() {
    let vault = tempfile::tempdir().expect("vault");
    let auth = TokenAuth::new(None, None, vault.path()).expect("new");
    assert!(auth.is_none());
}

#[test]
fn env_token_without_file_requires_auth() {
    let vault = tempfile::tempdir().expect("vault");
    let env = "e".repeat(40);
    let auth = TokenAuth::new(Some(env.clone()), None, vault.path())
        .expect("new")
        .expect("auth present");
    let now = Instant::now();
    assert_eq!(
        auth.check_at(Some(&env), 0, now),
        Verdict::Allowed(DEFAULT_LABEL.to_owned())
    );
    assert_eq!(auth.check_at(None, 0, now), Verdict::Denied);
    assert_eq!(auth.check_at(Some("other"), 0, now), Verdict::Denied);
}

#[test]
fn file_created_after_startup_turns_auth_on_without_restart() {
    let dir = tempfile::tempdir().expect("dir");
    let path = dir.path().join("mcp-tokens.toml");
    let vault = tempfile::tempdir().expect("vault");
    let auth = TokenAuth::new(None, Some(path.clone()), vault.path())
        .expect("new")
        .expect("auth present");
    let start = Instant::now();
    assert_eq!(auth.check_at(None, 1000, start), Verdict::Open);

    let (file, token, _) = two_labels();
    save(&path, &file).expect("save");
    let later = start + Duration::from_secs(3);
    assert_eq!(auth.check_at(None, 1000, later), Verdict::Denied);
    assert_eq!(auth.check_at(Some(&token), 1000, later), laptop_allowed());
}

#[test]
fn revocation_in_the_file_takes_effect_on_the_next_check() {
    let dir = tempfile::tempdir().expect("dir");
    let path = dir.path().join("mcp-tokens.toml");
    let vault = tempfile::tempdir().expect("vault");
    let (mut file, token, server) = two_labels();
    save(&path, &file).expect("save");
    let auth = TokenAuth::new(None, Some(path.clone()), vault.path())
        .expect("new")
        .expect("auth present");
    let start = Instant::now();
    assert_eq!(auth.check_at(Some(&token), 1000, start), laptop_allowed());

    file.revoke("laptop").expect("revoke");
    save(&path, &file).expect("save revoked");
    let later = start + Duration::from_secs(3);
    assert_eq!(auth.check_at(Some(&token), 1000, later), Verdict::Denied);
    assert_eq!(
        auth.check_at(Some(&server), 1000, later),
        Verdict::Allowed("server".to_owned())
    );
}

#[test]
fn emptied_file_denies_everything_and_keeps_auth_on() {
    let dir = tempfile::tempdir().expect("dir");
    let path = dir.path().join("mcp-tokens.toml");
    let vault = tempfile::tempdir().expect("vault");
    let (file, token, _) = two_labels();
    save(&path, &file).expect("save");
    let auth = TokenAuth::new(None, Some(path.clone()), vault.path())
        .expect("new")
        .expect("auth present");
    let start = Instant::now();
    assert_eq!(auth.check_at(Some(&token), 1000, start), laptop_allowed());

    std::fs::write(&path, "").expect("empty the file");
    let later = start + Duration::from_secs(3);
    assert_eq!(auth.check_at(Some(&token), 1000, later), Verdict::Denied);
    assert_eq!(auth.check_at(None, 1000, later), Verdict::Denied);
}

#[test]
fn deleted_file_fails_closed_instead_of_opening() {
    let dir = tempfile::tempdir().expect("dir");
    let path = dir.path().join("mcp-tokens.toml");
    let vault = tempfile::tempdir().expect("vault");
    let (file, token, _) = two_labels();
    save(&path, &file).expect("save");
    let auth = TokenAuth::new(None, Some(path.clone()), vault.path())
        .expect("new")
        .expect("auth present");
    let start = Instant::now();
    assert_eq!(auth.check_at(Some(&token), 1000, start), laptop_allowed());

    std::fs::remove_file(&path).expect("delete the file");
    let later = start + Duration::from_secs(3);
    assert_eq!(auth.check_at(None, 1000, later), Verdict::Denied);
    assert_eq!(auth.check_at(Some(&token), 1000, later), Verdict::Denied);
}

#[test]
fn corrupt_file_keeps_the_last_valid_set() {
    let dir = tempfile::tempdir().expect("dir");
    let path = dir.path().join("mcp-tokens.toml");
    let vault = tempfile::tempdir().expect("vault");
    let (file, token, _) = two_labels();
    save(&path, &file).expect("save");
    let auth = TokenAuth::new(None, Some(path.clone()), vault.path())
        .expect("new")
        .expect("auth present");
    let start = Instant::now();
    assert_eq!(auth.check_at(Some(&token), 1000, start), laptop_allowed());

    std::fs::write(&path, "[[token]\nlabel = ").expect("write corrupt file");
    let later = start + Duration::from_secs(3);
    assert_eq!(auth.check_at(Some(&token), 1000, later), laptop_allowed());
}

#[test]
fn reload_is_skipped_inside_the_poll_interval() {
    let dir = tempfile::tempdir().expect("dir");
    let path = dir.path().join("mcp-tokens.toml");
    let vault = tempfile::tempdir().expect("vault");
    let (file, token, _) = two_labels();
    save(&path, &file).expect("save");
    let auth = TokenAuth::new(None, Some(path.clone()), vault.path())
        .expect("new")
        .expect("auth present");
    let start = Instant::now();
    assert_eq!(auth.check_at(Some(&token), 1000, start), laptop_allowed());

    std::fs::remove_file(&path).expect("delete the file");
    let soon = start + Duration::from_millis(500);
    assert_eq!(auth.check_at(Some(&token), 1000, soon), laptop_allowed());
}
