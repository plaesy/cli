//! Unit tests for `filter_param_tests` split out of `mod.rs`.
//! Kept as a separate file so they are easy to find; included via `#[path]`.

use super::*;

#[test]
fn parse_filter_accepts_valid_and_rejects_invalid() {
    let f = parse_filter(&json!({
        "tier": ["knowledge", "procedural"], "path_prefix": "20-semantic/",
        "min_trust": "high", "include_inactive": true
    }))
    .expect("valid");
    assert_eq!(f.tiers, vec!["knowledge", "procedural"]);
    assert_eq!(f.path_prefix.as_deref(), Some("20-semantic/"));
    assert_eq!(f.min_trust, Some(Trust::High));
    assert!(f.include_inactive);

    assert!(parse_filter(&json!({})).expect("empty").tiers.is_empty());
    assert!(parse_filter(&json!({"tier": "bogus"})).is_err());
    assert!(parse_filter(&json!({"tier": 3})).is_err());
    assert!(parse_filter(&json!({"min_trust": "very"})).is_err());
    let asof = parse_filter(&json!({"as_of": "2026-06-15"})).expect("as_of");
    assert!(asof.as_of.is_some());
    assert!(parse_filter(&json!({"as_of": "not a date"})).is_err());
    assert!(parse_filter(&json!({"as_of": 20260615})).is_err());
    assert_eq!(
        parse_filter(&json!({"min_trust": 0.9}))
            .expect("num")
            .min_trust,
        Some(Trust::High)
    );
}
