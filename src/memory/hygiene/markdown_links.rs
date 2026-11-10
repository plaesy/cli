//! Filesystem-backed validation for local Markdown links.

use super::{finding, Finding};
use crate::mdlint::doc::Doc;
use crate::memory::storage::StoredDocument;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Component, Path, PathBuf};

pub(super) fn check(root: &Path, docs: &[StoredDocument]) -> Vec<Finding> {
    let Ok(root) = root.canonicalize() else {
        return Vec::new();
    };
    let mut headings = HashMap::new();
    for doc in docs {
        headings.insert(normalize_note_path(&doc.path), heading_ids(&doc.body));
    }

    let mut findings = Vec::new();
    for doc in docs {
        if is_template_path(&doc.path) {
            continue;
        }
        let markdown = Doc::parse(&doc.body);
        let (images, mut links) = markdown.ast_links();
        links.extend(images);
        for link in links {
            let destination = link.destination.trim();
            if destination.is_empty()
                || destination.contains("{{")
                || destination.contains("<topic>")
                || destination.contains("<related-note>")
            {
                continue;
            }
            if is_machine_specific(destination) {
                findings.push(finding(
                    "machine_specific_link",
                    &doc.path,
                    Some(link.line + doc.body_start_line - 1),
                    format!("local link depends on this machine: `{destination}`"),
                    "FACT",
                ));
                continue;
            }
            if has_uri_scheme(destination) || destination.starts_with("//") {
                continue;
            }

            let (without_fragment, fragment) = destination
                .split_once('#')
                .map_or((destination, None), |(path, fragment)| {
                    (path, Some(fragment))
                });
            let raw_path = without_fragment
                .split_once('?')
                .map_or(without_fragment, |(path, _)| path);
            let path = percent_decode(raw_path).replace('\\', "/");
            let source = root.join(normalize_note_path(&doc.path));
            let candidate = if path.is_empty() {
                source
            } else if path.starts_with('/') {
                root.join(path.trim_start_matches('/'))
            } else {
                let relative = source
                    .parent()
                    .unwrap_or(&root)
                    .join(path.trim_start_matches("./"));
                let explicit_relative = path.starts_with("./") || path.starts_with("../");
                if explicit_relative
                    || !is_within(&root, &relative)
                    || resolve_existing_target(&relative).is_some()
                {
                    relative
                } else {
                    let vault_rooted = root.join(&path);
                    if resolve_existing_target(&vault_rooted).is_some() {
                        vault_rooted
                    } else {
                        relative
                    }
                }
            };

            if let Some(issue) = resolve_target(&root, &candidate) {
                findings.push(finding(
                    "broken_markdown_link",
                    &doc.path,
                    Some(link.line + doc.body_start_line - 1),
                    format!("`{destination}`: {issue}"),
                    "FACT",
                ));
                continue;
            }

            let Some(fragment) = fragment.filter(|fragment| !fragment.is_empty()) else {
                continue;
            };
            let target = resolve_anchor_file(&candidate).unwrap_or(candidate);
            if !is_markdown_file(&target) {
                continue;
            }
            let target_key = target
                .strip_prefix(&root)
                .map(|path| path.to_string_lossy().replace('\\', "/"))
                .unwrap_or_default();
            let ids = if let Some(ids) = headings.get(&target_key) {
                Some(ids.clone())
            } else {
                fs::read_to_string(&target)
                    .ok()
                    .map(|text| heading_ids(&text))
            };
            let Some(ids) = ids else {
                continue;
            };
            let anchor = slug(&percent_decode(fragment));
            if !ids.contains(&anchor) {
                findings.push(finding(
                    "broken_link_anchor",
                    &doc.path,
                    Some(link.line + doc.body_start_line - 1),
                    format!(
                        "anchor `#{fragment}` does not exist in `{}`",
                        target.display()
                    ),
                    "INFERRED",
                ));
            }
        }
    }
    findings
}

fn resolve_target(root: &Path, candidate: &Path) -> Option<String> {
    if !is_within(root, candidate) {
        return Some("target escapes the indexed root".into());
    }
    let target = resolve_existing_target(candidate);
    let Some(target) = target else {
        return Some("target does not exist".into());
    };
    let canonical = target.canonicalize().unwrap_or(target);
    if !is_within(root, &canonical) {
        return Some("target resolves outside the indexed root".into());
    }
    None
}

