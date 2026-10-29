//! Maps a project to the instruction files that apply to it.
//!
//! Reads `skills/plaesy/assets/mapping.json`. The `always_load` list is always
//! included. The other categories match by keyword in manifests and docs, by
//! file extension, and by file name.
//! Port of `scripts/internal/detectstack` (Go).

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use regex::Regex;
use serde::Deserialize;
use thiserror::Error;

/// Manifests scanned for keywords, at the root and nested up to [`MAX_DEPTH`].
const MANIFEST_FILES: [&str; 8] = [
    "package.json",
    "go.mod",
    "pom.xml",
    "build.gradle",
    "Cargo.toml",
    "pubspec.yaml",
    "Gemfile",
    "requirements.txt",
];

/// Extra project documents whose content also feeds the keyword scan.
const DOC_FILES: [&str; 2] = ["docs/project.json", ".plaesy/analysis/project.json"];

/// Path segments below the target that the walk enters.
const MAX_DEPTH: usize = 3;

/// Errors from detection.
#[derive(Debug, Error)]
pub enum DetectError {
    /// `mapping.json` could not be parsed.
    #[error("parse {}: {source}", path.display())]
    Parse {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },
}

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
struct Entry {
    file: String,
    keywords: Vec<String>,
    extensions: Vec<String>,
    filenames: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct Document {
    mappings: BTreeMap<String, serde_json::Value>,
}

/// Parsed `mapping.json`: the always-load list and the detection categories.
#[derive(Debug, Default)]
struct Mapping {
    always_load: Vec<String>,
    categories: Vec<BTreeMap<String, Entry>>,
}

fn load_mapping(path: &Path) -> Result<Option<Mapping>, DetectError> {
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Ok(None),
    };
    let doc: Document = serde_json::from_str(&text).map_err(|source| DetectError::Parse {
        path: path.to_path_buf(),
        source,
    })?;

    let mut mapping = Mapping::default();
    for (name, value) in doc.mappings {
        if name == "always_load" {
            mapping.always_load =
                serde_json::from_value(value).map_err(|source| DetectError::Parse {
                    path: path.to_path_buf(),
                    source,
                })?;
            continue;
        }
        // Metadata such as `scope_load` is a plain array, not a category. Skip it.
        if !value.is_object() {
            continue;
        }
        let category: BTreeMap<String, Entry> =
            serde_json::from_value(value).map_err(|source| DetectError::Parse {
                path: path.to_path_buf(),
                source,
            })?;
        mapping.categories.push(category);
    }
    Ok(Some(mapping))
}

/// Keyword matcher with ASCII word boundaries, so `java` does not match inside
/// `javascript`. A boundary is added only where the keyword starts or ends with
/// a word character, so `c#` still matches.
fn keyword_regex(keyword: &str) -> Regex {
    let word = |c: char| c == '_' || c.is_ascii_alphanumeric();
    let prefix = if keyword.chars().next().is_some_and(word) {
        r"(?-u:\b)"
    } else {
        ""
    };
    let suffix = if keyword.chars().last().is_some_and(word) {
        r"(?-u:\b)"
    } else {
        ""
    };
    Regex::new(&format!("(?i){prefix}{}{suffix}", regex::escape(keyword))).expect("escaped keyword")
}

fn react_marker() -> &'static str {
    " tsx jsx react"
}

/// Returns the instruction files that apply to `target_dir`, sorted and unique.
/// `plaesy_root` is the Plaesy home that holds `skills/plaesy/assets/mapping.json`.
///
/// A missing mapping file yields an empty list. Any other problem is an error.
///
/// # Errors
///
/// [`DetectError::Parse`] when `mapping.json` exists but is not valid.
pub fn detect(target_dir: &Path, plaesy_root: &Path) -> Result<Vec<String>, DetectError> {
    let mapping_path = plaesy_root
        .join("skills")
        .join("plaesy")
        .join("assets")
        .join("mapping.json");
    let Some(mapping) = load_mapping(&mapping_path)? else {
        return Ok(Vec::new());
    };

    let mut result: BTreeSet<String> = mapping.always_load.iter().cloned().collect();

    let mut by_extension: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut by_filename: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for entry in mapping.categories.iter().flat_map(BTreeMap::values) {
        for ext in &entry.extensions {
            by_extension
                .entry(ext.to_lowercase())
                .or_default()
                .push(entry.file.clone());
        }
        for name in &entry.filenames {
            by_filename
                .entry(name.to_lowercase())
                .or_default()
                .push(entry.file.clone());
        }
    }

    let mut scan = String::new();
    let mut direct: BTreeSet<String> = BTreeSet::new();
    let mut react = false;

    for name in MANIFEST_FILES {
        append_file(&mut scan, &target_dir.join(name));
    }

    if target_dir.is_dir() {
        walk(target_dir, 0, &mut |path, is_file| {
            if !is_file {
                return;
            }
            let base = path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            let lower = base.to_lowercase();
            if path.parent() != Some(target_dir) && MANIFEST_FILES.contains(&base.as_str()) {
                append_file(&mut scan, path);
            }
            if lower.ends_with(".csproj") {
                append_file(&mut scan, path);
            }
            if lower.ends_with(".tsx") || lower.ends_with(".jsx") {
                react = true;
            }
            for (ext, files) in &by_extension {
                if lower.ends_with(ext.as_str()) {
                    direct.extend(files.iter().cloned());
                }
            }
            if let Some(files) = by_filename.get(&lower) {
                direct.extend(files.iter().cloned());
            }
        });
    }

    for doc in DOC_FILES {
        append_file(&mut scan, &target_dir.join(doc));
    }
    if react {
        scan.push_str(react_marker());
    }
    let scan_lower = scan.to_lowercase();

    for entry in mapping.categories.iter().flat_map(BTreeMap::values) {
        if entry.file.is_empty() {
            continue;
        }
        let matched = entry
            .keywords
            .iter()
            .map(|k| k.trim())
            .filter(|k| !k.is_empty())
            .any(|k| keyword_regex(k).is_match(&scan_lower));
        if matched {
            result.insert(entry.file.clone());
        }
    }
    result.extend(direct);
    Ok(result.into_iter().collect())
}

