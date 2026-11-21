//! Unit tests for `tests` split out of `installer.rs`.
//! Kept as a separate file so they are easy to find; included via `#[path]`.

use super::*;

#[test]
fn unix_instructions_name_the_shell_rc_file() {
    let dir = Path::new("/home/me/.local/bin");
    assert!(unix_path_instructions(dir, "/usr/bin/zsh").contains("~/.zshrc"));
    assert!(unix_path_instructions(dir, "/bin/bash").contains("~/.profile"));
    assert!(unix_path_instructions(dir, "").contains("~/.profile"));
}

#[cfg(windows)]
#[test]
fn windows_path_hint_edits_only_the_user_path() {
    let hint = path_instructions(Path::new(r"C:\Users\Me\AppData\Local\Plaesy\bin"));
    assert!(hint.contains("GetEnvironmentVariable('Path','User')"));
    assert!(!hint.contains("$env:Path"));
}

#[test]
fn copy_file_replaces_an_existing_destination() {
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("src");
    let dst = dir.path().join("dst");
    fs::write(&src, "new build").unwrap();
    fs::write(&dst, "old build").unwrap();
    copy_file(&src, &dst).unwrap();
    assert_eq!(fs::read_to_string(&dst).unwrap(), "new build");
    assert!(!fs::read_dir(dir.path())
        .unwrap()
        .filter_map(std::result::Result::ok)
        .any(|entry| entry.file_name().to_string_lossy().starts_with("dst.tmp-")));
}

#[test]
fn differing_digest_is_detected() {
    let dir = tempfile::tempdir().unwrap();
    let a = dir.path().join("a");
    let b = dir.path().join("b");
    fs::write(&a, "one").unwrap();
    fs::write(&b, "one").unwrap();
    assert!(!binary_differs(&a, &b).unwrap());
    fs::write(&b, "two").unwrap();
    assert!(binary_differs(&a, &b).unwrap());
    assert!(binary_differs(&a, &dir.path().join("missing")).unwrap());
}

#[test]
fn confirm_defaults_to_no() {
    assert!(confirm(io::Cursor::new("y\n"), ""));
    assert!(confirm(io::Cursor::new("YES\n"), ""));
    assert!(!confirm(io::Cursor::new("\n"), ""));
    assert!(!confirm(io::Cursor::new(""), ""));
}
