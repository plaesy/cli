//! Semantic rules for `api-ir/1` (ADR 0005, "Aturan semantik"). Port of `validateApi`.

use std::collections::HashSet;

use serde_json::Value;

use plaesy_kernel::json::{array, str_of};

pub fn validate(doc: &Value) -> Vec<String> {
    let mut errors = Vec::new();
    let types: HashSet<&str> = doc
        .get("types")
        .and_then(Value::as_object)
        .map(|o| o.keys().map(String::as_str).collect())
        .unwrap_or_default();

    if let Some(types_value) = doc.get("types") {
        check_refs(&mut errors, &types, types_value, "types");
    }

    let mut seen: HashSet<String> = HashSet::new();
    for endpoint in array(doc, "endpoints") {
        let id = str_of(endpoint, "id").unwrap_or("?");
        let method = str_of(endpoint, "method").unwrap_or("");
        let path = str_of(endpoint, "path").unwrap_or("");
        let key = format!("{method} {path}");
        if !seen.insert(key.clone()) {
            errors.push(format!("endpoint duplikat: {key}"));
        }

        let placeholders = placeholders(path);
        let declared: Vec<&str> = endpoint
            .get("path_params")
            .and_then(Value::as_object)
            .map(|o| o.keys().map(String::as_str).collect())
            .unwrap_or_default();
        for p in &placeholders {
            if !declared.contains(&p.as_str()) {
                errors.push(format!(
                    "{id}: placeholder {{{p}}} tidak dideklarasikan di path_params"
                ));
            }
        }
        for d in &declared {
            if !placeholders.iter().any(|p| p == d) {
                errors.push(format!("{id}: path_params {d} tidak ada di path"));
            }
        }

        if (method == "GET" || method == "DELETE") && endpoint.get("body").is_some() {
            errors.push(format!("{id}: {method} tidak boleh punya body"));
        }

        for field in ["body", "responses", "query_params", "path_params"] {
            if let Some(value) = endpoint.get(field) {
                check_refs(&mut errors, &types, value, id);
            }
        }
    }
    errors
}

/// Every object with a string `$ref` must name a type that exists in `types`.
fn check_refs(errors: &mut Vec<String>, types: &HashSet<&str>, node: &Value, where_: &str) {
    match node {
        Value::Object(object) => {
            if let Some(target) = object.get("$ref").and_then(Value::as_str) {
                if !types.contains(target) {
                    errors.push(format!("$ref tidak ada: {target} (di {where_})"));
                }
            }
            for v in object.values() {
                check_refs(errors, types, v, where_);
            }
        }
        Value::Array(items) => {
            for v in items {
                check_refs(errors, types, v, where_);
            }
        }
        _ => {}
    }
}

/// Names inside `{...}` in a path, where each name is `[a-z][a-z0-9_]*`.
fn placeholders(path: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = path;
    while let Some(start) = rest.find('{') {
        let after = &rest[start + 1..];
        let Some(end) = after.find('}') else {
            break;
        };
        let name = &after[..end];
        let valid = name.chars().next().is_some_and(|c| c.is_ascii_lowercase())
            && name
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_');
        if valid {
            out.push(name.to_string());
        }
        rest = &after[end + 1..];
    }
    out
}
