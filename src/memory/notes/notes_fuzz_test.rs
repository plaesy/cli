//! Unit tests for `fuzz` split out of `mod.rs`.
//! Kept as a separate file so they are easy to find; included via `#[path]`.

use super::*;
use proptest::prelude::*;

proptest! {
    // slugify must never panic, is always <= 80 chars, and only ever produces
    // lowercase ascii alphanumerics and single, non-leading/trailing hyphens.
    #[test]
    fn slugify_never_panics_and_is_bounded(title in ".*") {
        let slug = slugify(&title);
        prop_assert!(slug.chars().count() <= 80);
        prop_assert!(!slug.is_empty());
        prop_assert!(slug.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-'));
        prop_assert!(!slug.starts_with('-'));
        prop_assert!(!slug.ends_with('-'));
        prop_assert!(!slug.contains("--"));
    }

    // safe_folder must reject every attempt to leave the root (via '..', absolute paths,
    // or anything that is not a plain relative Normal-component path), for any input.
    #[test]
    fn safe_folder_never_escapes_root(folder in ".*") {
        if let Ok(path) = safe_folder(&folder) {
            for part in path.components() {
                prop_assert!(matches!(part, std::path::Component::Normal(_)));
            }
        }
        // Either outcome (Ok with a safe relative path, or Err) is acceptable; a panic is not.
    }

    #[test]
    fn safe_folder_rejects_all_parent_traversal(
        segments in prop::collection::vec("[a-zA-Z0-9_. ]{0,8}", 1..6)
    ) {
        let folder = segments.join("/") + "/../escape";
        prop_assert!(safe_folder(&folder).is_err());
    }

    // render() embeds arbitrary agent-supplied strings into YAML frontmatter via
    // serde_json::to_string (JSON double-quoted scalars are valid YAML scalars). The
    // produced frontmatter must always parse back as YAML, for any title/body/graph text,
    // including strings containing ':', '"', '#', control characters, or unicode.
    #[test]
    fn render_always_produces_parseable_yaml(
        title in ".{0,200}",
        body in ".{0,200}",
        id in ".{0,100}",
        project in ".{0,100}",
    ) {
        let note = NewNote {
            folder: String::new(),
            title: if title.trim().is_empty() { "x".to_string() } else { title },
            body: if body.trim().is_empty() { "x".to_string() } else { body },
            tier: "semantic".into(),
            confidence: "medium".into(),
            graph: Some(GraphFields {
                note_type: "decision".into(),
                id: if id.trim().is_empty() { "id".to_string() } else { id },
                status: None,
                project: if project.is_empty() { None } else { Some(project) },
                scope: vec![],
                supersedes: vec![],
            }),
        };
        let text = render(&note, NaiveDate::from_ymd_opt(2026, 1, 1).unwrap());
        let fm = text
            .strip_prefix("---\n")
            .and_then(|s| s.split_once("\n---\n"))
            .map(|(fm, _)| fm);
        prop_assert!(fm.is_some(), "frontmatter delimiters must always be present");
        let parsed: std::result::Result<serde_yaml::Value, _> = serde_yaml::from_str(fm.unwrap());
        prop_assert!(parsed.is_ok(), "rendered frontmatter must always be valid YAML: {:?}", parsed.err());
    }

    // strip_last_verified must be stable (idempotent) and must never panic on arbitrary
    // multi-line text, including text that itself contains lines starting with the same
    // prefix in positions other than the real date line.
    #[test]
    fn strip_last_verified_is_idempotent(text in ".{0,500}") {
        let once = strip_last_verified(&text);
        let twice = strip_last_verified(&once);
        prop_assert_eq!(once, twice);
    }
}
