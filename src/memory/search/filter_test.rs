//! Unit tests for `tests` split out of `filter.rs`.
//! Kept as a separate file so they are easy to find; included via `#[path]`.

use super::*;
use serde_json::json;

fn now() -> DateTime<Utc> {
    parse_date("2026-10-19").expect("date")
}

fn md(v: Value) -> HashMap<String, Value> {
    serde_json::from_value(v).expect("map")
}

#[test]
fn parse_date_rejects_trailing_garbage_after_calendar_date() {
    assert!(parse_date("2026-11-12").is_some());
    assert!(parse_date("2026-11-12garbage").is_none());
    assert!(parse_date("2026-11-12T00:00:00Zjunk").is_none());
}

#[test]
fn tier_aliases_and_path_inference() {
    assert_eq!(normalize_tier(" Knowledge "), Some("semantic"));
    assert_eq!(normalize_tier("daily"), Some("episodic"));
    assert_eq!(normalize_tier("nonsense"), None);
    assert_eq!(infer_tier("10-episodic/2026/10/a.md"), Some("episodic"));
    assert_eq!(infer_tier("20_semantic/people/x.md"), Some("semantic"));
    assert_eq!(infer_tier("00-core/persona.md"), Some("core"));
    // The file name itself never decides the tier.
    assert_eq!(infer_tier("notes/archive.md"), None);
}

#[test]
fn frontmatter_beats_path_and_numbers_map_to_trust() {
    let m = build_meta(
        "10-episodic/a.md",
        &md(json!({"tier": "procedural", "confidence": 0.9})),
        now(),
        180,
    );
    assert_eq!(m.tier.as_deref(), Some("procedural"));
    assert_eq!(m.trust.as_deref(), Some("high"));
    let low = build_meta("a.md", &md(json!({"confidence": "0.2"})), now(), 180);
    assert_eq!(low.trust.as_deref(), Some("low"));
    for invalid in [json!(-0.1), json!(1.1), json!("-0.1"), json!("1.1")] {
        assert_eq!(Trust::parse(&invalid), None, "invalid trust: {invalid}");
    }
    assert_eq!(Trust::parse(&json!(0.0)), Some(Trust::Low));
    assert_eq!(Trust::parse(&json!(1.0)), Some(Trust::High));
    assert_eq!(Trust::parse(&json!("0.5")), Some(Trust::Medium));
}

#[test]
fn staleness_rules() {
    let old = json!({"tier": "semantic", "last_verified": "2026-01-01"});
    assert!(build_meta("a.md", &md(old.clone()), now(), 180).stale);
    assert!(!build_meta("a.md", &md(old), now(), 0).stale, "0 disables");
    let fresh = json!({"tier": "semantic", "last_verified": "2026-10-01"});
    assert!(!build_meta("a.md", &md(fresh), now(), 180).stale);
    let log = json!({"tier": "episodic", "last_verified": "2020-01-01"});
    assert!(
        !build_meta("a.md", &md(log), now(), 180).stale,
        "episodic never stale"
    );
    let unverified = json!({"tier": "semantic"});
    assert!(!build_meta("a.md", &md(unverified), now(), 180).stale);
    assert_eq!(age_days("2026-10-09T10:00:00Z", now()), Some(9));
    assert_eq!(age_days("not a date", now()), None);
}

fn res(path: &str, meta: ResultMeta) -> SearchResult {
    SearchResult {
        chunk_id: 1,
        text: "t".into(),
        source_path: path.into(),
        start_line: 1,
        end_line: 1,
        revision: None,
        indexed_at: "2026-10-19T00:00:00Z".into(),
        score: 0.5,
        confidence: crate::memory::storage::ConfidenceLevel::Fact,
        meta,
        explain: None,
    }
}

#[test]
fn project_filter_matches_owner_and_shared_scope_case_insensitively() {
    let md = md(json!({"project": "Studio-A", "scope": ["Studio-B", "Studio-C"]}));
    let metadata = build_meta("a.md", &md, now(), 180);
    assert_eq!(metadata.project.as_deref(), Some("Studio-A"));
    assert_eq!(metadata.scope, vec!["Studio-B", "Studio-C"]);

    for project in ["studio-a", "STUDIO-B"] {
        let filter = SearchFilter {
            project: Some(project.into()),
            ..SearchFilter::default()
        };
        assert!(filter.is_restrictive());
        assert!(filter.accepts(&res("a.md", metadata.clone())));
    }
    let other = SearchFilter {
        project: Some("studio-d".into()),
        ..SearchFilter::default()
    };
    assert!(!other.accepts(&res("a.md", metadata)));
    assert!(SearchFilter::default().accepts(&res("a.md", ResultMeta::default())));
}

#[test]
fn non_string_validity_metadata_is_rejected_by_as_of_filter() {
    let filter = SearchFilter {
        as_of: parse_date("2026-06-15"),
        ..SearchFilter::default()
    };
    for raw in [json!(123), Value::Null, json!(true)] {
        let metadata = md(json!({"valid_from": raw}));
        let result = res("a.md", build_meta("a.md", &metadata, now(), 180));
        assert!(
            !filter.accepts(&result),
            "malformed validity metadata must not be treated as absent: {metadata:?}"
        );
    }
}

