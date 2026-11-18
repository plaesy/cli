//! Unit tests for `tests` split out of `xlsx.rs`.
//! Kept as a separate file so they are easy to find; included via `#[path]`.

use super::*;

#[test]
fn workbook_lists_sheets_with_relationship_ids() {
    let xml = br#"<workbook><sheets><sheet name="Data" sheetId="1" id="rId1"/><sheet name="Notes" sheetId="2" id="rId2"/></sheets></workbook>"#;
    let sheets = workbook_sheets(xml).unwrap();
    assert_eq!(sheets.len(), 2);
    assert_eq!(sheets[1].name, "Notes");
    assert_eq!(sheets[1].relationship_id, "rId2");
}

#[test]
fn relationship_prefixes_are_normalized() {
    assert_eq!(
        normalize_target("/xl/worksheets/sheet1.xml"),
        "worksheets/sheet1.xml"
    );
    assert_eq!(
        normalize_target("./worksheets/sheet1.xml"),
        "worksheets/sheet1.xml"
    );
    assert_eq!(
        normalize_target("worksheets/sheet1.xml"),
        "worksheets/sheet1.xml"
    );
}
