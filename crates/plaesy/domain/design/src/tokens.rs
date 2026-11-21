//! Pemipihan token DTCG dari `tokens` (ADR 0010). Daun adalah objek yang punya `$value`.

use serde_json::Value;

/// Flattens `tokens` into `(dotted name, DTCG token object)` pairs in document order.
///
/// A leaf is an object that has `$value`. Other objects are groups. The `extensions`
/// key is skipped.
pub fn flatten_tokens(doc: &Value) -> Vec<(String, Value)> {
    let mut out = Vec::new();
    if let Some(root) = doc.get("tokens").and_then(Value::as_object) {
        for (category, group) in root {
            if category != "extensions" {
                walk_tokens(category, group, &mut out);
            }
        }
    }
    out
}

fn walk_tokens(prefix: &str, node: &Value, out: &mut Vec<(String, Value)>) {
    let Some(object) = node.as_object() else {
        return;
    };
    if object.contains_key("$value") {
        out.push((prefix.to_string(), node.clone()));
        return;
    }
    for (key, child) in object {
        if key != "extensions" {
            walk_tokens(&format!("{prefix}.{key}"), child, out);
        }
    }
}
