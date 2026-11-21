//! Tiers, source trust, staleness, wikilinks and hygiene, end to end (keyword-only engine,
//! so these tests run without the ONNX model).

use plaesy::memory::config::SearchConfig;
use plaesy::memory::hygiene::HygieneOptions;
use plaesy::memory::search::{CitationBuilder, SearchEngine, SearchFilter, SearchMode, Trust};
use plaesy::memory::storage::{Database, DatabaseConfig};
use std::path::PathBuf;
use std::sync::Arc;

fn today() -> String {
    chrono::Utc::now().format("%Y-%m-%d").to_string()
}

fn write(root: &std::path::Path, rel: &str, body: &str) {
    let p = root.join(rel);
    std::fs::create_dir_all(p.parent().expect("parent")).expect("mkdir");
    std::fs::write(p, body).expect("write");
}

async fn vault() -> (SearchEngine, tempfile::TempDir) {
    let dir = tempfile::tempdir().expect("tmp");
    let t = today();
    write(
        dir.path(),
        "10-episodic/2026-10-01.md",
        "# Log\n\nWe discussed the deploy ritual on Tuesday.\n",
    );
    write(
        dir.path(),
        "20-semantic/deploy.md",
        &format!(
            "---\ntier: semantic\nconfidence: high\nlast_verified: {t}\n---\n# Deploy\n\nThe deploy ritual runs through [[runbook]].\n"
        ),
    );
    write(
        dir.path(),
        "20-semantic/old-deploy.md",
        "---\ntier: semantic\nconfidence: medium\nlast_verified: 2025-01-01\nstatus: superseded\n---\n# Old deploy\n\nThe deploy ritual used a manual FTP upload.\n",
    );
    write(
        dir.path(),
        "20-semantic/stale-deploy.md",
        "---\ntier: semantic\nconfidence: high\nlast_verified: 2020-01-01\n---\n# Stale deploy\n\nThe deploy ritual happens on Fridays.\n",
    );
    write(
        dir.path(),
        "30-procedural/runbook.md",
        &format!(
            "---\ntier: procedural\nconfidence: low\nlast_verified: {t}\n---\n# Runbook\n\nStep one: see [[missing-note]] and run `scripts/ghost.sh`.\nThen `Cargo.toml` stays.\n"
        ),
    );
    let db = Arc::new(
        Database::new(DatabaseConfig {
            db_path: PathBuf::from(":memory:"),
            ..DatabaseConfig::default()
        })
        .await
        .expect("db"),
    );
    let e = SearchEngine::new(db, None, SearchConfig::default())
        .await
        .expect("engine");
    e.index_directory(dir.path()).await.expect("index");
    (e, dir)
}

fn paths(rs: &[plaesy::memory::storage::SearchResult]) -> Vec<String> {
    rs.iter().map(|r| r.source_path.clone()).collect()
}

#[tokio::test]
async fn tier_filter_uses_frontmatter_and_path() {
    let (e, _d) = vault().await;
    let f = |tier: &str| SearchFilter {
        tiers: vec![tier.into()],
        ..SearchFilter::default()
    };
    let epi = e
        .search_with("deploy ritual", 10, SearchMode::Keyword, &f("episodic"))
        .await
        .expect("search");
    assert_eq!(
        paths(&epi),
        vec!["10-episodic/2026-10-01.md"],
        "tier inferred from the path"
    );

    let proc_ = e
        .search_with("runbook", 10, SearchMode::Hybrid, &f("procedural"))
        .await
        .expect("search");
    assert!(proc_
        .iter()
        .all(|r| r.source_path.starts_with("30-procedural/")));

    let sem = e
        .search_with("deploy ritual", 10, SearchMode::Keyword, &f("knowledge"))
        .await
        .expect("search");
    assert!(
        sem.iter()
            .all(|r| r.source_path.starts_with("20-semantic/")),
        "alias of semantic"
    );
}

