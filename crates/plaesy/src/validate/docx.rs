//! Structural check of a `.docx` document.
//!
//! Confirms the archive opens, `word/document.xml` is well-formed, the document
//! has at least one paragraph or table, and no paragraph still holds a `{{ }}`
//! template tag.

use std::path::Path;

use quick_xml::events::Event;
use quick_xml::Reader;

use super::ooxml::Package;
use super::{jinja_tag_regex, ValidateError};

/// Findings for a `.docx`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DocxReport {
    pub paragraphs: usize,
    pub tables: usize,
    /// Paragraph texts that still contain a `{{ ... }}` tag.
    pub leftover: Vec<String>,
}

/// Validates the `.docx` at `path`.
///
/// # Errors
///
/// [`ValidateError::Invalid`] for any structural failure or leftover template tag.
pub fn validate(path: &Path) -> Result<DocxReport, ValidateError> {
    let mut pkg = Package::open(path)?;
    let data = pkg
        .read_part("word/document.xml")
        .map_err(|e| ValidateError::invalid(format!("not a valid docx: {e}")))?;
    let report = parse_document(&data).map_err(|e| {
        ValidateError::invalid(format!("word/document.xml is not well-formed XML: {e}"))
    })?;
    if report.paragraphs == 0 && report.tables == 0 {
        return Err(ValidateError::invalid(
            "document has no paragraphs or tables",
        ));
    }
    if !report.leftover.is_empty() {
        return Err(ValidateError::invalid(format!(
            "leftover unfilled template tags found: {:?}",
            report.leftover
        )));
    }
    Ok(report)
}

/// Counts paragraphs and tables, and collects paragraph text that has a template tag.
fn parse_document(data: &[u8]) -> Result<DocxReport, String> {
    let mut reader = Reader::from_reader(data);
    let mut buf = Vec::new();
    let mut report = DocxReport::default();
    let mut paragraphs: Vec<String> = Vec::new();
    let mut in_text = false;
    loop {
        match reader
            .read_event_into(&mut buf)
            .map_err(|e| e.to_string())?
        {
            Event::Start(e) => match e.local_name().as_ref() {
                b"p" => {
                    report.paragraphs += 1;
                    paragraphs.push(String::new());
                }
                b"tbl" => report.tables += 1,
                b"t" => in_text = true,
                _ => {}
            },
            Event::Text(t) if in_text => {
                if let Some(current) = paragraphs.last_mut() {
                    current.push_str(&t.unescape().map_err(|e| e.to_string())?);
                }
            }
            Event::End(e) => match e.local_name().as_ref() {
                b"t" => in_text = false,
                b"p" => {
                    if let Some(text) = paragraphs.pop() {
                        if jinja_tag_regex().is_match(&text) {
                            report.leftover.push(text);
                        }
                    }
                }
                _ => {}
            },
            Event::Eof => break,
            _ => {}
        }
        buf.clear();
    }
    Ok(report)
}

#[cfg(test)]
mod tests {
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
}
