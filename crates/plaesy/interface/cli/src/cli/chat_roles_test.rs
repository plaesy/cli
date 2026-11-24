use super::*;

fn role(name: &str, system: Option<&str>) -> Role {
    Role {
        name: name.to_owned(),
        system: system.map(str::to_owned),
        ..Role::default()
    }
}

#[test]
fn parse_reads_the_roles_map() {
    let value = serde_json::json!({"roles": {"reviewer": {"system": "Tinjau.", "provider": "claude-cli", "skills": ["code-review"], "tools": ["search"]}}});
    let roles = parse_roles(&value).expect("valid");
    assert_eq!(roles[0].name, "reviewer");
    assert_eq!(roles[0].provider.as_deref(), Some("claude-cli"));
    assert_eq!(roles[0].skills, vec!["code-review"]);
    assert!(parse_roles(&serde_json::Value::Null)
        .expect("ok")
        .is_empty());
}

#[test]
fn repository_roles_replace_user_roles_with_the_same_name() {
    let merged = merge(
        vec![role("reviewer", Some("user")), role("penulis", None)],
        vec![role("reviewer", Some("repo"))],
    );
    let reviewer = merged.iter().find(|r| r.name == "reviewer").expect("kept");
    assert_eq!(reviewer.system.as_deref(), Some("repo"));
    assert!(merged.iter().any(|r| r.name == "penulis"));
    assert_eq!(merged.iter().filter(|r| r.name == "reviewer").count(), 1);
}

#[test]
fn default_role_exists_without_settings() {
    let default = find(&[], DEFAULT_ROLE).expect("built in");
    assert!(default.system.is_none() && default.provider.is_none());
    assert!(find(&[], "tidak-ada").is_none());
}

#[test]
fn system_text_joins_prompt_and_skill_files() {
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::create_dir_all(dir.path().join("code-review")).expect("mkdir");
    std::fs::write(
        dir.path().join("code-review").join("SKILL.md"),
        "Cek bug dulu.\n",
    )
    .expect("write");
    let mut r = role("reviewer", Some("Tinjau."));
    r.skills = vec!["code-review".to_owned(), "tidak-ada".to_owned()];

    let (text, warnings) = system_text(&r, dir.path());
    let text = text.expect("some text");
    assert!(text.starts_with("Tinjau."));
    assert!(text.contains("# Skill: code-review\n\nCek bug dulu."));
    assert_eq!(
        warnings.len(),
        1,
        "a missing skill is a warning: {warnings:?}"
    );
}

#[test]
fn skill_names_that_escape_the_store_are_refused() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut r = role("x", None);
    r.skills = vec!["../rahasia".to_owned(), "a/b".to_owned()];
    let (text, warnings) = system_text(&r, dir.path());
    assert!(text.is_none());
    assert_eq!(warnings.len(), 2);
}

#[test]
fn tool_list_narrows_only_when_present() {
    assert!(tool_allowed(&role("x", None), "anything"));
    let mut narrow = role("x", None);
    narrow.tools = vec!["search".to_owned()];
    assert!(tool_allowed(&narrow, "search"));
    assert!(!tool_allowed(&narrow, "delete"));
}
