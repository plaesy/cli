//! Navigasi antar layar (ADR 0006). Pure, tanpa I/O.
//!
//! Navigasi berasal dari `on_success` dan `on_failure` pada aksi event. Graf ini dipakai
//! untuk memeriksa tujuan navigasi dan mencari layar yang tidak terjangkau dari layar awal.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use serde_json::Value;

use crate::screens::route_param_names;

/// Satu tepi navigasi: dari layar `from` ke layar `to`, dengan nama parameter yang dikirim.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NavTarget {
    pub from: String,
    pub to: String,
    pub params: BTreeSet<String>,
}

/// Semua navigasi dari aksi event di setiap komponen layar.
#[must_use]
pub fn navigation_targets(doc: &Value) -> Vec<NavTarget> {
    let mut out = Vec::new();
    for screen in doc
        .get("screens")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let Some(from) = screen.get("id").and_then(Value::as_str) else {
            continue;
        };
        for component in components(screen.get("root")) {
            for event in component
                .get("events")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
            {
                let Some(action) = event.get("action") else {
                    continue;
                };
                for key in ["on_success", "on_failure"] {
                    let Some(nav) = action.get(key) else { continue };
                    let Some(to) = nav.get("screen").and_then(Value::as_str) else {
                        continue;
                    };
                    let params = nav
                        .get("params")
                        .and_then(Value::as_object)
                        .into_iter()
                        .flat_map(|o| o.keys().cloned())
                        .collect();
                    out.push(NavTarget {
                        from: from.to_owned(),
                        to: to.to_owned(),
                        params,
                    });
                }
            }
        }
    }
    out
}

/// Layar yang terjangkau dari `start_screen` lewat navigasi.
#[must_use]
pub fn reachable(doc: &Value) -> BTreeSet<String> {
    let edges = adjacency(doc);
    let Some(start) = doc.get("start_screen").and_then(Value::as_str) else {
        return BTreeSet::new();
    };
    let mut seen = BTreeSet::new();
    let mut queue = VecDeque::from([start.to_owned()]);
    while let Some(current) = queue.pop_front() {
        if !seen.insert(current.clone()) {
            continue;
        }
        for next in edges.get(&current).into_iter().flatten() {
            queue.push_back(next.clone());
        }
    }
    seen
}

/// Layar yang tidak terjangkau dari layar awal, terurut.
#[must_use]
pub fn unreachable(doc: &Value) -> Vec<String> {
    let reach = reachable(doc);
    doc.get("screens")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|s| s.get("id").and_then(Value::as_str))
        .filter(|id| !reach.contains(*id))
        .map(str::to_owned)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

/// Galat tujuan navigasi: layar tujuan harus ada, parameter yang dikirim harus dideklarasikan
/// di layar tujuan, dan setiap parameter rute tujuan harus dikirim.
#[must_use]
pub fn validate_navigation(doc: &Value) -> Vec<String> {
    let mut errors = Vec::new();
    let declared: BTreeMap<&str, (BTreeSet<String>, String)> = doc
        .get("screens")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|s| {
            let id = s.get("id")?.as_str()?;
            let route = s.get("route")?.as_str()?.to_owned();
            let params = s
                .get("params")
                .and_then(Value::as_object)
                .into_iter()
                .flat_map(|o| o.keys().cloned())
                .collect();
            Some((id, (params, route)))
        })
        .collect();

    if let Some(start) = doc.get("start_screen").and_then(Value::as_str) {
        if !declared.contains_key(start) {
            errors.push(format!(
                "start_screen merujuk layar yang tidak ada: {start}"
            ));
        }
    }

    for target in navigation_targets(doc) {
        let Some((target_params, route)) = declared.get(target.to.as_str()) else {
            errors.push(format!(
                "navigasi dari {} ke layar yang tidak ada: {}",
                target.from, target.to
            ));
            continue;
        };
        for param in &target.params {
            if !target_params.contains(param) {
                errors.push(format!(
                    "navigasi dari {} ke {}: parameter {param} tidak dideklarasikan",
                    target.from, target.to
                ));
            }
        }
        for required in route_param_names(route) {
            if !target.params.contains(&required) {
                errors.push(format!(
                    "navigasi dari {} ke {}: parameter rute {required} tidak dikirim",
                    target.from, target.to
                ));
            }
        }
    }
    errors
}

fn adjacency(doc: &Value) -> BTreeMap<String, BTreeSet<String>> {
    let mut edges: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for target in navigation_targets(doc) {
        edges.entry(target.from).or_default().insert(target.to);
    }
    edges
}

/// Semua komponen dalam pohon `root`, termasuk anak-anaknya.
pub(crate) fn components(root: Option<&Value>) -> Vec<&Value> {
    let mut out = Vec::new();
    let mut stack: Vec<&Value> = root.into_iter().collect();
    while let Some(node) = stack.pop() {
        out.push(node);
        for child in node
            .get("children")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            stack.push(child);
        }
    }
    out
}

#[cfg(test)]
#[path = "navigation_test.rs"]
mod navigation_test;
