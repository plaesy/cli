//! Schema-driven lenient reading for every IR (ADR 0009, section 2). Port of
//! `docs/adr/tools/lenient.js`.
//!
//! A reader that meets a document from a newer MINOR version moves unknown fields into
//! the nearest `extensions` object and reports one warning per field. An object with no
//! `extensions` keeps its unknown field, and a warning says so, so strict validation
//! still rejects it.
//!
//! Supported schema keywords: `properties`, `oneOf`, `anyOf`, `allOf`, `then`, `$ref` to
//! `#/$defs/...`, `items`, `additionalProperties` (as a schema), `additionalProperties:
//! false` and `unevaluatedProperties: false` (closed objects).

use serde_json::{Map, Value};

/// Applies lenient reading to `doc` in place and returns the warnings in order.
pub fn apply(schema: &Value, doc: &mut Value) -> Vec<String> {
    let mut warnings = Vec::new();
    lenient_node(schema, schema, doc, "", &mut warnings);
    warnings
}

fn resolve<'a>(root: &'a Value, node: &'a Value) -> &'a Value {
    let mut current = node;
    for _ in 0..32 {
        let Some(reference) = current.get("$ref").and_then(Value::as_str) else {
            break;
        };
        let Some(name) = reference.strip_prefix("#/$defs/") else {
            break;
        };
        match root.get("$defs").and_then(|d| d.get(name)) {
            Some(next) => current = next,
            None => break,
        }
    }
    current
}

/// Collects every object schema that applies to this node, including the branches of
/// `oneOf`, `anyOf`, `allOf`, and `then`.
fn object_schemas<'a>(
    root: &'a Value,
    schema: &'a Value,
    instance: &Value,
    out: &mut Vec<&'a Value>,
    depth: usize,
) {
    if depth > 32 {
        return;
    }
    let s = resolve(root, schema);
    if !s.is_object() {
        return;
    }
    let closed = s.get("additionalProperties") == Some(&Value::Bool(false))
        || s.get("unevaluatedProperties") == Some(&Value::Bool(false));
    if s.get("properties").is_some() || closed {
        out.push(s);
    }
    for key in ["oneOf", "anyOf", "allOf"] {
        for branch in s.get(key).and_then(Value::as_array).into_iter().flatten() {
            object_schemas(root, branch, instance, out, depth + 1);
        }
    }
    // A conditional applies only when its "if" matches the instance (ADR 0009, lenient).
    if let (Some(cond), Some(then)) = (s.get("if"), s.get("then")) {
        if matches_if(cond, instance) {
            object_schemas(root, then, instance, out, depth + 1);
        }
    }
}

/// Evaluates the subset of "if" used by the schemas: "properties" with "const" and
/// "enum", and "required". Any other keyword is treated as matching.
fn matches_if(cond: &Value, instance: &Value) -> bool {
    let Some(object) = instance.as_object() else {
        return false;
    };
    if let Some(required) = cond.get("required").and_then(Value::as_array) {
        for name in required.iter().filter_map(Value::as_str) {
            if !object.contains_key(name) {
                return false;
            }
        }
    }
    if let Some(props) = cond.get("properties").and_then(Value::as_object) {
        for (key, sub) in props {
            let Some(actual) = object.get(key) else {
                return false;
            };
            if let Some(c) = sub.get("const") {
                if actual != c {
                    return false;
                }
            }
            if let Some(list) = sub.get("enum").and_then(Value::as_array) {
                if !list.contains(actual) {
                    return false;
                }
            }
        }
    }
    true
}

fn lenient_node(
    root: &Value,
    schema: &Value,
    value: &mut Value,
    here: &str,
    warnings: &mut Vec<String>,
) {
    match value {
        Value::Array(items) => {
            let s = resolve(root, schema);
            let item_schema = s.get("items").cloned().unwrap_or(Value::Null);
            for (i, item) in items.iter_mut().enumerate() {
                lenient_node(root, &item_schema, item, &format!("{here}/{i}"), warnings);
            }
        }
        Value::Object(_) => lenient_object(root, schema, value, here, warnings),
        _ => {}
    }
}

fn lenient_object(
    root: &Value,
    schema: &Value,
    value: &mut Value,
    here: &str,
    warnings: &mut Vec<String>,
) {
    let mut branches = Vec::new();
    object_schemas(root, schema, value, &mut branches, 0);

    let mut known: Vec<String> = Vec::new();
    let mut closed = false;
    for b in &branches {
        if let Some(props) = b.get("properties").and_then(Value::as_object) {
            for k in props.keys() {
                if !known.contains(k) {
                    known.push(k.clone());
                }
            }
        }
        if b.get("additionalProperties") == Some(&Value::Bool(false))
            || b.get("unevaluatedProperties") == Some(&Value::Bool(false))
        {
            closed = true;
        }
    }
    let has_extensions = known.iter().any(|k| k == "extensions");

    let Some(object) = value.as_object_mut() else {
        return;
    };

    // Recurse into the values we keep.
    let keys: Vec<String> = object.keys().cloned().collect();
    for key in &keys {
        if known.contains(key) || key == "extensions" {
            if let Some(ps) = property_schema(root, &branches, key) {
                if let Some(child) = object.get_mut(key) {
                    lenient_node(root, &ps, child, &format!("{here}/{key}"), warnings);
                }
            }
        }
    }

    // Move or report the unknown keys.
    let unknown: Vec<String> = keys
        .iter()
        .filter(|k| !known.contains(k) && k.as_str() != "extensions")
        .cloned()
        .collect();
    if unknown.is_empty() || !closed {
        return;
    }
    if !has_extensions {
        for k in &unknown {
            warnings.push(format!(
                "{here}/{k}: field tidak dikenal, objek ini tidak punya extensions"
            ));
        }
        return;
    }
    let mut moved: Map<String, Value> = Map::new();
    for k in &unknown {
        if let Some(v) = object.remove(k) {
            moved.insert(k.clone(), v);
            warnings.push(format!(
                "{here}/{k}: field tidak dikenal, dipindah ke extensions"
            ));
        }
    }
    let ext = object
        .entry("extensions".to_string())
        .or_insert_with(|| Value::Object(Map::new()));
    if let Some(ext) = ext.as_object_mut() {
        for (k, v) in moved {
            ext.insert(k, v);
        }
    }
}

fn property_schema(root: &Value, branches: &[&Value], key: &str) -> Option<Value> {
    for b in branches {
        if let Some(p) = b.get("properties").and_then(|p| p.get(key)) {
            return Some(resolve(root, p).clone());
        }
    }
    for b in branches {
        if let Some(ap) = b.get("additionalProperties") {
            if ap.is_object() {
                return Some(resolve(root, ap).clone());
            }
        }
    }
    None
}
