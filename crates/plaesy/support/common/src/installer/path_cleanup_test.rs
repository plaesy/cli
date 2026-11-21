//! Unit tests for `tests` split out of `path_cleanup.rs`.
//! Kept as a separate file so they are easy to find; included via `#[path]`.

use super::*;

#[test]
fn drop_entry_removes_only_the_matching_segment() {
    let value = "/usr/bin;/home/me/.local/bin;/opt";
    let updated = drop_path_entry(value, Path::new("/home/me/.local/bin"), false);
    assert_eq!(updated.as_deref(), Some("/usr/bin;/opt"));
    assert_eq!(drop_path_entry(value, Path::new("/nowhere"), false), None);
}

#[test]
fn drop_entry_folds_case_on_windows_style_paths() {
    let value = r"C:\Tools;C:\Users\Me\AppData\Local\Plaesy\bin";
    let updated = drop_path_entry(
        value,
        Path::new(r"c:\users\me\appdata\local\plaesy\bin"),
        true,
    );
    assert_eq!(updated.as_deref(), Some(r"C:\Tools"));
}

#[test]
fn installer_block_is_removed_with_its_blank_line() {
    let dir = tempfile::tempdir().unwrap();
    let profile = dir.path().join(".bashrc");
    let bin = Path::new("/home/me/.local/bin");
    fs::write(
        &profile,
        format!(
            "alias ll='ls -l'\n\n{PROFILE_MARKER}\nexport PATH=\"{}:$PATH\"\n",
            bin.display()
        ),
    )
    .unwrap();
    assert!(remove_installer_lines(&profile, bin).unwrap());
    assert_eq!(fs::read_to_string(&profile).unwrap(), "alias ll='ls -l'\n");
    assert!(!remove_installer_lines(&profile, bin).unwrap());
}

#[test]
fn guarded_shell_and_fish_path_blocks_are_removed() {
    let dir = tempfile::tempdir().unwrap();
    let bin = Path::new("/home/me/.local/bin");
    let guarded = format!(
            "{PROFILE_MARKER}\ncase \":$PATH:\" in *:\"{}\":*) ;; *) export PATH=\"{}\":$PATH ;; esac\n",
            bin.display(),
            bin.display()
        );
    let bashrc = dir.path().join(".bashrc");
    fs::write(&bashrc, &guarded).unwrap();
    assert!(remove_installer_lines(&bashrc, bin).unwrap());
    assert_eq!(fs::read_to_string(&bashrc).unwrap(), "");

    let fish = dir.path().join("config.fish");
    fs::write(
        &fish,
        format!(
            "{PROFILE_MARKER}\nfish_add_path --path --prepend '{}'\n",
            bin.display()
        ),
    )
    .unwrap();
    assert!(remove_installer_lines(&fish, bin).unwrap());
    assert_eq!(fs::read_to_string(&fish).unwrap(), "");
}
