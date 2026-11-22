//! Unit tests for `tests` split out of `mod.rs`.
//! Kept as a separate file so they are easy to find; included via `#[path]`.

use super::*;

fn extract(content: &str) -> Document {
    Extractor::new()
        .extract(PathBuf::from("t.md"), content)
        .expect("extract")
}

#[test]
fn test_chunk_kind_serialize() {
    let kind = ChunkKind::Summary;
    let json = serde_json::to_string(&kind).expect("serialize");
    assert_eq!(json, "\"summary\"");
}

#[test]
fn test_chunker_default() {
    let chunker = Chunker::default();
    assert_eq!(chunker.chunk_size, 512);
    assert_eq!(chunker.overlap, 50);
}

#[test]
fn test_extract_frontmatter_and_body_offset() {
    let d = extract("---\nid: EV-01\ntags: [a, b]\n---\n\n# Title\n\nbody text\n");
    assert_eq!(d.metadata["id"], serde_json::json!("EV-01"));
    assert_eq!(d.metadata["tags"], serde_json::json!(["a", "b"]));
    assert_eq!(d.body_start_line, 5);
    assert!(d.body.starts_with("\n# Title"));
    assert_eq!(d.hash.as_deref().map(str::len), Some(64));
}

#[test]
fn test_extract_no_frontmatter_and_crlf() {
    let d = extract("# Hi\r\n\r\ntext\r\n");
    assert!(d.metadata.is_empty());
    assert_eq!(d.body_start_line, 1);
    assert!(!d.body.contains('\r'));
}

#[test]
fn test_extract_errors() {
    let ex = Extractor::new();
    assert!(matches!(
        ex.extract("a.md".into(), "---\nid: x\nno close"),
        Err(MemoryError::YamlError { .. })
    ));
    assert!(matches!(
        ex.extract("a.md".into(), "---\n: : :\n  - [\n---\nbody"),
        Err(MemoryError::YamlError { .. })
    ));
    assert!(matches!(
        ex.extract("a.md".into(), "---\n- a\n- b\n---\nbody"),
        Err(MemoryError::YamlError { .. })
    ));
    assert!(matches!(
        ex.extract("a.md".into(), "  \n"),
        Err(MemoryError::ExtractionError { .. })
    ));
}

#[test]
fn test_hash_changes_with_content() {
    assert_ne!(extract("a").hash, extract("b").hash);
    assert_eq!(extract("a").hash, extract("a").hash);
}

#[cfg(unix)]
#[test]
fn extract_file_rejects_symlink_and_non_regular_paths() {
    use std::os::unix::fs::symlink;

    let dir = tempfile::tempdir().expect("tmp");
    let target = dir.path().join("outside.md");
    std::fs::write(&target, "secret").expect("target");
    let link = dir.path().join("link.md");
    symlink(&target, &link).expect("symlink");
    assert!(Extractor::new().extract_file(&link).is_err());

    let subdir = dir.path().join("folder.md");
    std::fs::create_dir(&subdir).expect("directory");
    assert!(Extractor::new().extract_file(&subdir).is_err());
}

#[test]
fn extract_file_reads_regular_file() {
    let dir = tempfile::tempdir().expect("tmp");
    let path = dir.path().join("note.md");
    std::fs::write(&path, "hello").expect("note");
    assert_eq!(
        Extractor::new().extract_file(&path).expect("extract").body,
        "hello"
    );
}

