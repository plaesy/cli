//! Unit tests for `tests` split out of `mod.rs`.
//! Kept as a separate file so they are easy to find; included via `#[path]`.

use super::*;
use crate::memory::config::SearchConfig;
use crate::memory::storage::{Database, DatabaseConfig};
use std::sync::Arc;

#[test]
fn template_creation_fills_managed_frontmatter_and_primary_content() {
    let root = tempfile::tempdir().expect("temporary vault");
    let templates = root.path().join("templates");
    std::fs::create_dir_all(&templates).expect("templates directory");
    std::fs::write(
        templates.join("knowledge.md"),
        "---\ntype: knowledge\nstatus: draft\n---\n\n# {{TITLE}}\n\n{{ONE_SENTENCE_SUMMARY}}\n",
    )
    .expect("template");

    let (folder, note, graph) = render_template(
        root.path(),
        "knowledge",
        "Useful knowledge",
        "A concise explanation.",
        TemplateOverrides {
            folder: Some("knowledges/testing"),
            values: &[],
            id: None,
            status: None,
            project: None,
            scope: &[],
            supersedes: &[],
        },
    )
    .expect("rendered template");

    let frontmatter = note.split("---").nth(1).expect("frontmatter");
    let metadata: serde_yaml::Mapping = serde_yaml::from_str(frontmatter).expect("valid YAML");
    let generated_id = yaml_string(&metadata, "id").expect("id");
    assert_eq!(folder, "knowledges/testing");
    assert!(is_uuid(&generated_id));
    assert_eq!(yaml_string(&metadata, "type").as_deref(), Some("knowledge"));
    assert_eq!(
        yaml_string(&metadata, "title").as_deref(),
        Some("Useful knowledge")
    );
    assert_eq!(
        yaml_string(&metadata, "source").as_deref(),
        Some("tool:plaesy")
    );
    assert!(metadata.contains_key(yaml_key("schema")));
    assert!(metadata.contains_key(yaml_key("created")));
    assert!(metadata.contains_key(yaml_key("updated")));
    assert!(note.contains("# Useful knowledge"));
    assert!(note.contains("A concise explanation."));
    assert!(!note.contains("{{ONE_SENTENCE_SUMMARY}}"));
    assert_eq!(graph.expect("knowledge graph fields").id, generated_id);
}

async fn engine(root: &std::path::Path) -> SearchEngine {
    let db = Arc::new(
        Database::new(DatabaseConfig {
            db_path: ":memory:".into(),
            ..DatabaseConfig::default()
        })
        .await
        .expect("db"),
    );
    let e = SearchEngine::new(db, None, SearchConfig::default())
        .await
        .expect("engine");
    e.set_watch_root(root.to_path_buf());
    e
}

/// Make a directory link; `false` when the OS refuses (Windows needs developer mode or admin).
fn dir_link(target: &std::path::Path, link: &std::path::Path) -> bool {
    #[cfg(unix)]
    let made = std::os::unix::fs::symlink(target, link);
    #[cfg(windows)]
    let made = std::os::windows::fs::symlink_dir(target, link);
    made.is_ok()
}

#[test]
fn slugs_are_safe_and_bounded() {
    assert_eq!(slugify("Deploy: Steps!!"), "deploy-steps");
    assert!(slugify("***").starts_with("note-"));
    assert!(slugify(&"a ".repeat(100)).len() <= 80);
}

#[test]
fn non_ascii_titles_do_not_collide() {
    assert_ne!(slugify("漢字"), slugify("العربية"));
    assert_eq!(slugify("漢字"), slugify("漢字"));
}

#[tokio::test]
async fn agent_cannot_claim_high_confidence() {
    let dir = tempfile::tempdir().expect("tmp");
    let engine = engine(dir.path()).await;
    let note = NewNote {
        folder: "ops".into(),
        title: "Claimed high".into(),
        body: "trust me".into(),
        tier: "procedural".into(),
        confidence: "high".into(),
        graph: None,
    };
    let path = engine.remember(&note).await.expect("saved");
    let text = std::fs::read_to_string(path).expect("read");
    assert!(text.contains("confidence: medium"));
    assert!(!text.contains("confidence: high"));
}

