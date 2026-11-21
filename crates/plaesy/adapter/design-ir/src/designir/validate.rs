//! Semantic rules for `design-ir/1` (ADR 0010, section "Aturan semantik").
//! Port of `validateDesign` in `docs/adr/tools/validate-ir.js`.

use std::collections::{HashMap, HashSet};

use serde_json::Value;

use super::{array, flatten_tokens, str_of};

const CATEGORIES: &[&str] = &["color", "space", "radius", "font", "shadow"];
const NODE_TYPES: &[&str] = &[
    "frame", "rect", "text", "image", "vector", "instance", "opaque",
];

/// Token category that each style or layout property must use.
const USE_CATEGORY: &[(&str, &str)] = &[
    ("fill", "color"),
    ("stroke", "color"),
    ("text_color", "color"),
    ("radius", "radius"),
    ("shadow", "shadow"),
    ("font", "font"),
    ("padding", "space"),
    ("gap", "space"),
];

/// Returns every semantic error in `doc`. An empty list means the document passes.
pub fn validate(doc: &Value) -> Vec<String> {
    let mut errors = Vec::new();
    let tokens = flatten_tokens(doc);
    for (name, token) in &tokens {
        check_token(&mut errors, name, token);
    }
    let components = array(doc, "components");
    let mut ctx = Ctx {
        errors,
        token_names: tokens.iter().map(|(name, _)| name.as_str()).collect(),
        component_map: components
            .iter()
            .filter_map(|c| str_of(c, "id").map(|id| (id, c)))
            .collect(),
        seen: HashSet::new(),
    };
    for component in components {
        let id = str_of(component, "id").unwrap_or("?");
        if let Some(root) = component.get("root") {
            ctx.check_node(root, &format!("komponen {id}"));
        }
    }
    for page in array(doc, "pages") {
        let id = str_of(page, "id").unwrap_or("?");
        for frame in array(page, "frames") {
            ctx.check_node(frame, &format!("halaman {id}"));
        }
    }
    ctx.errors
}

struct Ctx<'a> {
    errors: Vec<String>,
    token_names: HashSet<&'a str>,
    component_map: HashMap<&'a str, &'a Value>,
    seen: HashSet<String>,
}

impl Ctx<'_> {
    fn check_node(&mut self, node: &Value, location: &str) {
        let node_id = str_of(node, "id").unwrap_or("?");
        if node_id == "?" {
            self.errors.push(format!("{location}: node tanpa id"));
        } else if !self.seen.insert(node_id.to_string()) {
            self.errors.push(format!("node duplikat: {node_id}"));
        }
        let here = format!("{location}/{node_id}");

        let kind = str_of(node, "type").unwrap_or("");
        if !NODE_TYPES.contains(&kind) {
            self.errors
                .push(format!("{here}: tipe node tidak dikenal: {kind}"));
        }

        for group in ["style", "layout"] {
            let Some(object) = node.get(group).and_then(Value::as_object) else {
                continue;
            };
            for (key, value) in object {
                let Some(reference) = value.as_str() else {
                    continue;
                };
                if !is_token_ref(reference) {
                    continue;
                }
                if !self.token_names.contains(reference) {
                    self.errors
                        .push(format!("{here}: token {reference} tidak ada"));
                }
                if let Some((_, want)) = USE_CATEGORY.iter().find(|(k, _)| k == key) {
                    if !reference.starts_with(&format!("{want}.")) {
                        let got = reference.split('.').next().unwrap_or("");
                        self.errors.push(format!(
                            "{here}: properti {key} memakai token kategori {got}, seharusnya {want}"
                        ));
                    }
                }
            }
        }

        if kind == "instance" {
            self.check_instance(node, &here);
        }

        for child in array(node, "children") {
            self.check_node(child, location);
        }
    }

    fn check_instance(&mut self, node: &Value, here: &str) {
        let component_id = str_of(node, "component").unwrap_or("");
        let Some(component) = self.component_map.get(component_id).copied() else {
            self.errors
                .push(format!("{here}: komponen tidak ada: {component_id}"));
            return;
        };
        let variants = component.get("variants").and_then(Value::as_object);
        let Some(values) = node.get("variant").and_then(Value::as_object) else {
            return;
        };
        for (name, value) in values {
            let allowed = variants.and_then(|v| v.get(name)).and_then(Value::as_array);
            match allowed {
                None => self.errors.push(format!(
                    "{here}: variant {name} tidak ada di {component_id}"
                )),
                Some(list) if !list.contains(value) => self.errors.push(format!(
                    "{here}: nilai {value} tidak termasuk variant {name}"
                )),
                Some(_) => {}
            }
        }
    }
}

/// A token reference is `<category>.<name>`, where the name uses `[A-Za-z0-9_.-]`.
pub fn is_token_ref(value: &str) -> bool {
    let Some((category, name)) = value.split_once('.') else {
        return false;
    };
    CATEGORIES.contains(&category)
        && !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '.' || c == '-')
}

fn check_token(errors: &mut Vec<String>, name: &str, token: &Value) {
    let value = token.get("$value");
    let ok = match str_of(token, "$type") {
        Some("color") => value.and_then(Value::as_str).is_some_and(is_color),
        Some("dimension") => value
            .and_then(Value::as_str)
            .and_then(|s| s.strip_suffix("px"))
            .is_some_and(|n| n.parse::<f64>().is_ok()),
        Some("typography") => has_keys(value, &["fontFamily", "fontSize", "fontWeight"]),
        Some("shadow") => has_keys(value, &["color", "offsetX", "offsetY", "blur"]),
        other => {
            errors.push(format!("token {name}: $type tidak dikenal ({other:?})"));
            return;
        }
    };
    if !ok {
        errors.push(format!(
            "token {name}: nilai {} tidak sesuai $type",
            str_of(token, "$type").unwrap_or("?")
        ));
    }
}

fn has_keys(value: Option<&Value>, keys: &[&str]) -> bool {
    value
        .and_then(Value::as_object)
        .is_some_and(|object| keys.iter().all(|k| object.contains_key(*k)))
}

fn is_color(value: &str) -> bool {
    if let Some(hex) = value.strip_prefix('#') {
        return (3..=8).contains(&hex.len()) && hex.chars().all(|c| c.is_ascii_hexdigit());
    }
    (value.starts_with("rgb(") || value.starts_with("rgba(")) && value.ends_with(')')
}