#[tokio::test]
async fn project_filter_limits_results_to_owner_and_shared_scope() {
    let (e, dir) = vault().await;
    write(
        dir.path(),
        "20-semantic/studio-a.md",
        "---\ntier: semantic\nproject: studio-a\n---\n# Studio A\n\nThe deploy ritual is project Alpha.\n",
    );
    write(
        dir.path(),
        "20-semantic/shared.md",
        "---\ntier: semantic\nscope: [studio-a, studio-b]\n---\n# Shared\n\nThe deploy ritual is shared across projects.\n",
    );
    write(
        dir.path(),
        "20-semantic/studio-b.md",
        "---\ntier: semantic\nproject: studio-b\n---\n# Studio B\n\nThe deploy ritual is project Beta.\n",
    );
    e.index_directory(dir.path()).await.expect("reindex");
    let results = e
        .search_with(
            "deploy ritual",
            20,
            SearchMode::Keyword,
            &SearchFilter {
                project: Some("STUDIO-A".into()),
                ..SearchFilter::default()
            },
        )
        .await
        .expect("project-scoped search");
    let got = paths(&results);
    assert!(got.contains(&"20-semantic/studio-a.md".into()));
    assert!(got.contains(&"20-semantic/shared.md".into()));
    assert!(!got.contains(&"20-semantic/studio-b.md".into()));
    assert!(results.iter().all(|result| {
        CitationBuilder::new()
            .build(result)
            .is_ok_and(|citation| citation.to_string().contains(&result.source_path))
    }));

    let unknown = e
        .search_with(
            "deploy ritual",
            20,
            SearchMode::Keyword,
            &SearchFilter {
                project: Some("unknown".into()),
                ..SearchFilter::default()
            },
        )
        .await
        .expect("unknown project search");
    assert!(unknown.is_empty());
}

#[tokio::test]
async fn superseded_hidden_unless_requested() {
    let (e, _d) = vault().await;
    let default = e.search("manual FTP upload", 10).await.expect("search");
    assert!(
        default.is_empty(),
        "superseded note must not surface by default: {default:?}"
    );
    let all = e
        .search_with(
            "manual FTP upload",
            10,
            SearchMode::Keyword,
            &SearchFilter {
                include_inactive: true,
                ..SearchFilter::default()
            },
        )
        .await
        .expect("search");
    assert_eq!(paths(&all), vec!["20-semantic/old-deploy.md"]);
    assert_eq!(all[0].meta.status.as_deref(), Some("superseded"));
}

#[tokio::test]
async fn stale_sources_are_flagged_and_outranked_by_fresh_ones() {
    let (e, _d) = vault().await;
    let r = e
        .search_with(
            "deploy ritual",
            10,
            SearchMode::Keyword,
            &SearchFilter {
                tiers: vec!["semantic".into()],
                ..SearchFilter::default()
            },
        )
        .await
        .expect("search");
    let stale = r
        .iter()
        .find(|x| x.source_path.ends_with("stale-deploy.md"))
        .expect("stale hit");
    let fresh = r
        .iter()
        .find(|x| x.source_path.ends_with("/deploy.md"))
        .expect("fresh hit");
    assert!(stale.meta.stale && !fresh.meta.stale);
    assert!(
        fresh.score > stale.score,
        "fresh {} vs stale {}",
        fresh.score,
        stale.score
    );
    let shown = CitationBuilder::new()
        .build(stale)
        .expect("citation")
        .to_string();
    assert!(
        shown.contains("STALE") && shown.contains("verified 2020-01-01"),
        "{shown}"
    );
}

#[tokio::test]
async fn min_trust_excludes_low_and_undeclared() {
    let (e, _d) = vault().await;
    let r = e
        .search_with(
            "deploy runbook ritual",
            20,
            SearchMode::Keyword,
            &SearchFilter {
                min_trust: Some(Trust::High),
                ..SearchFilter::default()
            },
        )
        .await
        .expect("search");
    let got = paths(&r);
    assert!(got.contains(&"20-semantic/deploy.md".to_string()));
    assert!(
        !got.iter().any(|p| p.contains("runbook")),
        "confidence: low is excluded"
    );
    assert!(
        !got.iter().any(|p| p.contains("episodic")),
        "no declared trust is excluded"
    );
}

#[tokio::test]
async fn backlinks_resolve_by_stem_and_report_file_lines() {
    let (e, _d) = vault().await;
    let back = e.links("runbook", true).await.expect("backlinks");
    assert_eq!(back.len(), 1);
    assert_eq!(back[0].source_path, "20-semantic/deploy.md");
    assert_eq!(
        back[0].resolved_path.as_deref(),
        Some("30-procedural/runbook.md")
    );
    assert_eq!(
        back[0].line, 8,
        "line counts the 5 frontmatter lines + heading + blank"
    );

    let out = e
        .links("30-procedural/runbook.md", false)
        .await
        .expect("outlinks");
    assert_eq!(out.len(), 1);
    assert!(
        out[0].resolved_path.is_none(),
        "[[missing-note]] is dangling"
    );
}