fn resolve_existing_target(candidate: &Path) -> Option<PathBuf> {
    if candidate.exists() {
        return Some(candidate.to_path_buf());
    }
    if candidate.extension().is_some() {
        return None;
    }
    [
        candidate.with_extension("md"),
        candidate.join("README.md"),
        candidate.join("index.md"),
    ]
    .into_iter()
    .find(|path| path.exists())
}

fn resolve_anchor_file(candidate: &Path) -> Option<PathBuf> {
    let target = resolve_existing_target(candidate)?;
    if !target.is_dir() {
        return Some(target);
    }
    [target.join("README.md"), target.join("index.md")]
        .into_iter()
        .find(|path| path.is_file())
        .or(Some(target))
}

fn is_within(root: &Path, candidate: &Path) -> bool {
    let Ok(relative) = candidate.strip_prefix(root) else {
        return false;
    };
    let mut depth = 0usize;
    for component in relative.components() {
        match component {
            Component::Normal(_) => depth += 1,
            Component::CurDir => {}
            Component::ParentDir if depth > 0 => depth -= 1,
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => return false,
        }
    }
    true
}

fn is_machine_specific(destination: &str) -> bool {
    let lower = destination.to_ascii_lowercase();
    lower.starts_with("file:")
        || lower.starts_with("~/")
        || destination.starts_with("\\\\")
        || (destination.len() > 1
            && destination.as_bytes()[0].is_ascii_alphabetic()
            && destination.as_bytes()[1] == b':')
}

fn is_template_path(path: &str) -> bool {
    let normalized = normalize_note_path(path);
    normalized.split('/').any(|part| part == "templates") || normalized.ends_with(".template.md")
}

fn has_uri_scheme(destination: &str) -> bool {
    let Some((scheme, _)) = destination.split_once(':') else {
        return false;
    };
    !scheme.is_empty()
        && scheme.as_bytes()[0].is_ascii_alphabetic()
        && scheme
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'+' | b'.' | b'-'))
}

fn is_markdown_file(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("md"))
}

fn normalize_note_path(path: &str) -> String {
    path.replace('\\', "/")
}

fn percent_decode(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' && index + 2 < bytes.len() {
            if let (Some(high), Some(low)) = (hex(bytes[index + 1]), hex(bytes[index + 2])) {
                decoded.push(high * 16 + low);
                index += 3;
                continue;
            }
        }
        decoded.push(bytes[index]);
        index += 1;
    }
    String::from_utf8_lossy(&decoded).into_owned()
}

fn hex(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

fn heading_ids(markdown: &str) -> HashSet<String> {
    let mut ids = HashSet::new();
    let mut occurrences: HashMap<String, usize> = HashMap::new();
    for heading in Doc::parse(markdown).ast_headings() {
        let (text, explicit_id) = explicit_heading_id(&heading.text);
        let base = slug(text);
        if !base.is_empty() {
            let occurrence = occurrences.entry(base.clone()).or_default();
            let id = if *occurrence == 0 {
                base
            } else {
                format!("{base}-{occurrence}")
            };
            *occurrence += 1;
            ids.insert(id);
        }
        if let Some(explicit_id) = explicit_id {
            let id = slug(explicit_id);
            if !id.is_empty() {
                ids.insert(id);
            }
        }
    }
    ids
}

fn explicit_heading_id(text: &str) -> (&str, Option<&str>) {
    let Some((heading, id)) = text.rsplit_once("{#") else {
        return (text, None);
    };
    match id.strip_suffix('}') {
        Some(id) if !id.is_empty() => (heading.trim_end(), Some(id)),
        _ => (text, None),
    }
}

fn slug(text: &str) -> String {
    let mut result = String::new();
    let mut pending_dash = false;
    for character in text.chars().flat_map(char::to_lowercase) {
        if character.is_alphanumeric() || character == '_' {
            if pending_dash && !result.is_empty() && !result.ends_with('-') {
                result.push('-');
            }
            pending_dash = false;
            result.push(character);
        } else if character == '-' {
            pending_dash = false;
            if !result.is_empty() && !result.ends_with('-') {
                result.push('-');
            }
        } else if character.is_whitespace() {
            pending_dash = true;
        }
    }
    result.trim_matches('-').to_owned()
}
