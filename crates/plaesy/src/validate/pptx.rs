//! Structural check of a `.pptx` presentation.
//!
//! Confirms the archive opens, `ppt/presentation.xml` is well-formed, the slide
//! count matches an expectation when one is given, and every slide part is
//! well-formed. Slides without a title placeholder are reported but do not fail.

use std::path::Path;

use quick_xml::events::Event;
use quick_xml::Reader;

use super::ooxml::{has_title_placeholder, read_checked, well_formed, Package};
use super::ValidateError;

/// Findings for a `.pptx`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PptxReport {
    pub slide_count: usize,
    /// One-based numbers of slides with no title placeholder.
    pub slides_without_title: Vec<u32>,
}

/// Validates the `.pptx` at `path`. A non-zero `expected_slides` must match the count.
///
/// # Errors
///
/// [`ValidateError::Invalid`] for structural failures or a slide-count mismatch.
pub fn validate(path: &Path, expected_slides: usize) -> Result<PptxReport, ValidateError> {
    let mut pkg = Package::open(path)?;
    let presentation = pkg
        .read_part("ppt/presentation.xml")
        .map_err(|e| ValidateError::invalid(format!("not a valid pptx: {e}")))?;
    let slide_count = slide_count(&presentation).map_err(|e| {
        ValidateError::invalid(format!("ppt/presentation.xml is not well-formed XML: {e}"))
    })?;

    let mut slides: Vec<(u32, String)> = pkg
        .part_names()
        .into_iter()
        .filter_map(|name| {
            let number = name
                .strip_prefix("ppt/slides/slide")?
                .strip_suffix(".xml")?
                .parse::<u32>()
                .ok()?;
            Some((number, name))
        })
        .collect();
    slides.sort_unstable();

    let mut report = PptxReport {
        slide_count,
        slides_without_title: Vec::new(),
    };
    for (number, name) in &slides {
        let data = read_checked(&mut pkg, name)?;
        if !has_title_placeholder(&data) {
            report.slides_without_title.push(*number);
        }
    }

    if expected_slides > 0 && report.slide_count != expected_slides {
        return Err(ValidateError::invalid(format!(
            "expected {expected_slides} slides, got {}",
            report.slide_count
        )));
    }
    Ok(report)
}

/// Counts `sldId` elements that are direct children of `sldIdLst`.
fn slide_count(data: &[u8]) -> Result<usize, String> {
    well_formed(data)?;
    let mut reader = Reader::from_reader(data);
    let mut buf = Vec::new();
    let mut depth = 0usize;
    let mut list_depth: Option<usize> = None;
    let mut count = 0;
    loop {
        match reader
            .read_event_into(&mut buf)
            .map_err(|e| e.to_string())?
        {
            Event::Start(e) => {
                depth += 1;
                if e.local_name().as_ref() == b"sldIdLst" {
                    list_depth = Some(depth);
                }
            }
            Event::Empty(e) => {
                if e.local_name().as_ref() == b"sldId" && list_depth == Some(depth) {
                    count += 1;
                }
            }
            Event::End(_) => {
                if list_depth == Some(depth) {
                    list_depth = None;
                }
                depth -= 1;
            }
            Event::Eof => break,
            _ => {}
        }
        buf.clear();
    }
    Ok(count)
}

#[cfg(test)]
mod tests {
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
}
