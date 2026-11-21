//! Design IR `design-ir/1` (ADR 0010): semantic validation, code generation to a
//! Rust module, and a structural round trip between the IR and that module.
//!
//! Scope of this first slice:
//! - Validation mirrors the rules in `docs/adr/tools/validate-ir.js` (DTCG tokens,
//!   token categories, component instances, unique node IDs).
//! - Codegen writes a Rust module with a token table and a node table. It does not
//!   yet emit Dioxus `rsx!` components; that is a later adapter step.
//! - The round trip checks that IDs, hierarchy, node kinds, and token values survive
//!   IR -> Rust module -> IR. It does not compare rendered output.
//! - Lenient reading moves unknown fields into `extensions` and reports a warning
//!   for each (ADR 0009, section 2).

pub mod codegen;
pub mod dioxus_codegen;
pub mod lenient;
pub mod roundtrip;
pub mod validate;

use serde_json::Value;
use thiserror::Error;

#[cfg(test)]
#[path = "designir_test.rs"]
mod designir_test;

/// Typed errors for design IR operations.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum DesignError {
    /// The document breaks one or more semantic rules.
    #[error("dokumen design tidak valid: {0:?}")]
    Invalid(Vec<String>),
    /// The document or generated module does not have the expected shape.
    #[error("bentuk dokumen design tidak sesuai: {0}")]
    Malformed(String),
}

/// Returns the string value of `key` in `value`, if it is a string.
pub fn str_of<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
    value.get(key).and_then(Value::as_str)
}

/// Returns the array at `key`, or an empty slice when it is missing.
pub fn array<'a>(value: &'a Value, key: &str) -> &'a [Value] {
    value
        .get(key)
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[])
}

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

/// Canonical string form of a DTCG `$value`: strings as they are, other values as
/// compact JSON.
pub fn token_value_string(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

#[cfg(test)]
#[path = "dioxus_test.rs"]
mod dioxus_test;

/// Komponen Dioxus hasil generate dari `order-desk-ui.json`. Hanya dikompilasi dengan
/// feature `designir-dioxus`, sehingga keluaran generator benar-benar valid terhadap Dioxus 0.7.
#[cfg(feature = "designir-dioxus")]
#[allow(dead_code)]
mod dioxus_sample;