#[tokio::test]
async fn remember_rejects_bad_titles_and_oversized_bodies() {
    let dir = tempfile::tempdir().expect("tmp");
    let engine = engine(dir.path()).await;
    let base = NewNote {
        folder: "ops".into(),
        title: "ok".into(),
        body: "x".into(),
        tier: "procedural".into(),
        confidence: "high".into(),
        graph: None,
    };
    let blank = NewNote {
        title: "   ".into(),
        ..base.clone()
    };
    assert!(engine.remember(&blank).await.is_err());
    let blank_body = NewNote {
        body: "  \n".into(),
        ..base.clone()
    };
    assert!(engine.remember(&blank_body).await.is_err());
    let long = NewNote {
        title: "t".repeat(MAX_TITLE_CHARS + 1),
        ..base.clone()
    };
    assert!(engine.remember(&long).await.is_err());
    let big = NewNote {
        body: "x".repeat(MAX_BODY_BYTES + 1),
        ..base.clone()
    };
    assert!(engine.remember(&big).await.is_err());
    assert!(!dir.path().join("ops").join("ok.md").exists());
}

#[tokio::test]
async fn remember_rejects_symlink_lock_path() {
    let root = tempfile::tempdir().expect("root");
    let outside = tempfile::tempdir().expect("outside");
    if !dir_link(
        outside.path(),
        &root.path().join(".plaesy-memory-writer.lock"),
    ) {
        return;
    }
    let engine = engine(root.path()).await;
    let note = NewNote {
        folder: "ops".into(),
        title: "lock symlink".into(),
        body: "x".into(),
        tier: "procedural".into(),
        confidence: "medium".into(),
        graph: None,
    };
    let err = engine
        .remember(&note)
        .await
        .expect_err("symlink lock must fail");
    assert!(err.to_string().contains("lock path is not a directory"));
    assert!(!outside.path().join("ops").exists());
}

#[tokio::test]
async fn remember_refuses_link_out_of_root() {
    let root = tempfile::tempdir().expect("tmp");
    let outside = tempfile::tempdir().expect("tmp");
    if !dir_link(outside.path(), &root.path().join("link")) {
        return; // no link privilege on this machine; nothing to test
    }
    let engine = engine(root.path()).await;
    let note = NewNote {
        folder: "link".into(),
        title: "escape".into(),
        body: "x".into(),
        tier: "procedural".into(),
        confidence: "high".into(),
        graph: None,
    };
    assert!(engine.remember(&note).await.is_err());
    assert!(!outside.path().join("escape.md").exists());
}

#[test]
fn folders_must_stay_inside_root() {
    assert_eq!(safe_folder("").unwrap(), PathBuf::from("inbox"));
    assert_eq!(
        safe_folder("ops/runbooks").unwrap(),
        PathBuf::from("ops/runbooks")
    );
    assert!(safe_folder("../outside").is_err());
    assert!(safe_folder("a/../../b").is_err());
    assert!(safe_folder("/etc").is_ok()); // leading slash is trimmed, then relative
}

#[test]
fn default_inbox_finds_numbered_vault_inbox() {
    let dir = tempfile::tempdir().expect("tmp");
    assert_eq!(default_inbox(dir.path()), PathBuf::from("inbox"));
    std::fs::create_dir(dir.path().join("00. Inbox")).expect("mkdir");
    assert_eq!(default_inbox(dir.path()), PathBuf::from("00. Inbox"));
}

#[test]
fn rendered_note_has_provenance() {
    let note = NewNote {
        folder: String::new(),
        title: "Rollback \"plan\"".into(),
        body: "Revert the release.".into(),
        tier: "procedural".into(),
        confidence: "medium".into(),
        graph: None,
    };
    let text = render(&note, NaiveDate::from_ymd_opt(2026, 10, 26).unwrap());
    assert!(text.contains("source: agent"));
    assert!(text.contains("last_verified: 2026-10-26"));
    assert!(text.contains("title: \"Rollback \\\"plan\\\"\""));
}

