//! Codegen from `design-ir/1` to Dioxus 0.7 components (ADR 0010, "Konversi").
//!
//! The output is Rust source that uses `#[component]` and `rsx!`. The generator only
//! writes text, so it needs no dependency. Compiling the output needs the
//! `designir-dioxus` feature, which is checked by `dioxus_sample`.
//!
//! Each node becomes one element with `data-plaesy-id` and `data-plaesy-kind`, so the
//! IDs and hierarchy survive into the rendered tree.

use std::fmt::Write as _;

use serde_json::Value;

use super::validate::validate;
use super::{array, flatten_tokens, str_of, token_value_string, DesignError};

/// Generates the Dioxus component module for `doc`. The document must pass `validate`.
pub fn generate(doc: &Value) -> Result<String, DesignError> {
    let errors = validate(doc);
    if !errors.is_empty() {
        return Err(DesignError::Invalid(errors));
    }
    let design_id = str_of(doc, "id")
        .ok_or_else(|| DesignError::Malformed("id dokumen tidak ada".to_string()))?;
    let tokens: Vec<(String, String)> = flatten_tokens(doc)
        .into_iter()
        .map(|(name, token)| {
            let value = token
                .get("$value")
                .map(token_value_string)
                .unwrap_or_default();
            (name, value)
        })
        .collect();

    let mut out = String::new();
    let _ = writeln!(
        out,
        "// Dihasilkan dari design-ir/1 `{design_id}` (ADR 0010). Jangan diedit manual."
    );
    out.push_str("#![allow(non_snake_case)]\n\n");
    out.push_str("use dioxus::prelude::*;\n");

    for component in array(doc, "components") {
        let id = str_of(component, "id").unwrap_or("?");
        let name = pascal_case(id);
        let Some(root) = component.get("root") else {
            continue;
        };
        out.push('\n');
        let _ = writeln!(out, "#[component]");
        let _ = writeln!(out, "pub fn {name}() -> Element {{");
        out.push_str("    rsx! {\n");
        write_node(&mut out, root, &tokens, 2);
        out.push_str("    }\n}\n");
    }

    for page in array(doc, "pages") {
        let id = str_of(page, "id").unwrap_or("?");
        let name = pascal_case(id);
        out.push('\n');
        out.push_str("#[component]\n");
        let _ = writeln!(out, "pub fn {name}() -> Element {{");
        out.push_str("    rsx! {\n");
        for frame in array(page, "frames") {
            write_node(&mut out, frame, &tokens, 2);
        }
        out.push_str("    }\n}\n");
    }
    Ok(out)
}

/// `cp_button` -> `CpButton`, `pg_orders` -> `PgOrders`.
pub fn pascal_case(id: &str) -> String {
    id.split(['_', '-', '.'])
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().chain(chars).collect::<String>(),
                None => String::new(),
            }
        })
        .collect()
}

fn write_node(out: &mut String, node: &Value, tokens: &[(String, String)], depth: usize) {
    let indent = "    ".repeat(depth);
    let id = str_of(node, "id").unwrap_or("?");
    let kind = str_of(node, "type").unwrap_or("");
    let tag = match kind {
        "text" => "span",
        "image" => "img",
        _ => "div",
    };

    let mut attrs: Vec<String> = vec![
        format!("\"data-plaesy-id\": {}", quote(id)),
        format!("\"data-plaesy-kind\": {}", quote(kind)),
    ];
    if let Some(component) = str_of(node, "component") {
        attrs.push(format!("\"data-plaesy-component\": {}", quote(component)));
    }
    if let Some(variant) = node.get("variant").and_then(Value::as_object) {
        for (name, value) in variant {
            let text = value
                .as_str()
                .map(str::to_string)
                .unwrap_or_else(|| value.to_string());
            attrs.push(format!("\"data-plaesy-variant-{name}\": {}", quote(&text)));
        }
    }
    if let Some(style) = inline_style(node, tokens) {
        attrs.push(format!("style: {}", quote(&style)));
    }
    if tag == "img" {
        let src = str_of(node, "src").unwrap_or("");
        attrs.push(format!("src: {}", quote(src)));
        attrs.push(format!("alt: {}", quote(id)));
    }

    let _ = writeln!(out, "{indent}{tag} {{");
    let inner = "    ".repeat(depth + 1);
    for attr in attrs {
        let _ = writeln!(out, "{inner}{attr},");
    }
    if kind == "text" {
        let text = str_of(node, "text").unwrap_or("");
        // Teks ditulis sebagai ekspresi agar kurung kurawal di dalamnya tidak ditafsirkan rsx.
        let _ = writeln!(out, "{inner}{{ {} }}", quote(text));
    }
    for child in array(node, "children") {
        write_node(out, child, tokens, depth + 1);
    }
    let _ = writeln!(out, "{indent}}}");
}

/// Builds the inline CSS for the style properties that map to tokens.
fn inline_style(node: &Value, tokens: &[(String, String)]) -> Option<String> {
    let style = node.get("style").and_then(Value::as_object)?;
    let mut parts = Vec::new();
    for (key, css) in [
        ("fill", "background-color"),
        ("text_color", "color"),
        ("radius", "border-radius"),
    ] {
        let Some(reference) = style.get(key).and_then(Value::as_str) else {
            continue;
        };
        let value = tokens
            .iter()
            .find(|(name, _)| name == reference)
            .map(|(_, v)| v.as_str());
        if let Some(value) = value {
            parts.push(format!("{css}: {value}"));
        }
    }
    if parts.is_empty() {
        None
    } else {
        Some(parts.join("; "))
    }
}

/// Writes `s` as a Rust string literal.
fn quote(s: &str) -> String {
    format!("{s:?}")
}
