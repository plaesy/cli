//! Shared OOXML container and XML helpers for the docx, pptx, and xlsx validators.
//!
//! An OOXML file is a ZIP archive of XML parts. The validators confirm the
//! archive opens, the required parts exist, and each part is well-formed XML
//! with a root element.

use std::collections::HashMap;
use std::fs::File;
use std::io::Read;
use std::path::Path;

use quick_xml::events::{BytesStart, Event};
use quick_xml::Reader;
use zip::ZipArchive;

use super::ValidateError;

/// An opened OOXML archive.
pub struct Package {
    archive: ZipArchive<File>,
}

impl Package {
    /// Opens `path` as a ZIP archive.
    ///
    /// # Errors
    ///
    /// Missing file, directory, or a file that is not a ZIP archive.
    pub fn open(path: &Path) -> Result<Self, ValidateError> {
        if !path.is_file() {
            return Err(ValidateError::invalid(format!(
                "file not found: {}",
                path.display()
            )));
        }
        let file = File::open(path)
            .map_err(|e| ValidateError::io(format!("opening {}", path.display()), e))?;
        let archive = ZipArchive::new(file).map_err(|e| {
            ValidateError::invalid(format!("not a well-formed OOXML (zip) file: {e}"))
        })?;
        let mut pkg = Self { archive };
        if !pkg.has_part("[Content_Types].xml") {
            return Err(ValidateError::invalid(
                "not a well-formed OOXML file: missing [Content_Types].xml",
            ));
        }
        Ok(pkg)
    }

    /// Reports whether the archive contains `name`.
    #[must_use]
    pub fn has_part(&mut self, name: &str) -> bool {
        self.archive.by_name(name).is_ok()
    }

    /// Names of all parts in the archive.
    #[must_use]
    pub fn part_names(&self) -> Vec<String> {
        self.archive.file_names().map(str::to_owned).collect()
    }

    /// Reads a required part.
    ///
    /// # Errors
    ///
    /// [`ValidateError::Invalid`] when the part is missing or unreadable.
    pub fn read_part(&mut self, name: &str) -> Result<Vec<u8>, ValidateError> {
        let mut file = self
            .archive
            .by_name(name)
            .map_err(|_| ValidateError::invalid(format!("missing required part: {name}")))?;
        let mut data = Vec::new();
        file.read_to_end(&mut data)
            .map_err(|e| ValidateError::invalid(format!("cannot read part {name}: {e}")))?;
        Ok(data)
    }
}

/// Checks that `data` is well-formed XML with at least one root element.
///
/// # Errors
///
/// A parse error, or input with no element at all. Plain text is not XML.
pub fn well_formed(data: &[u8]) -> Result<(), String> {
    let mut reader = Reader::from_reader(data);
    let mut buf = Vec::new();
    let mut depth: usize = 0;
    let mut saw_root = false;
    loop {
        match reader
            .read_event_into(&mut buf)
            .map_err(|e| e.to_string())?
        {
            Event::Start(_) => {
                if depth == 0 {
                    saw_root = true;
                }
                depth += 1;
            }
            Event::Empty(_) => {
                if depth == 0 {
                    saw_root = true;
                }
            }
            Event::End(_) => {
                depth = depth
                    .checked_sub(1)
                    .ok_or_else(|| "unexpected end tag".to_owned())?;
            }
            Event::Eof => break,
            _ => {}
        }
        buf.clear();
    }
    if depth != 0 {
        return Err("unexpected end of document".into());
    }
    if !saw_root {
        return Err("no XML element found".into());
    }
    Ok(())
}

/// Returns every element with the given local name, with its attributes.
pub(crate) fn elements_named(
    data: &[u8],
    local: &str,
) -> Result<Vec<HashMap<String, String>>, String> {
    let mut reader = Reader::from_reader(data);
    let mut buf = Vec::new();
    let mut out = Vec::new();
    loop {
        match reader
            .read_event_into(&mut buf)
            .map_err(|e| e.to_string())?
        {
            Event::Start(e) | Event::Empty(e) if e.local_name().as_ref() == local.as_bytes() => {
                out.push(attributes(&e)?);
            }
            Event::Eof => break,
            _ => {}
        }
        buf.clear();
    }
    Ok(out)
}

/// Attributes of an element, keyed by local name, with values unescaped.
fn attributes(e: &BytesStart<'_>) -> Result<HashMap<String, String>, String> {
    let mut map = HashMap::new();
    for attr in e.attributes() {
        let attr = attr.map_err(|err| err.to_string())?;
        let key = String::from_utf8_lossy(attr.key.local_name().as_ref()).into_owned();
        let value = attr
            .unescape_value()
            .map_err(|err| err.to_string())?
            .into_owned();
        map.insert(key, value);
    }
    Ok(map)
}

/// Reports whether any element of `data` is a `ph` placeholder whose `type` is
/// `title` or `ctrTitle`.
pub(crate) fn has_title_placeholder(data: &[u8]) -> bool {
    elements_named(data, "ph")
        .map(|phs| {
            phs.iter().any(|attrs| {
                matches!(
                    attrs.get("type").map(String::as_str),
                    Some("title" | "ctrTitle")
                )
            })
        })
        .unwrap_or(false)
}

/// Reads a part and checks that it is well-formed XML.
pub(crate) fn read_checked(pkg: &mut Package, name: &str) -> Result<Vec<u8>, ValidateError> {
    let data = pkg.read_part(name)?;
    well_formed(&data)
        .map_err(|e| ValidateError::invalid(format!("{name} is not well-formed XML: {e}")))?;
    Ok(data)
}

#[cfg(test)]
mod tests {
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
}
