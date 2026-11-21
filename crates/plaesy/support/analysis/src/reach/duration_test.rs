//! Unit tests for `tests` split out of `duration.rs`.
//! Kept as a separate file so they are easy to find; included via `#[path]`.

use super::*;

#[test]
fn parses_the_forms_the_config_uses() {
    assert_eq!(parse("24h"), Some(Duration::from_secs(86_400)));
    assert_eq!(parse("500ms"), Some(Duration::from_millis(500)));
    assert_eq!(parse("1h30m"), Some(Duration::from_secs(5_400)));
    assert_eq!(parse("1.5s"), Some(Duration::from_millis(1_500)));
    assert_eq!(parse("-1s"), None);
    assert_eq!(parse("soon"), None);
}

#[test]
fn formats_like_go() {
    assert_eq!(format(Duration::ZERO), "0s");
    assert_eq!(format(Duration::from_millis(500)), "500ms");
    assert_eq!(format(Duration::from_secs(300)), "5m0s");
    assert_eq!(format(Duration::from_secs(86_400)), "24h0m0s");
    assert_eq!(format(Duration::from_millis(1_500)), "1.5s");
}
