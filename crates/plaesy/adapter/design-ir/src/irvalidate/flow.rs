//! Semantic rules for `flow-ir/1` (ADR 0002, "Aturan semantik"). Port of `validateFlow`.

use std::collections::HashSet;

use serde_json::Value;

use super::{array, is_identifier, str_of, strings};

pub fn validate(doc: &Value) -> Vec<String> {
    let mut errors = Vec::new();
    let modules = array(doc, "modules");
    let ids: Vec<&str> = modules.iter().filter_map(|m| str_of(m, "id")).collect();
    let id_set: HashSet<&str> = ids.iter().copied().collect();
    if id_set.len() != ids.len() {
        errors.push("id module tidak unik".to_string());
    }
    let input_names: HashSet<&str> = doc
        .get("inputs")
        .and_then(Value::as_object)
        .map(|o| o.keys().map(String::as_str).collect())
        .unwrap_or_default();
    let scope_set: HashSet<&str> = strings(array(doc, "scope")).into_iter().collect();
    let flow_id = str_of(doc, "id");

    for module in modules {
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
                errors.push(format!("{here}: merujuk module yang tidak ada: {r}"));
            }
            if r == id {
                errors.push(format!("{here}: merujuk dirinya sendiri"));
            }
        }

        if let Some(on_error) = str_of(module, "on_error") {
            if on_error != "fail" && on_error != "continue" && !id_set.contains(on_error) {
                errors.push(format!(
                    "{here}: on_error merujuk module tidak ada: {on_error}"
                ));
            }
        }

        if str_of(module, "kind") == Some("subflow") && str_of(module, "flow_id") == flow_id {
            errors.push(format!(
                "{here}: subflow merujuk flow itu sendiri (rekursi)"
            ));
        }

        if let Some(scope) = str_of(module, "scope") {
            if !scope_set.contains(scope) {
                errors.push(format!("{here}: scope {scope} melebar dari scope flow"));
            }
        }

        let text = module.to_string();
        for name in input_references(&text) {
            if !input_names.contains(name) {
                errors.push(format!("{here}: flow.input.{name} tidak dideklarasikan"));
            }
        }
    }

    if let Some(handler) = str_of(doc, "failure_handler") {
        if !id_set.contains(handler) {
            errors.push(format!(
                "failure_handler merujuk module tidak ada: {handler}"
            ));
        }
    }
    errors
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
