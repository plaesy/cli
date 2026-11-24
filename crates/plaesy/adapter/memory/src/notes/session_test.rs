//! Unit tests for session summaries (`session.rs`), kept beside the module.

use super::*;
use crate::memory::config::SearchConfig;
use crate::memory::storage::{Database, DatabaseConfig};
use std::sync::Arc;

fn sample_date() -> NaiveDate {
    NaiveDate::from_ymd_opt(2026, 11, 23).expect("valid date")
}

fn sample_summary() -> SessionSummary {
    SessionSummary {
        title: "Perbaikan urutan checkpoint".into(),
        date: sample_date(),
        summary: "Fixed quartzbridge checkpoint ordering before the release.".into(),
        links: vec![SessionLink {
            label: "PR 12".into(),
            url: "https://example.com/pull/12".into(),
        }],
    }
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

#[test]
fn session_note_carries_title_date_summary_and_links() {
    let note = sample_summary().to_note().expect("note");
    assert_eq!(note.folder, SESSION_FOLDER);
    assert_eq!(note.tier, SESSION_TIER);
    assert_eq!(note.confidence, "medium");
    assert!(note.graph.is_none());
    assert_eq!(note.title, "Sesi 2026-11-23: Perbaikan urutan checkpoint");
    assert!(note.body.starts_with("Tanggal: 2026-11-23\n"));
    assert!(note
        .body
        .contains("Fixed quartzbridge checkpoint ordering before the release."));
    assert!(note.body.contains("## Tautan"));
    assert!(note.body.contains("- [PR 12](https://example.com/pull/12)"));
}

#[test]
fn session_note_without_links_has_no_links_section() {
    let mut summary = sample_summary();
    summary.links.clear();
    let note = summary.to_note().expect("note");
    assert!(!note.body.contains("Tautan"));
    assert!(note.body.contains("Tanggal: 2026-11-23"));
}

#[test]
fn session_note_rejects_empty_or_control_text() {
    let mut blank_title = sample_summary();
    blank_title.title = "   ".into();
    assert!(matches!(
        blank_title.to_note(),
        Err(MemoryError::ConfigError(_))
    ));

    let mut newline_title = sample_summary();
    newline_title.title = "first\nsecond".into();
    assert!(matches!(
        newline_title.to_note(),
        Err(MemoryError::ConfigError(_))
    ));

    let mut blank_summary = sample_summary();
    blank_summary.summary = "\n".into();
    assert!(matches!(
        blank_summary.to_note(),
        Err(MemoryError::ConfigError(_))
    ));
}

#[test]
fn session_links_must_be_plain_http_addresses() {
    let bad_links = [
        SessionLink {
            label: "Script".into(),
            url: "javascript:alert(1)".into(),
        },
        SessionLink {
            label: "Spaced".into(),
            url: "https://example.com/a b".into(),
        },
        SessionLink {
            label: "Paren".into(),
            url: "https://example.com/a)b".into(),
        },
        SessionLink {
            label: "Bracket [x]".into(),
            url: "https://example.com".into(),
        },
        SessionLink {
            label: "  ".into(),
            url: "https://example.com".into(),
        },
    ];
    for link in bad_links {
        let mut summary = sample_summary();
        summary.links = vec![link];
        assert!(
            matches!(summary.to_note(), Err(MemoryError::ConfigError(_))),
            "expected rejection"
        );
    }
}

#[test]
fn session_links_are_capped() {
    let mut summary = sample_summary();
    summary.links = (0..=MAX_SESSION_LINKS)
        .map(|i| SessionLink {
            label: format!("Link {i}"),
            url: format!("https://example.com/{i}"),
        })
        .collect();
    assert!(matches!(
        summary.to_note(),
        Err(MemoryError::ConfigError(_))
    ));
}

#[tokio::test]
async fn remember_session_writes_indexed_note_that_search_finds() {
    let dir = tempfile::tempdir().expect("tmp");
    let e = engine(dir.path()).await;
    let path = e
        .remember_session(&sample_summary())
        .await
        .expect("session saved");

    let expected_dir = dir.path().join(SESSION_FOLDER);
    assert!(path.starts_with(&expected_dir));
    let text = std::fs::read_to_string(&path).expect("read");
    assert!(text.contains("source: agent"));
    assert!(text.contains("tier: episodic"));
    assert!(text.contains("Tanggal: 2026-11-23"));

    // Indexed at once and found through the existing keyword search.
    let file_name = path
        .file_name()
        .and_then(|n| n.to_str())
        .expect("file name")
        .to_string();
    let hits = e.search("quartzbridge", 3).await.expect("search");
    assert!(hits
        .iter()
        .any(|h| h.source_path.ends_with(file_name.as_str())));
}

#[tokio::test]
async fn remember_session_rejects_invalid_summary_without_writing() {
    let dir = tempfile::tempdir().expect("tmp");
    let e = engine(dir.path()).await;
    let mut summary = sample_summary();
    summary.links = vec![SessionLink {
        label: "Bad".into(),
        url: "file:///etc/passwd".into(),
    }];
    assert!(e.remember_session(&summary).await.is_err());
    assert!(!dir.path().join(SESSION_FOLDER).exists());
}
