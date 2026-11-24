//! Unit tests for `env_file`, split out of `env_file.rs`.
//! Kept as a separate file so they are easy to find; included via `#[path]`.

use super::*;

#[test]
fn parses_plain_quoted_comments_and_export() {
    let map = parse(
        "# comment\n\
         \n\
         PLAESY_MCP_TOKEN=abc123\n\
         export TELEGRAM_BOT_TOKEN=\"quoted value\"\n\
         SINGLE='x y'\n\
         INLINE=value # trailing note\n\
         bad line without equals\n\
         1BAD=skipped\n",
    );
    assert_eq!(
        map.get("PLAESY_MCP_TOKEN").map(String::as_str),
        Some("abc123")
    );
    assert_eq!(
        map.get("TELEGRAM_BOT_TOKEN").map(String::as_str),
        Some("quoted value")
    );
    assert_eq!(map.get("SINGLE").map(String::as_str), Some("x y"));
    assert_eq!(map.get("INLINE").map(String::as_str), Some("value"));
    assert!(!map.contains_key("1BAD"));
    assert_eq!(map.len(), 4);
}

#[test]
fn later_line_overrides_earlier() {
    let map = parse("K=1\nK=2\n");
    assert_eq!(map.get("K").map(String::as_str), Some("2"));
}

#[test]
fn missing_file_is_empty() {
    let dir = tempfile::tempdir().unwrap();
    let got = load(&dir.path().join("nope.env")).unwrap();
    assert!(got.is_empty());
}

#[test]
fn file_lives_under_plaesy_folder() {
    let p = file_in_root(Path::new("/r"));
    assert!(p.ends_with(Path::new(".plaesy").join(".env")));
}

#[cfg(unix)]
#[test]
fn symlinked_env_file_is_rejected() {
    use std::os::unix::fs::symlink;

    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("outside.env");
    std::fs::write(&target, "PLAESY_MCP_TOKEN=x\n").unwrap();
    let path = dir.path().join(".env");
    symlink(&target, &path).unwrap();
    assert!(load(&path).is_err());
}
