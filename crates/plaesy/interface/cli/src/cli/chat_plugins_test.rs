use super::*;

fn plugin_dir(root: &Path, name: &str, manifest: &str) -> PathBuf {
    let dir = root.join(name);
    fs::create_dir_all(&dir).expect("mkdir");
    fs::write(dir.join("plugin.json"), manifest).expect("write manifest");
    dir
}

#[test]
fn manifest_needs_a_folder_matching_its_name() {
    let root = tempfile::tempdir().expect("tempdir");
    let good = r#"{"name":"rapat","version":"0.1.0","commands":[{"name":"ringkas","command":"bin/ringkas","help":"ringkasan"}],"permissions":["read:.plaesy/tasks"]}"#;
    let dir = plugin_dir(root.path(), "rapat", good);
    let plugin = parse_manifest(good, &dir, Scope::User).expect("valid");
    assert_eq!(plugin.commands[0].command, "bin/ringkas");

    let wrong_folder = root.path().join("lain");
    fs::create_dir_all(&wrong_folder).expect("mkdir");
    assert!(parse_manifest(good, &wrong_folder, Scope::User).is_err());
}

#[test]
fn command_paths_must_stay_inside_the_plugin() {
    assert!(valid_relative("bin/ringkas"));
    assert!(!valid_relative("../rahasia"));
    assert!(!valid_relative("/usr/bin/sh"));
    assert!(!valid_relative(""));
    assert!(!valid_name("../x"));
    assert!(!valid_name("a b"));
}

#[test]
fn bad_manifests_are_reported_and_good_ones_still_load() {
    let root = tempfile::tempdir().expect("tempdir");
    plugin_dir(root.path(), "baik", r#"{"name":"baik","version":"1"}"#);
    plugin_dir(
        root.path(),
        "rusak",
        r#"{"name":"rusak","commands":[{"name":"x","command":"../luar"}]}"#,
    );
    let (plugins, problems) = discover(root.path(), Scope::User);
    assert_eq!(plugins.len(), 1);
    assert_eq!(plugins[0].name, "baik");
    assert_eq!(problems.len(), 1, "problems: {problems:?}");
}

#[test]
fn repo_plugins_need_approval_and_the_manifest_is_covered() {
    let root = tempfile::tempdir().expect("tempdir");
    let dir = plugin_dir(root.path(), "rapat", r#"{"name":"rapat","version":"1"}"#);
    let plugin = parse_manifest(
        &fs::read_to_string(dir.join("plugin.json")).expect("read"),
        &dir,
        Scope::Repo,
    )
    .expect("valid");
    assert!(!is_trusted(&plugin, &[]));
    assert!(is_trusted(&plugin, &[fingerprint(&plugin)]));

    let mut edited = plugin.clone();
    edited.manifest.push(' ');
    assert_ne!(fingerprint(&plugin), fingerprint(&edited));
}

#[test]
fn list_shows_commands_and_permissions_or_the_approval_needed() {
    let root = tempfile::tempdir().expect("tempdir");
    let dir = plugin_dir(
        root.path(),
        "rapat",
        r#"{"name":"rapat","version":"1","commands":[{"name":"ringkas","command":"bin/x","help":"ringkasan"}],"permissions":["read:tasks"]}"#,
    );
    let plugin = parse_manifest(
        &fs::read_to_string(dir.join("plugin.json")).expect("read"),
        &dir,
        Scope::User,
    )
    .expect("valid");
    let text = list(std::slice::from_ref(&plugin), &[]);
    assert!(text.contains("ringkas (ringkasan)"));
    assert!(text.contains("permissions: read:tasks"));
}

#[test]
fn run_refuses_untrusted_plugins_and_unknown_commands_before_starting() {
    let root = tempfile::tempdir().expect("tempdir");
    let dir = plugin_dir(
        root.path(),
        "rapat",
        r#"{"name":"rapat","version":"1","commands":[{"name":"ringkas","command":"bin/x"}]}"#,
    );
    let repo = parse_manifest(
        &fs::read_to_string(dir.join("plugin.json")).expect("read"),
        &dir,
        Scope::Repo,
    )
    .expect("valid");
    let plugins = vec![repo];
    let err = run(
        &plugins,
        &[],
        "rapat",
        "ringkas",
        &[],
        Duration::from_secs(5),
    )
    .expect_err("untrusted");
    assert!(err.contains("not approved"));
    let approved = vec![fingerprint(&plugins[0])];
    let err = run(
        &plugins,
        &approved,
        "rapat",
        "tidak-ada",
        &[],
        Duration::from_secs(5),
    )
    .expect_err("no command");
    assert!(err.contains("has no command"));
}

#[cfg(windows)]
#[test]
fn run_capture_returns_stdout_and_kills_on_timeout() {
    let dir = tempfile::tempdir().expect("tempdir");
    let out = run_capture(
        Path::new("cmd"),
        &[
            "/D".to_owned(),
            "/S".to_owned(),
            "/C".to_owned(),
            "echo halo".to_owned(),
        ],
        dir.path(),
        Duration::from_secs(5),
    )
    .expect("runs");
    assert_eq!(out, "halo");

    let err = run_capture(
        Path::new("cmd"),
        &[
            "/C".to_owned(),
            "ping".to_owned(),
            "-n".to_owned(),
            "6".to_owned(),
            "127.0.0.1".to_owned(),
        ],
        dir.path(),
        Duration::from_millis(300),
    )
    .expect_err("times out");
    assert!(err.contains("timed out"));
}

#[cfg(not(windows))]
#[test]
fn run_capture_returns_stdout_and_kills_on_timeout() {
    let dir = tempfile::tempdir().expect("tempdir");
    let out = run_capture(
        Path::new("/bin/sh"),
        &["-c".to_owned(), "echo halo".to_owned()],
        dir.path(),
        Duration::from_secs(5),
    )
    .expect("runs");
    assert_eq!(out, "halo");

    let err = run_capture(
        Path::new("/bin/sh"),
        &["-c".to_owned(), "sleep 6".to_owned()],
        dir.path(),
        Duration::from_millis(300),
    )
    .expect_err("times out");
    assert!(err.contains("timed out"));
}
