//! Code generation from `design-ir/1` to a Rust module (ADR 0010, "Konversi").
//!
//! The generated module has three constants: `DESIGN_ID`, `TOKENS`, and `NODES`. The
//! layout is fixed and line-oriented so that `roundtrip` can read it back. Each node
//! occupies exactly one line in `NODES`.

use std::fmt::Write as _;

use serde_json::Value;

use super::validate::validate;
use super::{array, flatten_tokens, str_of, token_value_string, DesignError};

/// One node in document order: the ID, its parent ID, its kind, and its scope.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NodeRow {
    pub id: String,
    pub parent: Option<String>,
    pub kind: String,
    pub scope: String,
}

/// Lists every node in pre-order, with components first and then pages.
pub fn node_rows(doc: &Value) -> Vec<NodeRow> {
    let mut rows = Vec::new();
    for component in array(doc, "components") {
        let scope = format!("component:{}", str_of(component, "id").unwrap_or("?"));
        if let Some(root) = component.get("root") {
            push_rows(root, None, &scope, &mut rows);
        }
    }
    for page in array(doc, "pages") {
        let scope = format!("page:{}", str_of(page, "id").unwrap_or("?"));
        for frame in array(page, "frames") {
            push_rows(frame, None, &scope, &mut rows);
        }
    }
    rows
}

fn push_rows(node: &Value, parent: Option<&str>, scope: &str, rows: &mut Vec<NodeRow>) {
    let id = str_of(node, "id").unwrap_or("?").to_string();
    rows.push(NodeRow {
        id: id.clone(),
        parent: parent.map(str::to_string),
        kind: str_of(node, "type").unwrap_or("").to_string(),
        scope: scope.to_string(),
    });
    for child in array(node, "children") {
        push_rows(child, Some(&id), scope, rows);
    }
}

/// Lists `(dotted token name, value string)` pairs in document order.
pub fn token_rows(doc: &Value) -> Vec<(String, String)> {
    flatten_tokens(doc)
        .into_iter()
        .map(|(name, token)| {
            let value = token
                .get("$value")
                .map(token_value_string)
                .unwrap_or_default();
            (name, value)
        })
        .collect()
}

/// Generates the Rust module for `doc`. The document must pass `validate` first.
pub fn generate(doc: &Value) -> Result<String, DesignError> {
    let errors = validate(doc);
    if !errors.is_empty() {
        return Err(DesignError::Invalid(errors));
    }
    let design_id = str_of(doc, "id")
        .ok_or_else(|| DesignError::Malformed("id dokumen tidak ada".to_string()))?;

    let mut out = String::new();
    out.push_str("// Dihasilkan dari design-ir/1 (ADR 0010). Jangan diedit manual.\n");
    out.push_str(
        "// Kontrak baris: satu NodeSpec per baris di NODES, satu entri per baris di TOKENS.\n\n",
    );
    out.push_str("pub struct NodeSpec {\n");
    out.push_str("    pub id: &'static str,\n");
    out.push_str("    pub parent: Option<&'static str>,\n");
    out.push_str("    pub kind: &'static str,\n");
    out.push_str("    pub scope: &'static str,\n");
    out.push_str("}\n\n");
    let _ = writeln!(out, "pub const DESIGN_ID: &str = {};\n", quote(design_id));

    out.push_str("pub const TOKENS: &[(&str, &str)] = &[\n");
    for (name, value) in token_rows(doc) {
        let _ = writeln!(out, "    ({}, {}),", quote(&name), quote(&value));
    }
    out.push_str("];\n\n");

    out.push_str("pub const NODES: &[NodeSpec] = &[\n");
    for row in node_rows(doc) {
        let parent = match &row.parent {
            None => "None".to_string(),
            Some(p) => format!("Some({})", quote(p)),
        };
        let _ = writeln!(
            out,
            "    NodeSpec {{ id: {}, parent: {}, kind: {}, scope: {} }},",
            quote(&row.id),
            parent,
            quote(&row.kind),
            quote(&row.scope)
        );
    }
    out.push_str("];\n");
    Ok(out)
}

/// Writes `s` as a Rust string literal. `Debug` escapes quotes and backslashes.
fn quote(s: &str) -> String {
    format!("{s:?}")
}
