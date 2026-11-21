//! Semantic rules for `app-ir/1` (ADR 0006, "Aturan semantik"). Port of `validateApp`.

use std::collections::{HashMap, HashSet};

use serde_json::Value;

use super::{array, str_of, strings};

pub fn validate(doc: &Value) -> Vec<String> {
    let mut errors = Vec::new();
    let screens = array(doc, "screens");

    let mut screen_map: HashMap<&str, &Value> = HashMap::new();
    let mut routes: HashSet<&str> = HashSet::new();
    for screen in screens {
        let id = str_of(screen, "id").unwrap_or("?");
        if screen_map.insert(id, screen).is_some() {
            errors.push(format!("layar duplikat: {id}"));
        }
        let route = str_of(screen, "route").unwrap_or("");
        if !routes.insert(route) {
            errors.push(format!("route duplikat: {route}"));
        }
        let declared: HashSet<&str> = object_keys(screen, "params");
        for name in route_params(route) {
            if !declared.contains(name.as_str()) {
                errors.push(format!(
                    "{id}: parameter route :{name} tidak dideklarasikan"
                ));
            }
        }
    }

    if let Some(start) = str_of(doc, "start_screen") {
        if !screen_map.contains_key(start) {
            errors.push(format!("start_screen tidak ada: {start}"));
        }
    }

    let mut component_ids: HashSet<String> = HashSet::new();
    for screen in screens {
        if let Some(root) = screen.get("root") {
            let mut ctx = Walk {
                errors: &mut errors,
                component_ids: &mut component_ids,
                screen_map: &screen_map,
                screen,
            };
            ctx.component(root);
        }
    }
    errors
}

struct Walk<'a> {
    errors: &'a mut Vec<String>,
    component_ids: &'a mut HashSet<String>,
    screen_map: &'a HashMap<&'a str, &'a Value>,
    screen: &'a Value,
}

impl Walk<'_> {
    fn component(&mut self, component: &Value) {
        let id = str_of(component, "id").unwrap_or("?");
        let screen_id = str_of(self.screen, "id").unwrap_or("?");
        if !self.component_ids.insert(id.to_string()) {
            self.errors.push(format!("komponen duplikat: {id}"));
        }
        let declared = object_keys(self.screen, "params");
        let queries: HashSet<&str> = strings(array(self.screen, "queries")).into_iter().collect();
        let here = format!("{screen_id}/{id}");

        if let Some(props) = component.get("props").and_then(Value::as_object) {
            for value in props.values() {
                check_value(self.errors, &here, &declared, &queries, value);
            }
        }

        for event in array(component, "events") {
            let Some(action) = event.get("action") else {
                continue;
            };
            if let Some(inputs) = action.get("inputs").and_then(Value::as_object) {
                for value in inputs.values() {
                    check_value(self.errors, &here, &declared, &queries, value);
                }
            }
            for key in ["on_success", "on_failure"] {
                let Some(nav) = action.get(key) else {
                    continue;
                };
                let target = str_of(nav, "screen").unwrap_or("");
                if !self.screen_map.contains_key(target) {
                    self.errors
                        .push(format!("{here}: navigasi ke layar tidak ada: {target}"));
                }
                if let Some(params) = nav.get("params").and_then(Value::as_object) {
                    for value in params.values() {
                        check_value(self.errors, &here, &declared, &queries, value);
                    }
                }
            }
        }

        for child in array(component, "children") {
            self.component(child);
        }
    }
}

fn check_value(
    errors: &mut Vec<String>,
    here: &str,
    declared: &HashSet<&str>,
    queries: &HashSet<&str>,
    value: &Value,
) {
    let Some(object) = value.as_object() else {
        return;
    };
    if let Some(name) = object.get("param").and_then(Value::as_str) {
        if !declared.contains(name) {
            errors.push(format!("{here}: param {name} tidak ada di layar"));
        }
    }
    if let Some(name) = object.get("query").and_then(Value::as_str) {
        if !queries.contains(name) {
            errors.push(format!("{here}: query {name} tidak ada di layar"));
        }
    }
}

fn object_keys<'a>(value: &'a Value, key: &str) -> HashSet<&'a str> {
    value
        .get(key)
        .and_then(Value::as_object)
        .map(|o| o.keys().map(String::as_str).collect())
        .unwrap_or_default()
}

/// Names after `:` in a route, where each name is `[a-z][a-z0-9_]*`.
fn route_params(route: &str) -> Vec<String> {
    let mut out = Vec::new();
    for (i, _) in route.match_indices(':') {
        let name: String = route[i + 1..]
            .chars()
            .take_while(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || *c == '_')
            .collect();
        if name.chars().next().is_some_and(|c| c.is_ascii_lowercase()) {
            out.push(name);
        }
    }
    out
}
