//! Unit tests for `tests` split out of `mod.rs`.
//! Kept as a separate file so they are easy to find; included via `#[path]`.

use super::*;
use std::collections::HashMap as Map;

fn doc(path: &str, meta: &[(&str, Value)]) -> StoredDocument {
    let mut metadata = Map::new();
    for (k, v) in meta {
        metadata.insert(k.to_string(), v.clone());
    }
    StoredDocument {
        path: path.to_string(),
        metadata,
        body: String::new(),
        body_start_line: 1,
    }
}

fn s(v: &str) -> Value {
    Value::String(v.to_string())
}

fn graph_of(docs: Vec<StoredDocument>) -> Graph {
    let mut notes = Vec::new();
    let mut malformed = Vec::new();
    for d in &docs {
        match decode(d) {
            None => {}
            Some(Ok(n)) => notes.push(n),
            Some(Err(reason)) => malformed.push(MalformedNote {
                path: d.path.clone(),
                note_type: as_str(d.metadata.get("type")).unwrap_or_default(),
                reason,
            }),
        }
    }
    Graph { notes, malformed }
}

#[test]
fn note_without_type_is_ignored_not_malformed() {
    let g = graph_of(vec![doc("a.md", &[("title", s("plain note"))])]);
    assert!(g.notes.is_empty());
    assert!(g.malformed.is_empty());
}

#[test]
fn decision_missing_id_is_skipped_with_hygiene_reason() {
    let g = graph_of(vec![doc("d.md", &[("type", s("decision"))])]);
    assert!(g.notes.is_empty());
    assert_eq!(g.malformed.len(), 1);
    assert!(g.malformed[0].reason.contains("missing a required 'id'"));
}

#[test]
fn unknown_type_is_malformed_not_silently_dropped() {
    let g = graph_of(vec![doc("x.md", &[("type", s("unknown")), ("id", s("x"))])]);
    assert!(g.notes.is_empty());
    assert_eq!(g.malformed.len(), 1);
}

#[test]
fn invalid_status_for_type_is_malformed() {
    let g = graph_of(vec![doc(
        "p.md",
        &[
            ("type", s("project")),
            ("id", s("p1")),
            ("status", s("superseded")),
        ],
    )]);
    assert!(g.notes.is_empty());
    assert_eq!(g.malformed.len(), 1);
}

#[test]
fn empty_vault_lists_nothing() {
    let g = graph_of(vec![]);
    assert!(g.projects().is_empty());
}

#[test]
fn duplicate_exact_project_ids_are_ambiguous() {
    let g = graph_of(vec![
        doc(
            "a.md",
            &[("type", s("project")), ("id", s("same")), ("title", s("A"))],
        ),
        doc(
            "b.md",
            &[("type", s("project")), ("id", s("same")), ("title", s("B"))],
        ),
    ]);
    match resolve_project(&g, "same") {
        Resolved::Ambiguous { candidates, .. } => assert_eq!(candidates.len(), 2),
        _ => panic!("duplicate ids must not resolve to the first claimant"),
    }
}

#[test]
fn resolve_exact_id() {
    let g = graph_of(vec![doc(
        "a.md",
        &[
            ("type", s("project")),
            ("id", s("studio-a")),
            ("title", s("Studio A")),
        ],
    )]);
    assert!(matches!(
        resolve_project(&g, "studio-a"),
        Resolved::Found(_)
    ));
}

#[test]
fn resolve_by_title_case_insensitive() {
    let g = graph_of(vec![doc(
        "a.md",
        &[
            ("type", s("project")),
            ("id", s("studio-a")),
            ("title", s("Studio A")),
        ],
    )]);
    match resolve_project(&g, "studio a") {
        Resolved::Found(n) => assert_eq!(n.id, "studio-a"),
        _ => panic!("expected Found"),
    }
}

