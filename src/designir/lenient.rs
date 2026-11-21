//! Lenient reading for `design-ir/1` (ADR 0009, section 2).
//!
//! A reader that meets a document from a newer MINOR version moves every unknown
//! field into the nearest `extensions` object and reports one warning per field.
//! The moved values are not interpreted. Objects without `extensions` keep the
//! unknown field and a warning says so, so strict validation still rejects it.
//!
//! This is a design-specific port of `docs/adr/tools/lenient.js`. The Node version
//! reads the JSON Schema; this one uses the known key sets of `design-ir/1`.

use serde_json::{Map, Value};

const ROOT: &[&str] = &[
    "ir",
    "id",
    "version",
    "name",
    "tokens",
    "components",
    "pages",
    "extensions",
];
const TOKEN_ROOT: &[&str] = &["color", "space", "radius", "font", "shadow", "extensions"];
const COMPONENT: &[&str] = &["id", "name", "root", "variants", "extensions"];
const PAGE: &[&str] = &["id", "name", "frames", "extensions"];
const NODE: &[&str] = &[
    "id",
    "type",
    "name",
    "x",
    "y",
    "width",
    "height",
    "layout",
    "style",
    "text",
    "src",
    "component",
    "variant",
    "children",
    "construct",
    "source",
    "path",
    "extensions",
];

/// Applies lenient reading in place and returns one warning per moved or kept field.
pub fn apply(doc: &mut Value) -> Vec<String> {
    let mut warnings = Vec::new();
    if let Some(object) = doc.as_object_mut() {
        close_object(object, ROOT, "", &mut warnings);
        if let Some(tokens) = object.get_mut("tokens").and_then(Value::as_object_mut) {
            close_object(tokens, TOKEN_ROOT, "/tokens", &mut warnings);
        }
        if let Some(components) = object.get_mut("components").and_then(Value::as_array_mut) {
            for (i, c) in components.iter_mut().enumerate() {
                let here = format!("/components/{i}");
                if let Some(object) = c.as_object_mut() {
                    close_object(object, COMPONENT, &here, &mut warnings);
                    if let Some(root) = object.get_mut("root") {
                        apply_node(root, &format!("{here}/root"), &mut warnings);
                    }
                }
            }
        }
        if let Some(pages) = object.get_mut("pages").and_then(Value::as_array_mut) {
            for (i, p) in pages.iter_mut().enumerate() {
                let here = format!("/pages/{i}");
                if let Some(object) = p.as_object_mut() {
                    close_object(object, PAGE, &here, &mut warnings);
                    if let Some(frames) = object.get_mut("frames").and_then(Value::as_array_mut) {
                        for (j, f) in frames.iter_mut().enumerate() {
                            apply_node(f, &format!("{here}/frames/{j}"), &mut warnings);
                        }
                    }
                }
            }
        }
    }
    warnings
}

fn apply_node(node: &mut Value, here: &str, warnings: &mut Vec<String>) {
    let Some(object) = node.as_object_mut() else {
        return;
    };
    close_object(object, NODE, here, warnings);
    if let Some(children) = object.get_mut("children").and_then(Value::as_array_mut) {
        for (i, child) in children.iter_mut().enumerate() {
            apply_node(child, &format!("{here}/children/{i}"), warnings);
        }
    }
}

/// Moves keys that are not in `known` into `extensions`, or keeps them with a warning
/// when the object has no `extensions` key.
fn close_object(
    object: &mut Map<String, Value>,
    known: &[&str],
    here: &str,
    warnings: &mut Vec<String>,
) {
    let unknown: Vec<String> = object
        .keys()
        .filter(|k| !known.contains(&k.as_str()))
        .cloned()
        .collect();
    if unknown.is_empty() {
        return;
    }
    let has_extensions = known.contains(&"extensions");
    for key in unknown {
        if !has_extensions {
            warnings.push(format!(
                "{here}/{key}: field tidak dikenal, objek ini tidak punya extensions"
            ));
            continue;
        }
        if let Some(value) = object.remove(&key) {
            let ext = object
                .entry("extensions".to_string())
                .or_insert_with(|| Value::Object(Map::new()));
            if let Some(ext) = ext.as_object_mut() {
                ext.insert(key.clone(), value);
            }
            warnings.push(format!(
                "{here}/{key}: field tidak dikenal, dipindah ke extensions"
            ));
        }
    }
}