#[test]
fn event_date_bounds_are_explicit_and_fail_closed() {
    let in_range = res(
        "a.md",
        build_meta("a.md", &md(json!({"event_date": "2026-06-15"})), now(), 180),
    );
    let filter = SearchFilter {
        event_from: parse_date("2026-06-15"),
        event_until: parse_date("2026-06-16"),
        ..SearchFilter::default()
    };
    assert!(filter.is_restrictive());
    assert!(filter.accepts(&in_range));
    assert!(!SearchFilter {
        event_from: parse_date("2026-06-16"),
        ..SearchFilter::default()
    }
    .accepts(&in_range));
    assert!(!SearchFilter {
        event_until: parse_date("2026-06-15"),
        ..SearchFilter::default()
    }
    .accepts(&in_range));
    assert!(!filter.accepts(&res("undated.md", ResultMeta::default())));
    let malformed = res(
        "bad.md",
        build_meta("bad.md", &md(json!({"event_date": true})), now(), 180),
    );
    assert!(!filter.accepts(&malformed));
}

#[test]
fn index_time_bounds_filter_inclusively_and_fail_closed() {
    let result = res("a.md", ResultMeta::default());
    let day = |value: &str| parse_date(value).expect("valid date");
    let same_day = SearchFilter {
        indexed_since: Some(day("2026-10-19")),
        indexed_until: Some(day("2026-10-20")),
        ..SearchFilter::default()
    };
    assert!(same_day.is_restrictive());
    assert!(same_day.accepts(&result));

    let before = SearchFilter {
        indexed_since: Some(day("2026-10-20")),
        ..SearchFilter::default()
    };
    assert!(!before.accepts(&result));
    let after = SearchFilter {
        indexed_until: Some(day("2026-10-19")),
        ..SearchFilter::default()
    };
    assert!(!after.accepts(&result));

    let mut malformed = result;
    malformed.indexed_at = "not-a-time".into();
    assert!(!same_day.accepts(&malformed));
}

#[test]
fn as_of_keeps_sources_valid_on_that_day() {
    let dated = |from: Option<&str>, to: Option<&str>| ResultMeta {
        valid_from: from.map(Into::into),
        valid_to: to.map(Into::into),
        ..ResultMeta::default()
    };
    let f = SearchFilter {
        as_of: parse_date("2026-06-15"),
        ..SearchFilter::default()
    };
    assert!(f.is_restrictive());
    assert!(f.accepts(&res("a.md", dated(Some("2026-01-01"), Some("2026-12-31")))));
    assert!(
        !f.accepts(&res("a.md", dated(Some("2026-07-01"), None))),
        "not yet valid"
    );
    assert!(
        !f.accepts(&res("a.md", dated(None, Some("2026-06-01")))),
        "already expired"
    );
    assert!(f.accepts(&res("a.md", dated(None, None))), "undated passes");
    assert!(
        !f.accepts(&res("a.md", dated(Some("not-a-date"), None))),
        "invalid validity start must not pass an as-of filter"
    );
    assert!(
        !f.accepts(&res("a.md", dated(None, Some("not-a-date")))),
        "invalid validity end must not pass an as-of filter"
    );
    // Date-only bounds include the whole calendar day, even at its final instant.
    let end_of_day = SearchFilter {
        as_of: parse_date("2026-06-15T23:59:59Z"),
        ..SearchFilter::default()
    };
    assert!(end_of_day.accepts(&res("a.md", dated(None, Some("2026-06-15")))));
    assert!(end_of_day.accepts(&res("a.md", dated(None, Some("2026-06-15T23:59:59Z")))));
    let after_timestamp_end = SearchFilter {
        as_of: parse_date("2026-06-15T23:59:59.000001Z"),
        ..SearchFilter::default()
    };
    assert!(!after_timestamp_end.accepts(&res("a.md", dated(None, Some("2026-06-15T23:59:59Z")))));

    let m = build_meta(
        "a.md",
        &md(json!({"valid_from": "2026-01-01", "valid_to": "2026-06-30"})),
        now(),
        180,
    );
    assert_eq!(m.valid_from.as_deref(), Some("2026-01-01"));
    assert_eq!(m.valid_to.as_deref(), Some("2026-06-30"));
}

#[test]
fn boolean_tag_expressions_parse_and_match_exact_declared_tags() {
    let tags = vec!["Rust".to_string(), "memory".to_string()];
    for (expression, expected) in [
        ("rust AND memory", true),
        ("rust AND NOT archived", true),
        ("(rust OR cli) AND memory", true),
        ("Rust OR cli AND absent", true),
        ("rust and missing", false),
        ("NOT archived", true),
        ("RUST", true),
        ("rusty", false),
    ] {
        let parsed = TagExpr::parse(expression).unwrap_or_else(|e| panic!("{expression}: {e}"));
        assert_eq!(parsed.matches(&tags), expected, "{expression}");
    }
    let filter = SearchFilter {
        tag_filter: Some(TagExpr::parse("rust AND memory").expect("valid expression")),
        ..SearchFilter::default()
    };
    assert!(filter.is_restrictive());
    assert!(filter.accepts(&res(
        "a.md",
        ResultMeta {
            tags,
            ..ResultMeta::default()
        }
    )));
}