fn append_file(scan: &mut String, path: &Path) {
    if let Ok(text) = fs::read_to_string(path) {
        scan.push_str(&text);
        scan.push(' ');
    }
}

/// Depth-limited walk that prunes `.git` and `node_modules`.
fn walk(dir: &Path, depth: usize, visit: &mut impl FnMut(&Path, bool)) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    let mut paths: Vec<PathBuf> = entries.flatten().map(|e| e.path()).collect();
    paths.sort();
    for path in paths {
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let child_depth = depth + 1;
        if path.is_dir() {
            if child_depth > MAX_DEPTH || name == ".git" || name == "node_modules" {
                continue;
            }
            walk(&path, child_depth, visit);
        } else if child_depth <= MAX_DEPTH {
            visit(&path, true);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mapping_fixture(root: &Path) {
        fs::create_dir_all(root.join("skills/plaesy/assets")).unwrap();
        fs::write(
            root.join("skills/plaesy/assets/mapping.json"),
            r#"{"mappings":{
                "always_load":["plaesy.md"],
                "scope_load":["assess-design.md"],
                "languages":{
                    "java":{"file":"java.md","keywords":["java"],"extensions":[".java"],"filenames":[]},
                    "csharp":{"file":"csharp.md","keywords":["c#"],"extensions":[".cs"],"filenames":[]},
                    "react":{"file":"react.md","keywords":["react"],"extensions":[],"filenames":[]}
                }
            }}"#,
        )
        .unwrap();
    }

    #[test]
    fn always_load_is_included_and_metadata_arrays_are_skipped() {
        let root = tempfile::tempdir().unwrap();
        mapping_fixture(root.path());
        let project = tempfile::tempdir().unwrap();
        let found = detect(project.path(), root.path()).unwrap();
        assert_eq!(found, ["plaesy.md"]);
    }

    #[test]
    fn keywords_match_on_word_boundaries_and_extensions_match_files() {
        let root = tempfile::tempdir().unwrap();
        mapping_fixture(root.path());
        let project = tempfile::tempdir().unwrap();
        fs::write(
            project.path().join("package.json"),
            r#"{"deps":["javascript"]}"#,
        )
        .unwrap();
        fs::write(project.path().join("Program.cs"), "class A {}").unwrap();
        let found = detect(project.path(), root.path()).unwrap();
        assert!(found.contains(&"csharp.md".to_owned()), "{found:?}");
        assert!(
            !found.contains(&"java.md".to_owned()),
            "java must not match javascript: {found:?}"
        );
    }

    #[test]
    fn tsx_files_add_the_react_keywords() {
        let root = tempfile::tempdir().unwrap();
        mapping_fixture(root.path());
        let project = tempfile::tempdir().unwrap();
        fs::write(project.path().join("App.tsx"), "export {}").unwrap();
        assert!(detect(project.path(), root.path())
            .unwrap()
            .contains(&"react.md".to_owned()));
    }

    #[test]
    fn missing_mapping_is_not_an_error() {
        let root = tempfile::tempdir().unwrap();
        let project = tempfile::tempdir().unwrap();
        assert!(detect(project.path(), root.path()).unwrap().is_empty());
    }

    #[test]
    fn corrupt_mapping_is_an_error() {
        let root = tempfile::tempdir().unwrap();
        fs::create_dir_all(root.path().join("skills/plaesy/assets")).unwrap();
        fs::write(
            root.path().join("skills/plaesy/assets/mapping.json"),
            "{ nope",
        )
        .unwrap();
        let project = tempfile::tempdir().unwrap();
        assert!(matches!(
            detect(project.path(), root.path()),
            Err(DetectError::Parse { .. })
        ));
    }
}