#[test]
fn resolve_unique_prefix_is_case_insensitive() {
    let g = graph_of(vec![doc(
        "a.md",
        &[
            ("type", s("project")),
            ("id", s("Studio-A")),
            ("title", s("Studio A")),
        ],
    )]);
    assert!(matches!(resolve_project(&g, "STUDIO"), Resolved::Found(_)));
}

#[test]
fn resolve_unique_prefix() {
    let g = graph_of(vec![doc(
        "a.md",
        &[
            ("type", s("project")),
            ("id", s("8f3a2c1e")),
            ("title", s("studio")),
        ],
    )]);
    match resolve_project(&g, "8f3a") {
        Resolved::Found(n) => assert_eq!(n.id, "8f3a2c1e"),
        _ => panic!("expected Found"),
    }
}

#[test]
fn resolve_ambiguous_prefix_lists_every_candidate() {
    let g = graph_of(vec![
        doc(
            "a.md",
            &[
                ("type", s("project")),
                ("id", s("8f3a-one")),
                ("title", s("A")),
            ],
        ),
        doc(
            "b.md",
            &[
                ("type", s("project")),
                ("id", s("8f3a-two")),
                ("title", s("B")),
            ],
        ),
    ]);
    match resolve_project(&g, "8f3a") {
        Resolved::Ambiguous { candidates, .. } => assert_eq!(candidates.len(), 2),
        _ => panic!("expected Ambiguous"),
    }
}

#[test]
fn resolve_not_found_suggests_fuzzy_matches() {
    let g = graph_of(vec![doc(
        "a.md",
        &[
            ("type", s("project")),
            ("id", s("autonomous-venture-studio")),
            ("title", s("Autonomous Venture Studio")),
        ],
    )]);
    match resolve_project(&g, "venture") {
        Resolved::NotFound { suggestions, .. } => assert_eq!(suggestions.len(), 1),
        _ => panic!("expected NotFound"),
    }
}

#[test]
fn resolve_not_found_with_no_suggestions_when_nothing_close() {
    let g = graph_of(vec![doc(
        "a.md",
        &[
            ("type", s("project")),
            ("id", s("studio-a")),
            ("title", s("Studio A")),
        ],
    )]);
    match resolve_project(&g, "zzzz") {
        Resolved::NotFound { suggestions, .. } => assert!(suggestions.is_empty()),
        _ => panic!("expected NotFound"),
    }
}

#[test]
fn captures_list_uses_project_status_and_limit_filters() {
    let g = graph_of(vec![
        doc(
            "one.md",
            &[
                ("type", s("capture")),
                ("id", s("c1")),
                ("project", s("p1")),
                ("status", s("active")),
            ],
        ),
        doc(
            "two.md",
            &[
                ("type", s("capture")),
                ("id", s("c2")),
                ("scope", Value::Array(vec![s("p1")])),
                ("status", s("archived")),
            ],
        ),
        doc(
            "three.md",
            &[
                ("type", s("capture")),
                ("id", s("c3")),
                ("project", s("p2")),
                ("status", s("active")),
            ],
        ),
    ]);
    let filtered = ListFilter {
        projects: vec!["p1".into()],
        status: vec!["archived".into()],
        include_inactive: false,
        limit: 10,
    };
    let captures = list(&g, "capture", &filtered);
    assert_eq!(
        captures.iter().map(|n| n.id.as_str()).collect::<Vec<_>>(),
        vec!["c2"]
    );
    let default_visible = list(&g, "capture", &ListFilter::default());
    assert_eq!(
        default_visible
            .iter()
            .map(|n| n.id.as_str())
            .collect::<Vec<_>>(),
        vec!["c1", "c3"]
    );
    let limited = ListFilter {
        limit: 1,
        ..ListFilter::default()
    };
    assert_eq!(list(&g, "capture", &limited).len(), 1);
}

#[test]
fn decisions_default_hides_superseded_and_revoked() {
    let g = graph_of(vec![
        doc(
            "d1.md",
            &[
                ("type", s("decision")),
                ("id", s("d1")),
                ("status", s("active")),
            ],
        ),
        doc(
            "d2.md",
            &[
                ("type", s("decision")),
                ("id", s("d2")),
                ("status", s("superseded")),
            ],
        ),
    ]);
    let shown = list(&g, "decision", &ListFilter::default());
    assert_eq!(shown.len(), 1);
    assert_eq!(shown[0].id, "d1");
}

