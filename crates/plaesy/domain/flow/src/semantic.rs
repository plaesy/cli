//! Aturan semantik `flow-ir/1` (ADR 0002, "Aturan semantik").
//! Port dari `validateFlow` di `docs/adr/tools/validate-ir.js`.

use std::collections::HashSet;

use serde_json::Value;

use crate::error::SemanticError;

/// Returns every semantic error in `doc`. An empty vector means the document passes.
pub fn validate(doc: &Value) -> Vec<SemanticError> {
    let mut errors = Vec::new();
    let modules = array(doc, "modules");
    let ids: Vec<&str> = modules.iter().filter_map(|m| str_of(m, "id")).collect();
    let id_set: HashSet<&str> = ids.iter().copied().collect();
    if id_set.len() != ids.len() {
        errors.push(SemanticError::new("id module tidak unik"));
    }
    let input_names: HashSet<&str> = doc
        .get("inputs")
        .and_then(Value::as_object)
        .map(|o| o.keys().map(String::as_str).collect())
        .unwrap_or_default();
    let scope_set: HashSet<&str> = strings(array(doc, "scope")).into_iter().collect();
    let flow_id = str_of(doc, "id");

    for (module_index, module) in modules.iter().enumerate() {
        let id = str_of(module, "id").unwrap_or("?");
        let here = format!("module {id}");

        let mut refs: Vec<&str> = Vec::new();
        refs.extend(strings(array(module, "default")));
        refs.extend(strings(array(module, "modules")));
        for branch in array(module, "branches") {
            refs.extend(strings(array(branch, "modules")));
        }
        if let Some(reject) = str_of(module, "on_reject") {
            refs.push(reject);
        }
        for r in refs {
            if !id_set.contains(r) {
                errors.push(SemanticError::new(format!(
                    "{here}: merujuk module yang tidak ada: {r}"
                )));
            }
            if r == id {
                errors.push(SemanticError::new(format!(
                    "{here}: merujuk dirinya sendiri"
                )));
            }
        }

        if let Some(on_error) = str_of(module, "on_error") {
            if on_error != "fail" && on_error != "continue" && !id_set.contains(on_error) {
                errors.push(SemanticError::new(format!(
                    "{here}: on_error merujuk module tidak ada: {on_error}"
                )));
            }
        }

        if str_of(module, "kind") == Some("subflow") && str_of(module, "flow_id") == flow_id {
            errors.push(SemanticError::new(format!(
                "{here}: subflow merujuk flow itu sendiri (rekursi)"
            )));
        }

        if let Some(scope) = str_of(module, "scope") {
            if !scope_set.contains(scope) {
                errors.push(SemanticError::new(format!(
                    "{here}: scope {scope} melebar dari scope flow"
                )));
            }
        }

        // Data references occur in operation input values and CEL expressions,
        // while descriptions and operation names are arbitrary text.
        for name in input_references_in_module(module) {
            if !input_names.contains(name.as_str()) {
                errors.push(SemanticError::new(format!(
                    "{here}: flow.input.{name} tidak dideklarasikan"
                )));
            }
        }
        for reference in module_references(module) {
            if reference == id {
                continue;
            }
            if let Some((target_index, _)) = modules
                .iter()
                .enumerate()
                .find(|(_, candidate)| str_of(candidate, "id") == Some(reference.as_str()))
            {
                if target_index >= module_index {
                    errors.push(SemanticError::new(format!(
                        "{here}: data reference points to module that has not run: {reference}"
                    )));
                }
            }
        }
    }

    if let Some(handler) = str_of(doc, "failure_handler") {
        if !id_set.contains(handler) {
            errors.push(SemanticError::new(format!(
                "failure_handler merujuk module tidak ada: {handler}"
            )));
        }
    }
    errors
}

fn input_references_in_module(module: &Value) -> Vec<String> {
    let mut expressions = String::new();
    collect_expression_values(module, &mut expressions);
    if let Some(inputs) = module.get("inputs") {
        collect_string_values(inputs, &mut expressions);
    }
    input_references(&expressions)
        .into_iter()
        .map(str::to_owned)
        .collect()
}

fn module_references(module: &Value) -> Vec<String> {
    let mut refs = Vec::new();
    collect_module_references(module, None, &mut refs);
    refs
}

fn collect_module_references(value: &Value, parent_key: Option<&str>, out: &mut Vec<String>) {
    match value {
        Value::String(text)
            if parent_key.is_some_and(|key| {
                matches!(
                    key,
                    "when" | "condition" | "retry_if" | "skip_if" | "stop_after_if"
                )
            }) =>
        {
            for token in text.split(|c: char| !(c.is_ascii_alphanumeric() || c == '_')) {
                if token.starts_with('m') && token.len() > 1 {
                    out.push(token.to_owned());
                }
            }
        }
        Value::Array(items) => {
            for item in items {
                collect_module_references(item, parent_key, out);
            }
        }
        Value::Object(fields) => {
            for (key, child) in fields {
                collect_module_references(child, Some(key), out);
            }
        }
        _ => {}
    }
}