#[tokio::test]
async fn hygiene_reports_each_problem_kind() {
    let (e, d) = vault().await;
    write(
        d.path(),
        "templates/x.template.md",
        "# T\n\nUse `scripts/not-yet.sh` after copying.\n",
    );
    e.index_directory(d.path()).await.expect("reindex");
    let report = e
        .hygiene(&HygieneOptions {
            root: Some(d.path().to_path_buf()),
            ..HygieneOptions::default()
        })
        .await
        .expect("hygiene");
    assert_eq!(report.checked, 6);
    assert!(
        !report
            .findings
            .iter()
            .any(|f| f.path.starts_with("templates/")),
        "templates are not checked for dead paths"
    );
    let has = |kind: &str, path: &str| {
        report
            .findings
            .iter()
            .any(|f| f.kind == kind && f.path == path)
    };
    assert!(has("stale", "20-semantic/stale-deploy.md"));
    assert!(has(
        "superseded_without_replacement",
        "20-semantic/old-deploy.md"
    ));
    assert!(has("broken_link", "30-procedural/runbook.md"));
    assert!(has("dead_path_reference", "30-procedural/runbook.md"));
    assert!(
        !report
            .findings
            .iter()
            .any(|f| f.kind == "dead_path_reference" && f.detail.contains("Cargo.toml")),
        "a bare file name is not treated as a path reference"
    );
    // The episodic log needs no verification metadata.
    assert!(!report
        .findings
        .iter()
        .any(|f| f.path.starts_with("10-episodic/")));
    let ghost = report
        .findings
        .iter()
        .find(|f| f.kind == "dead_path_reference")
        .expect("ghost");
    assert_eq!(ghost.label, "INFERRED");
    assert_eq!(ghost.line, Some(8), "file line, not body line");
    // old-deploy (2025) and stale-deploy (2020) are both past the 180-day window.
    assert_eq!(report.summary.get("stale"), Some(&2));
}

#[tokio::test]
async fn hygiene_reports_invalid_and_inverted_validity_ranges() {
    let (e, d) = vault().await;
    write(
        d.path(),
        "20-semantic/invalid-validity.md",
        "---\ntier: semantic\nconfidence: high\nlast_verified: 2026-11-12\nvalid_from: \"not-a-date\"\nvalid_to: \"2026-11-12\"\n---\n# Invalid validity\n\nThis note has an invalid start date.\n",
    );
    write(
        d.path(),
        "20-semantic/inverted-validity.md",
        "---\ntier: semantic\nconfidence: high\nlast_verified: 2026-11-12\nvalid_from: \"2026-11-13\"\nvalid_to: \"2026-11-12\"\n---\n# Inverted validity\n\nThis note ends before it starts.\n",
    );
    e.index_directory(d.path()).await.expect("reindex");
    let report = e
        .hygiene(&HygieneOptions {
            root: Some(d.path().to_path_buf()),
            ..HygieneOptions::default()
        })
        .await
        .expect("hygiene");
    assert!(report.findings.iter().any(|f| {
        f.kind == "invalid_validity_date" && f.path == "20-semantic/invalid-validity.md"
    }));
    assert!(report.findings.iter().any(|f| {
        f.kind == "inverted_validity_range" && f.path == "20-semantic/inverted-validity.md"
    }));
}