#[test]
fn include_inactive_shows_everything() {
    let g = graph_of(vec![
        doc(
            "d1.md",
            &[
                ("type", s("decision")),
                ("id", s("d1")),
                ("status", s("active")),
            ],
        ),
        doc(
            "d2.md",
            &[
                ("type", s("decision")),
                ("id", s("d2")),
                ("status", s("superseded")),
            ],
        ),
    ]);
    let filter = ListFilter {
        include_inactive: true,
        ..ListFilter::default()
    };
    assert_eq!(list(&g, "decision", &filter).len(), 2);
}

#[test]
fn explicit_status_filter_is_case_insensitive() {
    let g = graph_of(vec![doc(
        "d2.md",
        &[
            ("type", s("decision")),
            ("id", s("d2")),
            ("status", s("superseded")),
        ],
    )]);
    let filter = ListFilter {
        status: vec!["SUPERSEDED".into()],
        ..ListFilter::default()
    };
    assert_eq!(list(&g, "decision", &filter).len(), 1);
}

#[test]
fn project_filter_matches_owner_and_scope_case_insensitively() {
    let note = GraphNote {
        path: "d.md".into(),
        id: "d1".into(),
        note_type: "decision".into(),
        title: "Decision".into(),
        status: None,
        project: Some("Studio-A".into()),
        scope: vec!["Studio-B".into()],
        supersedes: Vec::new(),
        updated: None,
    };
    assert!(note.belongs_to("studio-a"));
    assert!(note.belongs_to("studio-b"));
    assert!(!note.is_shared_only("studio-a"));
    assert!(note.is_shared_only("studio-b"));
}

#[test]
fn project_filter_matches_direct_owner() {
    let g = graph_of(vec![doc(
        "d.md",
        &[
            ("type", s("decision")),
            ("id", s("d1")),
            ("project", s("studio-a")),
        ],
    )]);
    let filter = ListFilter {
        projects: vec!["studio-a".into()],
        ..ListFilter::default()
    };
    assert_eq!(list(&g, "decision", &filter).len(), 1);
}

#[test]
fn project_filter_matches_via_scope() {
    let g = graph_of(vec![doc(
        "d.md",
        &[
            ("type", s("decision")),
            ("id", s("d1")),
            ("project", s("studio-b")),
            ("scope", Value::Array(vec![s("studio-a")])),
        ],
    )]);
    let filter = ListFilter {
        projects: vec!["studio-a".into()],
        ..ListFilter::default()
    };
    let shown = list(&g, "decision", &filter);
    assert_eq!(shown.len(), 1);
    assert!(shown[0].is_shared_only("studio-a"));
}

#[test]
fn multiple_project_flags_are_union_not_intersection() {
    let g = graph_of(vec![
        doc(
            "d1.md",
            &[
                ("type", s("decision")),
                ("id", s("d1")),
                ("project", s("a")),
            ],
        ),
        doc(
            "d2.md",
            &[
                ("type", s("decision")),
                ("id", s("d2")),
                ("project", s("b")),
            ],
        ),
        doc(
            "d3.md",
            &[
                ("type", s("decision")),
                ("id", s("d3")),
                ("project", s("c")),
            ],
        ),
    ]);
    let filter = ListFilter {
        projects: vec!["a".into(), "b".into()],
        ..ListFilter::default()
    };
    assert_eq!(list(&g, "decision", &filter).len(), 2);
}

#[test]
fn project_filter_with_unknown_id_is_empty_not_an_error() {
    let g = graph_of(vec![doc(
        "d.md",
        &[
            ("type", s("decision")),
            ("id", s("d1")),
            ("project", s("studio-a")),
        ],
    )]);
    let filter = ListFilter {
        projects: vec!["zzzz".into()],
        ..ListFilter::default()
    };
    assert!(list(&g, "decision", &filter).is_empty());
}

