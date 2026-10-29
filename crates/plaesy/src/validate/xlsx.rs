//! Structural check of a `.xlsx` workbook.
//!
//! Confirms the archive opens, `xl/workbook.xml` lists at least one sheet, every
//! expected sheet name appears, and each sheet's worksheet part exists and is
//! well-formed.

use std::collections::HashMap;
use std::path::Path;

use super::ooxml::{elements_named, read_checked, Package};
use super::ValidateError;

/// Findings for an `.xlsx`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct XlsxReport {
    pub sheets: Vec<String>,
}

struct Sheet {
    name: String,
    relationship_id: String,
}

/// Validates the `.xlsx` at `path`. Every name in `expected_sheets` must exist.
///
/// # Errors
///
/// [`ValidateError::Invalid`] for structural failures or missing sheets.
pub fn validate(path: &Path, expected_sheets: &[String]) -> Result<XlsxReport, ValidateError> {
    let mut pkg = Package::open(path)?;
    let workbook = pkg
        .read_part("xl/workbook.xml")
        .map_err(|e| ValidateError::invalid(format!("not a valid xlsx: {e}")))?;
    let sheets = workbook_sheets(&workbook).map_err(|e| {
        ValidateError::invalid(format!("xl/workbook.xml is not well-formed XML: {e}"))
    })?;
    if sheets.is_empty() {
        return Err(ValidateError::invalid("workbook has no sheets"));
    }
    let names: Vec<String> = sheets.iter().map(|s| s.name.clone()).collect();
    for want in expected_sheets {
        if !names.contains(want) {
            return Err(ValidateError::invalid(format!(
                "expected sheet '{want}' not found in {names:?}"
            )));
        }
    }

    let relationships = relationship_targets(&mut pkg);
    for (index, sheet) in sheets.iter().enumerate() {
        let target = relationships
            .get(&sheet.relationship_id)
            .cloned()
            .unwrap_or_else(|| format!("worksheets/sheet{}.xml", index + 1));
        let part = format!("xl/{}", normalize_target(&target));
        if !pkg.has_part(&part) {
            return Err(ValidateError::invalid(format!(
                "worksheet part {part} for sheet {:?} is missing from the archive",
                sheet.name
            )));
        }
        read_checked(&mut pkg, &part)?;
    }
    Ok(XlsxReport { sheets: names })
}

fn workbook_sheets(data: &[u8]) -> Result<Vec<Sheet>, String> {
    Ok(elements_named(data, "sheet")?
        .into_iter()
        .map(|attrs| Sheet {
            name: attrs.get("name").cloned().unwrap_or_default(),
            relationship_id: attrs.get("id").cloned().unwrap_or_default(),
        })
        .collect())
}

/// Maps relationship id to target from `xl/_rels/workbook.xml.rels`. A missing or
/// unreadable file yields an empty map, and sheets then use the conventional names.
fn relationship_targets(pkg: &mut Package) -> HashMap<String, String> {
    let Ok(data) = pkg.read_part("xl/_rels/workbook.xml.rels") else {
        return HashMap::new();
    };
    elements_named(&data, "Relationship")
        .unwrap_or_default()
        .into_iter()
        .filter_map(|attrs| Some((attrs.get("Id")?.clone(), attrs.get("Target")?.clone())))
        .collect()
}

/// Strips the `/xl/`, `xl/`, or `./` prefixes some producers write into relationship targets.
fn normalize_target(target: &str) -> &str {
    ["/xl/", "xl/", "./"]
        .iter()
        .find_map(|prefix| target.strip_prefix(prefix).filter(|rest| !rest.is_empty()))
        .unwrap_or(target)
}

#[cfg(test)]
mod tests {
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
}