#[tokio::test]
async fn hygiene_reports_duplicate_graph_id_and_self_superseding_decision() {
    let dir = tempfile::tempdir().expect("tmp");
    let t = today();
    write(
        dir.path(),
        "projects/a.md",
        &format!(
            "---\ntier: semantic\nconfidence: high\nlast_verified: {t}\ntype: \"project\"\nid: \"dup-project\"\nstatus: \"active\"\n---\n# Project A\n\nFirst note with this id.\n"
        ),
    );
    write(
        dir.path(),
        "projects/b.md",
        &format!(
            "---\ntier: semantic\nconfidence: high\nlast_verified: {t}\ntype: \"project\"\nid: \"dup-project\"\nstatus: \"active\"\n---\n# Project B\n\nSecond note, same id, written outside the `remember` CLI path.\n"
        ),
    );
    write(
        dir.path(),
        "decisions/self.md",
        &format!(
            "---\ntier: semantic\nconfidence: high\nlast_verified: {t}\ntype: \"decision\"\nid: \"self-ref\"\nsupersedes: [\"self-ref\"]\n---\n# Self Ref\n\nClaims to supersede itself.\n"
        ),
    );
    write(
        dir.path(),
        "decisions/dangling.md",
        &format!(
            "---\ntier: semantic\nconfidence: high\nlast_verified: {t}\ntype: \"decision\"\nid: \"new-decision\"\nsupersedes: [\"missing-decision\"]\n---\n# New Decision\n\nReferences a missing predecessor.\n"
        ),
    );
    write(
        dir.path(),
        "decisions/dangling-replacement.md",
        &format!(
            "---\ntier: semantic\nconfidence: high\nlast_verified: {t}\ntype: \"decision\"\nid: \"old-decision\"\nstatus: \"superseded\"\nsuperseded_by: \"missing-replacement\"\n---\n# Old Decision\n\nReplacement ID does not exist.\n"
        ),
    );
    write(
        dir.path(),
        "decisions/inconsistent-replacement.md",
        &format!(
            "---\ntier: semantic\nconfidence: high\nlast_verified: {t}\ntype: \"decision\"\nid: \"inconsistent-old\"\nstatus: \"superseded\"\nsuperseded_by: \"replacement-decision\"\n---\n# Inconsistent Old\n\nReplacement does not reciprocate.\n"
        ),
    );
    write(
        dir.path(),
        "decisions/replacement.md",
        &format!(
            "---\ntier: semantic\nconfidence: high\nlast_verified: {t}\ntype: \"decision\"\nid: \"replacement-decision\"\nstatus: \"active\"\n---\n# Replacement\n\nDoes not list predecessor in supersedes.\n"
        ),
    );
    write(
        dir.path(),
        "decisions/cycle-c.md",
        &format!(
            "---\ntier: semantic\nconfidence: high\nlast_verified: {t}\ntype: \"decision\"\nid: \"cycle-c\"\nstatus: \"active\"\nsupersedes: [\"cycle-d\"]\n---\n# Cycle C\n\nSecond independent cycle.\n"
        ),
    );
    write(
        dir.path(),
        "decisions/cycle-d.md",
        &format!(
            "---\ntier: semantic\nconfidence: high\nlast_verified: {t}\ntype: \"decision\"\nid: \"cycle-d\"\nstatus: \"active\"\nsupersedes: [\"cycle-c\"]\n---\n# Cycle D\n\nSecond independent cycle.\n"
        ),
    );
    write(
        dir.path(),
        "decisions/cycle-bridge.md",
        &format!(
            "---\ntier: semantic\nconfidence: high\nlast_verified: {t}\ntype: \"decision\"\nid: \"cycle-bridge\"\nstatus: \"active\"\nsupersedes: [\"cycle-c\"]\n---\n# Cycle Bridge\n\nConnects one cycle to another.\n"
        ),
    );
    write(
        dir.path(),
        "decisions/cycle-tail.md",
        &format!(
            "---\ntier: semantic\nconfidence: high\nlast_verified: {t}\ntype: \"decision\"\nid: \"cycle-tail\"\nstatus: \"active\"\nsupersedes: [\"cycle-a\"]\n---\n# Cycle Tail\n\nPoints into a cycle without being part of it.\n"
        ),
    );
    write(
        dir.path(),
        "decisions/ambiguous-a.md",
        &format!(
            "---\ntier: semantic\nconfidence: high\nlast_verified: {t}\ntype: \"decision\"\nid: \"ambiguous-decision\"\nstatus: \"active\"\nsupersedes: [\"cycle-edge\"]\n---\n# Ambiguous A\n\nDuplicate decision id.\n"
        ),
    );
    write(
        dir.path(),
        "decisions/ambiguous-b.md",
        &format!(
            "---\ntier: semantic\nconfidence: high\nlast_verified: {t}\ntype: \"decision\"\nid: \"ambiguous-decision\"\nstatus: \"active\"\nsupersedes: [\"cycle-edge\"]\n---\n# Ambiguous B\n\nDuplicate decision id.\n"
        ),
    );
    write(
        dir.path(),
        "decisions/ambiguous-edge.md",
        &format!(
            "---\ntier: semantic\nconfidence: high\nlast_verified: {t}\ntype: \"decision\"\nid: \"cycle-edge\"\nstatus: \"active\"\nsupersedes: [\"ambiguous-decision\"]\n---\n# Ambiguous Edge\n\nMust not be treated as a definite cycle through duplicate ids.\n"
        ),
    );
    write(
        dir.path(),
        "decisions/cycle-a.md",
        &format!(
            "---\ntier: semantic\nconfidence: high\nlast_verified: {t}\ntype: \"decision\"\nid: \"cycle-a\"\nstatus: \"active\"\nsupersedes: [\"cycle-b\"]\n---\n# Cycle A\n\nPart of a two-note replacement cycle.\n"
        ),
    );
    write(
        dir.path(),
        "decisions/cycle-b.md",
        &format!(
            "---\ntier: semantic\nconfidence: high\nlast_verified: {t}\ntype: \"decision\"\nid: \"cycle-b\"\nstatus: \"active\"\nsupersedes: [\"cycle-a\"]\n---\n# Cycle B\n\nPart of a two-note replacement cycle.\n"
        ),
    );
    write(
        dir.path(),
        "decisions/empty-replacement.md",
        &format!(
            "---\ntier: semantic\nconfidence: high\nlast_verified: {t}\ntype: \"decision\"\nid: \"empty-old\"\nstatus: \"superseded\"\nsuperseded_by: \"   \"\n---\n# Empty Replacement\n\nReplacement ID is blank.\n"
        ),
    );
    let db = Arc::new(
        Database::new(DatabaseConfig {
            db_path: PathBuf::from(":memory:"),
            ..DatabaseConfig::default()
        })
        .await
        .expect("db"),
    );
    let e = SearchEngine::new(db, None, SearchConfig::default())
        .await
        .expect("engine");
    e.index_directory(dir.path()).await.expect("index");
    let report = e
        .hygiene(&HygieneOptions {
            root: Some(dir.path().to_path_buf()),
            ..HygieneOptions::default()
        })
        .await
        .expect("hygiene");
    assert!(
        report
            .findings
            .iter()
            .any(|f| f.kind == "duplicate_graph_id" && f.detail.contains("dup-project")),
        "a duplicate id written outside `remember` (e.g. by hand, or by a future auto-capture \
         path) must still be caught by hygiene, not just rejected at write time"
    );
    assert!(
        report
            .findings
            .iter()
            .any(|f| f.kind == "self_superseding_decision" && f.path == "decisions/self.md"),
        "a decision that lists its own id in `supersedes` must be flagged"
    );
    assert!(
        report.findings.iter().any(|f| {
            f.kind == "dangling_superseded_by_reference"
                && f.path == "decisions/dangling-replacement.md"
                && f.detail.contains("missing-replacement")
        }),
        "a `superseded_by` reference without a matching decision must be flagged"
    );
    assert!(
        report.findings.iter().any(|f| {
            f.kind == "supersession_cycle"
                && f.path == "(graph)"
                && f.detail.contains("cycle-a")
                && f.detail.contains("cycle-b")
                && f.detail.contains("cycle-c")
                && f.detail.contains("cycle-d")
                && !f.detail.contains("cycle-bridge")
                && !f.detail.contains("ambiguous-decision")
                && !f.detail.contains("cycle-edge")
                && !f.detail.contains("cycle-tail")
        }),
        "report actual cycle members, excluding ambiguous IDs and nodes that only lead into a cycle"
    );
    assert!(
        report.findings.iter().any(|f| {
            f.kind == "inconsistent_supersession_reference"
                && f.path == "decisions/inconsistent-replacement.md"
                && f.detail.contains("replacement-decision")
        }),
        "a superseded_by relationship must be reciprocal with the replacement's supersedes field"
    );
    assert!(
        report.findings.iter().any(|f| {
            f.kind == "superseded_without_replacement" && f.path == "decisions/empty-replacement.md"
        }),
        "a blank `superseded_by` value is missing a usable replacement"
    );
    assert!(
        !report.findings.iter().any(|f| {
            f.kind == "dangling_superseded_by_reference" && f.path == "decisions/dangling.md"
        }),
        "a decision without a replacement field is a different hygiene issue"
    );
    assert!(
        report.findings.iter().any(|f| {
            f.kind == "dangling_supersedes_reference"
                && f.path == "decisions/dangling.md"
                && f.detail.contains("missing-decision")
        }),
        "a missing supersession target must be flagged"
    );
    assert!(
        !report
            .findings
            .iter()
            .any(|f| f.kind == "dangling_supersedes_reference" && f.path == "decisions/self.md"),
        "self-reference is not dangling when the decision exists"
    );
    assert!(
        !report
            .findings
            .iter()
            .any(|f| f.kind == "dangling_supersedes_reference" && f.path == "decisions/a.md"),
        "an existing duplicate ID is not dangling"
    );
}
