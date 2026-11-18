//! Unit tests for `tests` split out of `types.rs`.
//! Kept as a separate file so they are easy to find; included via `#[path]`.

use super::*;

#[test]
fn platform_names_round_trip() {
    for p in Platform::ALL {
        assert_eq!(p.as_str().parse::<Platform>(), Ok(p));
    }
    assert!("nope".parse::<Platform>().is_err());
}

#[test]
fn auth_lists_match_the_go_tables() {
    assert!(Platform::Twitter.requires_auth());
    assert!(!Platform::Web.requires_auth());
    assert!(Platform::ZERO_CONFIG.iter().all(|p| !p.requires_auth()));
}