#[test]
fn boolean_tag_expressions_reject_invalid_or_excessive_input() {
    for invalid in ["", "rust AND", "(rust OR cli", "rust xor cli", "rust/tag"] {
        assert!(TagExpr::parse(invalid).is_err(), "accepted {invalid:?}");
    }
    assert!(TagExpr::parse(&"a ".repeat(65)).is_err());
    assert!(TagExpr::parse(&format!("{}a{}", "(".repeat(33), ")".repeat(33))).is_err());
    assert!(TagExpr::parse(&"a ".repeat(600)).is_err());
}

#[test]
fn tag_filter_matching_is_case_insensitive() {
    let meta = ResultMeta {
        tags: vec!["Rust".into(), "Memory".into()],
        ..ResultMeta::default()
    };
    let any = SearchFilter {
        tags: vec!["rust".into()],
        ..SearchFilter::default()
    };
    let all = SearchFilter {
        tags: vec!["RUST".into(), "memory".into()],
        tags_all: true,
        ..SearchFilter::default()
    };
    assert!(any.accepts(&res("a.md", meta.clone())));
    assert!(all.accepts(&res("a.md", meta)));
}

#[test]
fn tag_filter_matches_any_by_default_and_all_when_requested() {
    let tagged = ResultMeta {
        tags: vec!["rust".into(), "memory".into()],
        ..ResultMeta::default()
    };
    let any = SearchFilter {
        tags: vec!["rust".into(), "cli".into()],
        ..SearchFilter::default()
    };
    assert!(any.accepts(&res("a.md", tagged.clone())));
    assert!(!any.accepts(&res("a.md", ResultMeta::default())));
    let all = SearchFilter {
        tags: vec!["rust".into(), "cli".into()],
        tags_all: true,
        ..SearchFilter::default()
    };
    assert!(!all.accepts(&res("a.md", tagged)));
}

#[test]
fn filter_semantics() {
    let sem = ResultMeta {
        tier: Some("semantic".into()),
        trust: Some("medium".into()),
        ..ResultMeta::default()
    };
    let old = ResultMeta {
        status: Some("superseded".into()),
        ..sem.clone()
    };
    let all = SearchFilter::default();
    assert!(all.accepts(&res("a.md", sem.clone())));
    assert!(
        !all.accepts(&res("a.md", old.clone())),
        "superseded hidden by default"
    );
    let uppercase_inactive = ResultMeta {
        status: Some("ARCHIVED".into()),
        ..sem.clone()
    };
    assert!(
        !all.accepts(&res("a.md", uppercase_inactive)),
        "inactive status matching is case-insensitive"
    );
    let inc = SearchFilter {
        include_inactive: true,
        ..SearchFilter::default()
    };
    assert!(inc.accepts(&res("a.md", old)));
    for status in ["deprecated", "archived", "obsolete"] {
        let inactive = ResultMeta {
            status: Some(status.into()),
            ..sem.clone()
        };
        assert!(
            !all.accepts(&res("a.md", inactive.clone())),
            "{status} should be hidden by default"
        );
        assert!(
            inc.accepts(&res("a.md", inactive)),
            "{status} should appear when include_inactive is true"
        );
    }

    let by_tier = SearchFilter {
        tiers: vec!["knowledge".into()], // alias of semantic
        ..SearchFilter::default()
    };
    assert!(by_tier.accepts(&res("a.md", sem.clone())));
    assert!(
        !by_tier.accepts(&res("a.md", ResultMeta::default())),
        "no tier never matches"
    );

    let trusted = SearchFilter {
        min_trust: Some(Trust::High),
        ..SearchFilter::default()
    };
    assert!(!trusted.accepts(&res("a.md", sem.clone())));
    assert!(
        !trusted.accepts(&res("a.md", ResultMeta::default())),
        "unknown trust excluded"
    );
    let ok = ResultMeta {
        trust: Some("high".into()),
        ..sem
    };
    assert!(trusted.accepts(&res("a.md", ok)));

    let scoped = SearchFilter {
        path_prefix: Some("20-semantic/".into()),
        ..SearchFilter::default()
    };
    assert!(scoped.accepts(&res("20-semantic\\x.md", ResultMeta::default())));
    assert!(scoped.accepts(&res("20-semantic/x.md", ResultMeta::default())));
    assert!(scoped.accepts(&res("20-semantic", ResultMeta::default())));
    assert!(!scoped.accepts(&res("20-semantics/x.md", ResultMeta::default())));
    assert!(!scoped.accepts(&res("20-semantic-old/x.md", ResultMeta::default())));
    assert!(!scoped.accepts(&res("10-episodic/x.md", ResultMeta::default())));
    let empty_scope = SearchFilter {
        path_prefix: Some(String::new()),
        ..SearchFilter::default()
    };
    assert!(empty_scope.accepts(&res("anywhere/note.md", ResultMeta::default())));
}
