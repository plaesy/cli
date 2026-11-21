//! Unit tests for `tests` split out of `ooxml.rs`.
//! Kept as a separate file so they are easy to find; included via `#[path]`.

use super::*;

#[test]
fn well_formed_requires_a_root_element() {
    assert!(well_formed(b"<a><b/></a>").is_ok());
    assert!(well_formed(b"").is_err());
    assert!(well_formed(b"not xml at all").is_err());
    assert!(well_formed(b"<a><b></a>").is_err());
    assert!(well_formed(b"<a>").is_err());
}

#[test]
fn title_placeholder_is_detected_by_type() {
    assert!(has_title_placeholder(br#"<sp><ph type="title"/></sp>"#));
    assert!(has_title_placeholder(br#"<sp><ph type="ctrTitle"/></sp>"#));
    assert!(!has_title_placeholder(br#"<sp><ph type="body"/></sp>"#));
}
