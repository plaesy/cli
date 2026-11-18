//! Unit tests for `tests` split out of `pptx.rs`.
//! Kept as a separate file so they are easy to find; included via `#[path]`.

use super::*;

#[test]
fn slide_count_reads_direct_children_of_slide_id_list() {
    let xml = br#"<p:presentation><p:sldIdLst><p:sldId id="256"/><p:sldId id="257"/></p:sldIdLst></p:presentation>"#;
    assert_eq!(slide_count(xml).unwrap(), 2);
    assert_eq!(slide_count(b"<p:presentation/>").unwrap(), 0);
}

#[test]
fn invalid_presentation_xml_is_an_error() {
    assert!(slide_count(b"not xml").is_err());
}