#[tokio::test]
async fn remember_is_idempotent_on_identical_retry() {
    let dir = tempfile::tempdir().expect("tmp");
    let engine = engine(dir.path()).await;
    let note = NewNote {
        folder: "".into(),
        title: "Retry Note".into(),
        body: "Same content both times.".into(),
        tier: "semantic".into(),
        confidence: "medium".into(),
        graph: None,
    };
    let first = engine.remember(&note).await.expect("first call");
    // Simulate a client retry after e.g. a timeout: identical call must succeed again with
    // the same path, not error as a conflict.
    let second = engine.remember(&note).await.expect("idempotent retry");
    assert_eq!(first, second);
}

#[tokio::test]
async fn remember_rejects_same_title_with_different_content() {
    let dir = tempfile::tempdir().expect("tmp");
    let engine = engine(dir.path()).await;
    let mut note = NewNote {
        folder: "".into(),
        title: "Conflict Note".into(),
        body: "Original content.".into(),
        tier: "semantic".into(),
        confidence: "medium".into(),
        graph: None,
    };
    engine.remember(&note).await.expect("first call");
    note.body = "Different content, not a retry.".into();
    let err = engine
        .remember(&note)
        .await
        .expect_err("must not silently overwrite differing content");
    assert!(err.to_string().contains("different content"));
}

#[tokio::test]
async fn remember_writes_valid_graph_fields_and_they_are_queryable() {
    let dir = tempfile::tempdir().expect("tmp");
    let engine = engine(dir.path()).await;
    let note = NewNote {
        folder: "01. Projects".into(),
        title: "Studio C".into(),
        body: "A new studio.".into(),
        tier: "semantic".into(),
        confidence: "medium".into(),
        graph: Some(GraphFields {
            note_type: "project".into(),
            id: "studio-c".into(),
            status: Some("active".into()),
            project: None,
            scope: vec![],
            supersedes: vec![],
        }),
    };
    let path = engine.remember(&note).await.expect("saved");
    let text = std::fs::read_to_string(&path).expect("read");
    assert!(text.contains("type: \"project\""));
    assert!(text.contains("id: \"studio-c\""));
    assert!(text.contains("status: \"active\""));
    let graph = crate::memory::graph::load_graph(&engine.database())
        .await
        .expect("graph loads");
    assert!(graph.notes.iter().any(|n| n.id == "studio-c"));
}

#[tokio::test]
async fn remember_rejects_duplicate_graph_id() {
    let dir = tempfile::tempdir().expect("tmp");
    let engine = engine(dir.path()).await;
    let first = NewNote {
        folder: "01. Projects".into(),
        title: "Studio D".into(),
        body: "First.".into(),
        tier: "semantic".into(),
        confidence: "medium".into(),
        graph: Some(GraphFields {
            note_type: "project".into(),
            id: "dup-id".into(),
            status: Some("active".into()),
            project: None,
            scope: vec![],
            supersedes: vec![],
        }),
    };
    engine.remember(&first).await.expect("first saved");
    let second = NewNote {
        folder: "01. Projects".into(),
        title: "Studio E".into(),
        body: "Second, different title, same id.".into(),
        tier: "semantic".into(),
        confidence: "medium".into(),
        graph: Some(GraphFields {
            note_type: "project".into(),
            id: "dup-id".into(),
            status: Some("active".into()),
            project: None,
            scope: vec![],
            supersedes: vec![],
        }),
    };
    let err = engine
        .remember(&second)
        .await
        .expect_err("duplicate id rejected");
    assert!(err.to_string().contains("dup-id"));
}

