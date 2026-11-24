//! Layar dan pencocokan rute (ADR 0006). Pure, tanpa I/O.
//!
//! Rute memakai segmen `:nama` untuk parameter, misalnya `/customers/:customer_id`.

use std::collections::BTreeMap;

use serde_json::Value;

/// Parameter rute hasil pencocokan, dengan nilai berupa string.
pub type RouteParams = BTreeMap<String, String>;

/// Satu layar dari dokumen.
#[derive(Debug, Clone, PartialEq)]
pub struct Screen<'a> {
    pub id: &'a str,
    pub route: &'a str,
    /// Nama parameter yang dideklarasikan di `params` layar, terurut.
    pub params: Vec<&'a str>,
    pub value: &'a Value,
}

/// Semua layar dokumen, dalam urutan deklarasi.
#[must_use]
pub fn screens(doc: &Value) -> Vec<Screen<'_>> {
    doc.get("screens")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|value| {
            let id = value.get("id")?.as_str()?;
            let route = value.get("route")?.as_str()?;
            let mut params: Vec<&str> = value
                .get("params")
                .and_then(Value::as_object)
                .into_iter()
                .flat_map(|o| o.keys().map(String::as_str))
                .collect();
            params.sort_unstable();
            Some(Screen {
                id,
                route,
                params,
                value,
            })
        })
        .collect()
}

/// Mencocokkan `path` dengan pola `route`. Mengembalikan parameter bila cocok, `None` bila tidak.
#[must_use]
pub fn match_route(route: &str, path: &str) -> Option<RouteParams> {
    let pattern: Vec<&str> = segments(route);
    let actual: Vec<&str> = segments(path);
    if pattern.len() != actual.len() {
        return None;
    }
    let mut params = RouteParams::new();
    for (expected, given) in pattern.iter().zip(actual.iter()) {
        match expected.strip_prefix(':') {
            Some(name) => {
                if name.is_empty() {
                    return None;
                }
                params.insert(name.to_owned(), (*given).to_owned());
            }
            None => {
                if expected != given {
                    return None;
                }
            }
        }
    }
    Some(params)
}

/// Layar pertama yang rutenya cocok dengan `path`, beserta parameternya.
#[must_use]
pub fn find_by_path<'a>(doc: &'a Value, path: &str) -> Option<(Screen<'a>, RouteParams)> {
    screens(doc)
        .into_iter()
        .find_map(|screen| match_route(screen.route, path).map(|params| (screen, params)))
}

/// Nama parameter rute, misalnya `customer_id` dari `/customers/:customer_id`.
#[must_use]
pub fn route_param_names(route: &str) -> Vec<String> {
    segments(route)
        .into_iter()
        .filter_map(|s| s.strip_prefix(':').map(str::to_owned))
        .collect()
}

fn segments(path: &str) -> Vec<&str> {
    path.trim_matches('/')
        .split('/')
        .filter(|s| !s.is_empty())
        .collect()
}

#[cfg(test)]
#[path = "screens_test.rs"]
mod screens_test;