#[test]
fn test_nested_headings_do_not_leave_a_bare_title_chunk() {
    // "## Overview" has no body of its own; its body sits under the "###" sub-heading.
    let src = "## Overview\n\n### Phases\n- one\n- two\n";
    let d = extract(src);
    let chunks = Chunker::new().chunk(&d).expect("chunk");
    assert!(
        chunks
            .iter()
            .all(|c| c.kind != ChunkKind::Heading || c.text.lines().count() > 1),
        "no heading-only chunk: {:?}",
        chunks.iter().map(|c| c.text.clone()).collect::<Vec<_>>()
    );
    let joined: String = chunks
        .iter()
        .map(|c| c.text.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(joined.contains("- two"), "body kept: {joined}");
}

#[test]
fn test_chunk_line_numbers_are_exact() {
    let src = "---\nid: 1\n---\n# Intro\n\nfirst para\n\n```rust\nfn x() {}\n```\n\n> quoted\n";
    let d = extract(src);
    let chunks = Chunker::new().chunk(&d).expect("chunk");
    let src_lines: Vec<&str> = src.lines().collect();
    // Every non-summary chunk text must equal the source lines it claims.
    for c in &chunks {
        let slice = src_lines[c.start_line - 1..c.end_line].join("\n");
        assert_eq!(c.text, slice, "chunk {:?}", c.kind);
    }
    let kinds: Vec<_> = chunks.iter().map(|c| c.kind).collect();
    assert_eq!(
        kinds,
        vec![
            ChunkKind::Summary,
            ChunkKind::Detail,
            ChunkKind::Code,
            ChunkKind::Quote
        ]
    );
    assert_eq!((chunks[0].start_line, chunks[0].end_line), (1, 3));
    assert_eq!(chunks[1].start_line, 4); // heading + paragraph share one chunk
    assert_eq!(chunks[1].end_line, 6);
    assert!(chunks[1].text.starts_with("# Intro\n\nfirst para"));
    assert_eq!(chunks[2].start_line, 8);
    assert_eq!(chunks[2].end_line, 10);
}

#[test]
fn test_chunk_section_metadata() {
    let d = extract("# Alpha\n\ntext one\n\n## Beta\n\ntext two\n");
    let chunks = Chunker::new().chunk(&d).expect("chunk");
    let detail: Vec<_> = chunks
        .iter()
        .filter(|c| c.kind == ChunkKind::Detail)
        .collect();
    assert_eq!(detail.len(), 2);
    assert_eq!(detail[0].metadata.as_ref().expect("m")["section"], "Alpha");
    assert_eq!(detail[1].metadata.as_ref().expect("m")["section"], "Beta");
}

#[test]
fn test_chunk_merges_small_paragraphs_and_splits_large() {
    let d = extract("one\n\ntwo\n\nthree");
    let merged = Chunker::new().chunk(&d).expect("chunk");
    assert_eq!(merged.len(), 1);
    assert_eq!(merged[0].text, "one\n\ntwo\n\nthree");
    assert_eq!((merged[0].start_line, merged[0].end_line), (1, 5));

    let big = (0..40)
        .map(|i| format!("line number {i}"))
        .collect::<Vec<_>>()
        .join("\n");
    let d = extract(&big);
    let c = Chunker {
        chunk_size: 100,
        overlap: 10,
    };
    let chunks = c.chunk(&d).expect("chunk");
    assert!(chunks.len() > 1);
    assert!(chunks.iter().all(|c| c.text.chars().count() <= 100));
    // Contiguous, non-overlapping line coverage for line-based splits.
    assert_eq!(chunks[0].start_line, 1);
    assert_eq!(chunks.last().expect("last").end_line, 40);
    for w in chunks.windows(2) {
        assert_eq!(w[0].end_line + 1, w[1].start_line);
    }
}

#[test]
fn test_chunk_long_single_line_uses_overlap() {
    let d = extract(&"x".repeat(250));
    let c = Chunker {
        chunk_size: 100,
        overlap: 20,
    };
    let chunks = c.chunk(&d).expect("chunk");
    assert_eq!(chunks.len(), 3); // 0-100, 80-180, 160-250
    assert!(chunks.iter().all(|c| c.start_line == 1 && c.end_line == 1));
}

#[test]
fn test_chunk_unicode_safe() {
    let d = extract(&"สวัสดี".repeat(60));
    let c = Chunker {
        chunk_size: 50,
        overlap: 5,
    };
    assert!(c.chunk(&d).is_ok());
}

#[test]
fn test_chunker_rejects_bad_settings() {
    let d = extract("x");
    assert!(Chunker {
        chunk_size: 0,
        overlap: 0
    }
    .chunk(&d)
    .is_err());
    assert!(Chunker {
        chunk_size: 10,
        overlap: 10
    }
    .chunk(&d)
    .is_err());
}

#[test]
fn test_chunk_unterminated_fence_runs_to_eof() {
    let d = extract("```\ncode\nmore");
    let chunks = Chunker::new().chunk(&d).expect("chunk");
    assert_eq!(chunks.len(), 1);
    assert_eq!(chunks[0].kind, ChunkKind::Code);
    assert_eq!(chunks[0].end_line, 3);
}

#[test]
fn test_coalesce_events() {
    let p = |s: &str| PathBuf::from(s);
    let ev = |s: &str, k| FileEvent {
        path: p(s),
        kind: k,
    };
    let out = Watcher::coalesce(vec![
        ev("a.md", EventKind::Added),
        ev("a.md", EventKind::Modified),
        ev("b.md", EventKind::Modified),
        ev("b.md", EventKind::Deleted),
        ev("c.md", EventKind::Deleted),
        ev("c.md", EventKind::Added),
    ]);
    let got: Vec<_> = out
        .iter()
        .map(|e| (e.path.to_str().expect("p"), e.kind))
        .collect();
    assert_eq!(
        got,
        vec![
            ("a.md", EventKind::Added),
            ("b.md", EventKind::Deleted),
            ("c.md", EventKind::Modified)
        ]
    );
}

#[test]
fn test_is_indexable_and_scan_dir() {
    assert!(is_indexable(Path::new("a.MD")));
    assert!(!is_indexable(Path::new("a.json")));
    let dir = tempfile::tempdir().expect("tmp");
    std::fs::create_dir(dir.path().join("sub")).expect("mkdir");
    std::fs::write(dir.path().join("b.md"), "x").expect("w");
    std::fs::write(dir.path().join("sub").join("a.md"), "x").expect("w");
    std::fs::write(dir.path().join("c.json"), "x").expect("w");
    let files = scan_dir(dir.path());
    assert_eq!(files.len(), 2);
}

#[test]
fn test_scan_dir_skips_hidden_and_build_dirs_by_default() {
    let dir = tempfile::tempdir().expect("tmp");
    let root = dir.path();
    for d in [
        ".obsidian",
        ".git",
        ".claude/agents",
        "target",
        "node_modules",
        "notes",
        ".plaesy/memory",
    ] {
        std::fs::create_dir_all(root.join(d)).expect("mkdir");
    }
    for f in [
        ".obsidian/plugin.md",
        ".git/notes.md",
        ".claude/agents/a.md",
        "target/build.md",
        "node_modules/x.md",
        "notes/keep.md",
        ".plaesy/memory/keep.md",
    ] {
        std::fs::write(root.join(f), "x").expect("w");
    }
    let mut names: Vec<String> = scan_dir(root)
        .iter()
        .map(|p| {
            p.strip_prefix(root)
                .expect("rel")
                .to_string_lossy()
                .replace('\\', "/")
        })
        .collect();
    names.sort();
    assert_eq!(names, vec![".plaesy/memory/keep.md", "notes/keep.md"]);
    assert!(is_excluded(root, &root.join(".obsidian/plugin.md"), &[]));
    assert!(!is_excluded(
        root,
        &root.join(".plaesy/memory/keep.md"),
        &[]
    ));
}

#[test]
fn test_is_excluded_matches_folder_and_children_only() {
    let root = Path::new("/vault");
    let ex = vec![".plaesy/templates".to_string()];
    assert!(is_excluded(
        root,
        Path::new("/vault/.plaesy/templates/a.md"),
        &ex
    ));
    assert!(is_excluded(
        root,
        Path::new("/vault/.plaesy/templates"),
        &ex
    ));
    assert!(is_excluded(
        root,
        Path::new("/vault/01 Projects/X/.plaesy/templates/a.md"),
        &ex
    ));
    assert!(!is_excluded(
        root,
        Path::new("/vault/.plaesy/memory/a.md"),
        &ex
    ));
    // A prefix of a folder name is not the folder itself.
    assert!(!is_excluded(
        root,
        Path::new("/vault/.plaesy/templates-old/a.md"),
        &ex
    ));
    assert!(!is_excluded(
        root,
        Path::new("/vault/.plaesy/templates/a.md"),
        &[]
    ));
}

#[tokio::test]
async fn test_watcher_rejects_missing_dir() {
    assert!(matches!(
        Watcher::new("definitely/not/a/dir").await,
        Err(MemoryError::WatcherError(_))
    ));
}

#[tokio::test]
async fn test_watcher_reports_new_file() {
    let dir = tempfile::tempdir().expect("tmp");
    let mut w = Watcher::new(dir.path().to_str().expect("utf8"))
        .await
        .expect("watcher")
        .with_debounce(Duration::from_millis(100));
    let file = dir.path().join("new.md");
    let f2 = file.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(200)).await;
        std::fs::write(f2, "hello").expect("write");
    });
    let events = tokio::time::timeout(Duration::from_secs(15), w.changes())
        .await
        .expect("timed out waiting for event")
        .expect("changes");
    assert!(
        events
            .iter()
            .any(|e| e.path.file_name() == file.file_name()),
        "events: {events:?}"
    );
}

#[test]
fn decode_text_keeps_utf8_and_falls_back_to_latin1() {
    assert_eq!(decode_text("halo é".as_bytes().to_vec()), "halo é");
    // 0xE9 adalah "é" di ISO-8859-1 dan bukan UTF-8 yang valid.
    assert_eq!(decode_text(vec![b'c', b'a', b'f', 0xE9]), "café");
}