fn collect_expression_values(value: &Value, out: &mut String) {
    match value {
        Value::String(_) => {}
        Value::Array(items) => {
            for item in items {
                collect_expression_values(item, out);
            }
        }
        Value::Object(fields) => {
            for (key, child) in fields {
                if matches!(
                    key.as_str(),
                    "when" | "condition" | "retry_if" | "skip_if" | "stop_after_if"
                ) {
                    if let Some(expression) = child.as_str() {
                        out.push_str(expression);
                        out.push('\n');
                    }
                } else {
                    collect_expression_values(child, out);
                }
            }
        }
        _ => {}
    }
}

fn collect_string_values(value: &Value, out: &mut String) {
    match value {
        Value::String(text) => {
            out.push_str(text);
            out.push('\n');
        }
        Value::Array(items) => items
            .iter()
            .for_each(|item| collect_string_values(item, out)),
        Value::Object(fields) => fields
            .values()
            .for_each(|item| collect_string_values(item, out)),
        _ => {}
    }
}

fn str_of<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
    value.get(key).and_then(Value::as_str)
}

fn array<'a>(value: &'a Value, key: &str) -> &'a [Value] {
    value
        .get(key)
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[])
}

fn strings(values: &[Value]) -> Vec<&str> {
    values.iter().filter_map(Value::as_str).collect()
}

fn is_identifier(name: &str) -> bool {
    let mut chars = name.chars();
    match chars.next() {
        Some(c) if c.is_ascii_alphabetic() || c == '_' => {}
        _ => return false,
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// Finds every `flow.input.<name>` in `text` and returns the names.
fn input_references(text: &str) -> Vec<&str> {
    const PREFIX: &str = "flow.input.";
    let mut names = Vec::new();
    let mut rest = text;
    while let Some(pos) = rest.find(PREFIX) {
        let after = &rest[pos + PREFIX.len()..];
        let end = after
            .char_indices()
            .find(|(_, c)| !(c.is_ascii_alphanumeric() || *c == '_'))
            .map(|(i, _)| i)
            .unwrap_or(after.len());
        let name = &after[..end];
        if is_identifier(name) {
            names.push(name);
        }
        rest = &after[end..];
    }
    names
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn base() -> Value {
        json!({
            "ir": "flow-ir/1",
            "id": "flw_1",
            "inputs": { "x": { "type": "string" } },
            "scope": ["a:read"],
            "modules": [
                { "id": "m1", "kind": "action", "scope": "a:read",
                  "inputs": { "v": "flow.input.x" }, "default": ["m2"] },
                { "id": "m2", "kind": "action", "scope": "a:read" }
            ]
        })
    }

    #[test]
    fn valid_document_has_no_errors() {
        assert!(validate(&base()).is_empty());
    }

    #[test]
    fn duplicate_module_id_is_rejected() {
        let mut doc = base();
        doc["modules"][1]["id"] = json!("m1");
        let errs = validate(&doc);
        assert!(errs.iter().any(|e| e.message == "id module tidak unik"));
    }

    #[test]
    fn missing_module_reference_is_rejected() {
        let mut doc = base();
        doc["modules"][0]["default"] = json!(["ghost"]);
        let errs = validate(&doc);
        assert!(errs
            .iter()
            .any(|e| e.message.contains("merujuk module yang tidak ada: ghost")));
    }

    #[test]
    fn forward_module_reference_is_rejected() {
        let mut doc = base();
        doc["modules"][0]["condition"] = json!("m2.value == 1");
        let errs = validate(&doc);
        assert!(errs.iter().any(|e| e.message.contains("has not run: m2")));
    }

    #[test]
    fn input_like_text_in_metadata_is_not_a_data_reference() {
        let mut doc = base();
        doc["modules"][0]["description"] = json!("Example: flow.input.not_a_real_input");
        assert!(validate(&doc).is_empty());
    }

    #[test]
    fn undeclared_input_in_expression_is_rejected() {
        let mut doc = base();
        doc["modules"][0]["condition"] = json!("flow.input.not_declared == true");
        let errs = validate(&doc);
        assert!(errs.iter().any(|e| e
            .message
            .contains("flow.input.not_declared tidak dideklarasikan")));
    }

    #[test]
    fn self_reference_is_rejected() {
        let mut doc = base();
        doc["modules"][0]["default"] = json!(["m1"]);
        let errs = validate(&doc);
        assert!(errs
            .iter()
            .any(|e| e.message.contains("merujuk dirinya sendiri")));
    }

    #[test]
    fn undeclared_flow_input_is_rejected() {
        let mut doc = base();
        doc["modules"][0]["inputs"]["v"] = json!("flow.input.nope");
        let errs = validate(&doc);
        // Literal input values in the data map are also references, unlike
        // arbitrary descriptive strings.
        assert!(errs
            .iter()
            .any(|e| e.message.contains("flow.input.nope tidak dideklarasikan")));
    }

    #[test]
    fn scope_wider_than_flow_is_rejected() {
        let mut doc = base();
        doc["modules"][0]["scope"] = json!("billing:write");
        let errs = validate(&doc);
        assert!(errs
            .iter()
            .any(|e| e.message.contains("melebar dari scope flow")));
    }

    #[test]
    fn recursive_subflow_is_rejected() {
        let mut doc = base();
        doc["modules"][0]["kind"] = json!("subflow");
        doc["modules"][0]["flow_id"] = json!("flw_1");
        let errs = validate(&doc);
        assert!(errs.iter().any(|e| e.message.contains("rekursi")));
    }
}
