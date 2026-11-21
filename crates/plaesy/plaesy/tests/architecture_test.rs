//! Architecture checks from ADR 0003 (acceptance criteria 2 and 3). They read the resolved
//! dependency graph from `cargo metadata`, so they run with `cargo test --workspace`.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::process::Command;

use serde_json::Value;

/// Crates that perform I/O or network access. A `plaesy-domain-*` crate must not reach them.
const IO_CRATES: &[&str] = &[
    "tokio", "axum", "rusqlite", "ureq", "notify", "ort", "usearch", "keyring", "reqwest", "hyper",
];

fn metadata() -> Value {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_owned());
    let output = Command::new(cargo)
        .args(["metadata", "--format-version", "1", "--locked"])
        .output()
        .expect("run cargo metadata");
    assert!(
        output.status.success(),
        "cargo metadata failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("parse cargo metadata")
}

/// Package name and manifest path for each workspace member, keyed by package id.
fn members(meta: &Value) -> BTreeMap<String, (String, String)> {
    let member_ids: BTreeSet<&str> = meta["workspace_members"]
        .as_array()
        .expect("workspace_members")
        .iter()
        .filter_map(Value::as_str)
        .collect();
    meta["packages"]
        .as_array()
        .expect("packages")
        .iter()
        .filter(|p| member_ids.contains(p["id"].as_str().unwrap_or_default()))
        .map(|p| {
            (
                p["id"].as_str().unwrap_or_default().to_owned(),
                (
                    p["name"].as_str().unwrap_or_default().to_owned(),
                    p["manifest_path"].as_str().unwrap_or_default().to_owned(),
                ),
            )
        })
        .collect()
}

/// Names of every package reachable from `start` through normal (non-dev, non-build) edges.
fn normal_closure(meta: &Value, start: &str) -> BTreeSet<String> {
    let names: BTreeMap<&str, &str> = meta["packages"]
        .as_array()
        .expect("packages")
        .iter()
        .filter_map(|p| Some((p["id"].as_str()?, p["name"].as_str()?)))
        .collect();
    let edges: BTreeMap<&str, Vec<&str>> = meta["resolve"]["nodes"]
        .as_array()
        .expect("resolve nodes")
        .iter()
        .map(|node| {
            let id = node["id"].as_str().unwrap_or_default();
            let normal = node["deps"]
                .as_array()
                .into_iter()
                .flatten()
                .filter(|d| {
                    d["dep_kinds"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .any(|k| k["kind"].is_null())
                })
                .filter_map(|d| d["pkg"].as_str())
                .collect();
            (id, normal)
        })
        .collect();

    let mut seen = BTreeSet::new();
    let mut stack = vec![start];
    while let Some(id) = stack.pop() {
        for next in edges.get(id).into_iter().flatten() {
            if seen.insert(*next) {
                stack.push(next);
            }
        }
    }
    seen.iter()
        .filter_map(|id| names.get(id).map(|n| (*n).to_owned()))
        .collect()
}

#[test]
fn workspace_members_use_plaesy_prefix_and_live_under_crates_plaesy() {
    let meta = metadata();
    let mut problems = Vec::new();
    for (_, (name, manifest)) in members(&meta) {
        let normalized = manifest.replace('\\', "/");
        // Vendored path dependencies (`vendor/`) keep their upstream names by design.
        if normalized.contains("/vendor/") {
            continue;
        }
        if !name.starts_with("plaesy") {
            problems.push(format!("{name}: name must start with `plaesy`"));
        }
        if !normalized.contains("/crates/plaesy/") {
            problems.push(format!(
                "{name}: manifest is outside crates/plaesy/ ({normalized})"
            ));
        }
    }
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

#[test]
fn domain_crates_do_not_depend_on_io_crates() {
    let meta = metadata();
    let mut problems = Vec::new();
    let mut checked = 0;
    for (id, (name, _)) in members(&meta) {
        if !name.starts_with("plaesy-domain-") {
            continue;
        }
        checked += 1;
        let closure = normal_closure(&meta, &id);
        for io in IO_CRATES {
            if closure.contains(*io) {
                problems.push(format!("{name} reaches `{io}` through normal dependencies"));
            }
        }
    }
    assert!(checked > 0, "no plaesy-domain-* crates found");
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

#[test]
fn root_manifest_is_a_virtual_workspace() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../Cargo.toml");
    let text = std::fs::read_to_string(&root).expect("read root Cargo.toml");
    assert!(
        text.contains("[workspace]"),
        "root must declare [workspace]"
    );
    assert!(
        !text.lines().any(|l| l.trim() == "[package]"),
        "root Cargo.toml must be a virtual manifest (no [package])"
    );
}
