//! Semantic validators in Rust for `flow-ir/1`, `data-ir/1`, `api-ir/1`, and `app-ir/1`
//! (ADR 0002, 0004, 0005, 0006). Each module is a port of the matching function in
//! `docs/adr/tools/validate-ir.js`, and both must agree on every example and fixture.
//! The design IR lives in `crate::designir`.
//!
//! Shape checks (JSON Schema) are not repeated here. They are done by `check-ir.js`
//! and, for lenient reading, by `lenient`.

pub mod api;
pub mod app;
pub mod data;
pub mod flow;
pub mod lenient;

use serde_json::Value;

#[cfg(test)]
#[path = "irvalidate_test.rs"]
mod irvalidate_test;

/// Returns the semantic errors for a document, dispatching on its `ir` field.
/// Unknown IR names return a single error.
pub fn validate(doc: &Value) -> Vec<String> {
    match doc.get("ir").and_then(Value::as_str) {
        Some("flow-ir/1") => flow::validate(doc),
        Some("data-ir/1") => data::validate(doc),
        Some("api-ir/1") => api::validate(doc),
        Some("app-ir/1") => app::validate(doc),
        Some("design-ir/1") => crate::designir::validate::validate(doc),
        other => vec![format!("ir tidak dikenal: {other:?}")],
    }
}

/// Returns the string at `key`, if present.
pub fn str_of<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
    value.get(key).and_then(Value::as_str)
}

/// Returns the array at `key`, or an empty slice when missing.
pub fn array<'a>(value: &'a Value, key: &str) -> &'a [Value] {
    value
        .get(key)
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[])
}

/// Returns the object at `key`, or an empty map when missing.
pub fn object<'a>(value: &'a Value, key: &str) -> Option<&'a serde_json::Map<String, Value>> {
    value.get(key).and_then(Value::as_object)
}

/// Collects the string values of an array of strings.
pub fn strings(values: &[Value]) -> Vec<&str> {
    values.iter().filter_map(Value::as_str).collect()
}

/// Returns true when `name` looks like an identifier: `[A-Za-z_][A-Za-z0-9_]*`.
pub fn is_identifier(name: &str) -> bool {
    let mut chars = name.chars();
    match chars.next() {
        Some(c) if c.is_ascii_alphabetic() || c == '_' => {}
        _ => return false,
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}