#[tokio::test]
async fn remember_rejects_self_superseding_decision() {
    let dir = tempfile::tempdir().expect("tmp");
    let engine = engine(dir.path()).await;
    let note = NewNote {
        folder: "02. Areas".into(),
        title: "Self Ref".into(),
        body: "Supersedes itself.".into(),
        tier: "semantic".into(),
        confidence: "medium".into(),
        graph: Some(GraphFields {
            note_type: "decision".into(),
            id: "self-1".into(),
            status: None,
            project: None,
            scope: vec![],
            supersedes: vec!["self-1".into()],
        }),
    };
    let err = engine
        .remember(&note)
        .await
        .expect_err("self-supersede rejected");
    assert!(err.to_string().contains("supersede itself"));
}

#[tokio::test]
async fn remember_rejects_unknown_graph_type_and_missing_id() {
    let dir = tempfile::tempdir().expect("tmp");
    let engine = engine(dir.path()).await;
    let bad_type = NewNote {
        folder: String::new(),
        title: "X".into(),
        body: "x".into(),
        tier: "semantic".into(),
        confidence: "medium".into(),
        graph: Some(GraphFields {
            note_type: "not-a-type".into(),
            id: "x".into(),
            status: None,
            project: None,
            scope: vec![],
            supersedes: vec![],
        }),
    };
    assert!(engine.remember(&bad_type).await.is_err());
    let missing_id = NewNote {
        folder: String::new(),
        title: "Y".into(),
        body: "y".into(),
        tier: "semantic".into(),
        confidence: "medium".into(),
        graph: Some(GraphFields {
            note_type: "decision".into(),
            id: String::new(),
            status: None,
            project: None,
            scope: vec![],
            supersedes: vec![],
        }),
    };
    assert!(engine.remember(&missing_id).await.is_err());
    let bad_status = NewNote {
        folder: String::new(),
        title: "Z".into(),
        body: "z".into(),
        tier: "semantic".into(),
        confidence: "medium".into(),
        graph: Some(GraphFields {
            note_type: "decision".into(),
            id: "z1".into(),
            status: Some("not-a-status".into()),
            project: None,
            scope: vec![],
            supersedes: vec![],
        }),
    };
    assert!(engine.remember(&bad_status).await.is_err());
}

#[tokio::test]
async fn graph_fields_with_yaml_breaking_characters_still_parse() {
    // Regression test: id/status/project/scope with ':' or '"' used to be written as bare
    // or naively single-quoted YAML scalars, producing invalid frontmatter that silently
    // fell back to plain-text indexing (the note vanished from the graph without error).
    let dir = tempfile::tempdir().expect("tmp");
    let engine = engine(dir.path()).await;
    let note = NewNote {
        folder: String::new(),
        title: "Edge case".into(),
        body: "x".into(),
        tier: "semantic".into(),
        confidence: "medium".into(),
        graph: Some(GraphFields {
            note_type: "decision".into(),
            id: "weird: id \"with\" quotes".into(),
            status: Some("active".into()),
            project: Some("studio: a".into()),
            scope: vec!["studio\"b".into(), "plain".into()],
            supersedes: vec![],
        }),
    };
    let path = engine.remember(&note).await.expect("saved");
    let text = std::fs::read_to_string(&path).expect("read");
    // The written file must be valid YAML frontmatter, not a fallback-to-plain-text note.
    let fm = text
        .strip_prefix("---\n")
        .and_then(|s| s.split_once("\n---\n"))
        .map(|(fm, _)| fm)
        .expect("has frontmatter delimiters");
    let parsed: serde_yaml::Value =
        serde_yaml::from_str(fm).expect("frontmatter must be valid YAML");
    assert_eq!(
        parsed.get("id").and_then(|v| v.as_str()),
        Some("weird: id \"with\" quotes")
    );
    assert_eq!(
        parsed.get("project").and_then(|v| v.as_str()),
        Some("studio: a")
    );
    let graph = crate::memory::graph::load_graph(&engine.database())
        .await
        .expect("graph loads");
    assert!(graph
        .notes
        .iter()
        .any(|n| n.id == "weird: id \"with\" quotes"));
}