#[test]
fn global_decisions_have_no_project() {
    let g = graph_of(vec![doc(
        "d.md",
        &[("type", s("decision")), ("id", s("d1"))],
    )]);
    let shown = list(&g, "decision", &ListFilter::default());
    assert_eq!(shown.len(), 1);
    assert!(shown[0].project.is_none());
}

#[test]
fn star_wildcard_means_every_project() {
    let g = graph_of(vec![
        doc(
            "d1.md",
            &[
                ("type", s("decision")),
                ("id", s("d1")),
                ("project", s("a")),
            ],
        ),
        doc("d2.md", &[("type", s("decision")), ("id", s("d2"))]),
    ]);
    let filter = ListFilter {
        projects: vec!["*".into()],
        ..ListFilter::default()
    };
    assert_eq!(list(&g, "decision", &filter).len(), 2);
}

#[test]
fn conflicting_supersession_is_detected() {
    let g = graph_of(vec![
        doc(
            "d1.md",
            &[
                ("type", s("decision")),
                ("id", s("d1")),
                ("status", s("active")),
                ("supersedes", Value::Array(vec![s("old")])),
            ],
        ),
        doc(
            "d2.md",
            &[
                ("type", s("decision")),
                ("id", s("d2")),
                ("status", s("active")),
                ("supersedes", Value::Array(vec![s("old")])),
            ],
        ),
    ]);
    let (conflicts, map) = supersession_map(&g);
    assert_eq!(conflicts.len(), 1);
    assert_eq!(map["old"].len(), 2);
}

#[test]
fn supersedes_pointing_nowhere_does_not_panic() {
    let g = graph_of(vec![doc(
        "d1.md",
        &[
            ("type", s("decision")),
            ("id", s("d1")),
            ("status", s("active")),
            ("supersedes", Value::Array(vec![s("ghost")])),
        ],
    )]);
    let (conflicts, map) = supersession_map(&g);
    assert!(conflicts.is_empty());
    assert_eq!(map["ghost"], vec!["d1".to_string()]);
}

#[test]
fn limit_truncates_results() {
    let docs = (0..5)
        .map(|i| {
            doc(
                &format!("d{i}.md"),
                &[("type", s("decision")), ("id", s(&format!("d{i}")))],
            )
        })
        .collect();
    let g = graph_of(docs);
    let filter = ListFilter {
        limit: 2,
        ..ListFilter::default()
    };
    assert_eq!(list(&g, "decision", &filter).len(), 2);
}

#[test]
fn contexts_default_hides_archived() {
    let g = graph_of(vec![
        doc(
            "c1.md",
            &[
                ("type", s("context")),
                ("id", s("c1")),
                ("status", s("active")),
            ],
        ),
        doc(
            "c2.md",
            &[
                ("type", s("context")),
                ("id", s("c2")),
                ("status", s("archived")),
            ],
        ),
    ]);
    assert_eq!(list(&g, "context", &ListFilter::default()).len(), 1);
}

#[test]
fn projects_default_hides_completed_and_archived() {
    let g = graph_of(vec![
        doc(
            "p1.md",
            &[
                ("type", s("project")),
                ("id", s("p1")),
                ("status", s("active")),
            ],
        ),
        doc(
            "p2.md",
            &[
                ("type", s("project")),
                ("id", s("p2")),
                ("status", s("paused")),
            ],
        ),
        doc(
            "p3.md",
            &[
                ("type", s("project")),
                ("id", s("p3")),
                ("status", s("completed")),
            ],
        ),
    ]);
    assert_eq!(list(&g, "project", &ListFilter::default()).len(), 2);
}

#[test]
fn missing_status_defaults_to_active() {
    let g = graph_of(vec![doc(
        "p1.md",
        &[("type", s("project")), ("id", s("p1"))],
    )]);
    assert_eq!(g.notes[0].effective_status(), "active");
}
