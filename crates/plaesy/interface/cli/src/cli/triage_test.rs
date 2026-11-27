use super::*;

fn facts() -> Facts {
    Facts {
        version: "0.0.1".to_owned(),
        os: "windows".to_owned(),
        arch: "x86_64".to_owned(),
        git: Some("git version 2.45.0".to_owned()),
        in_repo: true,
        providers: vec![(
            "local".to_owned(),
            "ollama".to_owned(),
            "enabled".to_owned(),
        )],
        notes: Vec::new(),
    }
}

#[test]
fn bundle_lists_version_platform_git_and_providers() {
    let text = bundle(&facts());
    assert!(text.contains("plaesy: 0.0.1"));
    assert!(text.contains("os: windows (x86_64)"));
    assert!(text.contains("git: git version 2.45.0"));
    assert!(text.contains("repository: yes"));
    assert!(text.contains("local kind=ollama status=enabled"));
}

#[test]
fn missing_git_and_no_providers_are_reported() {
    let mut f = facts();
    f.git = None;
    f.in_repo = false;
    f.providers.clear();
    let text = bundle(&f);
    assert!(text.contains("git: not found"));
    assert!(text.contains("repository: no"));
    assert!(text.contains("(none)"));
}

#[test]
fn a_token_in_a_note_is_redacted_before_output() {
    let mut f = facts();
    let fake_token = ["xoxb", "1234567890", "abcdefghijklmnop"].join("-");
    f.notes.push(format!("config carried {fake_token}"));
    let text = bundle(&f);
    assert!(!text.contains(&fake_token));
    assert!(text.contains("note: "));
}

#[test]
fn a_token_in_a_provider_name_is_redacted_before_output() {
    let mut f = facts();
    f.providers.push((
        ["ghp", "abcdefghijklmnopqrstuvwxyz0123456789"]
            .join("_")
            .to_owned(),
        "openai".to_owned(),
        "disabled".to_owned(),
    ));
    let text = bundle(&f);
    assert!(!text.contains(&["ghp", "abcdefghijklmnopqrstuvwxyz0123456789"].join("_")));
}

#[test]
fn render_keeps_raw_text_for_the_redactor_to_handle() {
    let mut f = facts();
    f.notes.push("plain note".to_owned());
    assert!(render(&f).contains("note: plain note"));
}
