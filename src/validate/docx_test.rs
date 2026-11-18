//! Unit tests for `tests` split out of `docx.rs`.
//! Kept as a separate file so they are easy to find; included via `#[path]`.

use super::*;

#[test]
fn counts_paragraphs_tables_and_flags_leftover_tags() {
    let xml = br#"<w:document><w:body>
            <w:p><w:r><w:t>Hello</w:t></w:r></w:p>
            <w:tbl><w:tr/></w:tbl>
            <w:p><w:r><w:t>Dear {{NAME}}</w:t></w:r></w:p>
        </w:body></w:document>"#;
    let report = parse_document(xml).unwrap();
    assert_eq!(report.paragraphs, 2);
    assert_eq!(report.tables, 1);
    assert_eq!(report.leftover, ["Dear {{NAME}}"]);
}
