//! Renderer web (ADR 0011) dengan Dioxus. Hanya dikompilasi dengan fitur `web`.
//!
//! Layar ini read-only: menampilkan modul, alur data, dan masalah validasi lokal dari
//! `features::flow`. Editing dan pengambilan dokumen dari server belum ada.

use crate::features::flow::model::{
    edges, modules_from_flow_json, validate, LocalIssue, ModuleView,
};
use dioxus::prelude::*;

/// Menjalankan aplikasi web. Dipanggil dari titik masuk biner (lihat `examples/web.rs`).
pub fn run() {
    dioxus::launch(App);
}

#[component]
fn App() -> Element {
    // Dokumen flow-ir/1 nyata, disematkan saat compile (tanpa jaringan, sesuai aturan offline).
    let loaded = modules_from_flow_json(include_str!("kirim-faktur.json"));
    rsx! {
        style { {STYLE} }
        match loaded {
            Ok((name, modules)) => rsx! { FlowView { name, modules } },
            Err(message) => rsx! { main { h1 { "Gagal memuat flow" } p { "{message}" } } },
        }
    }
}

/// Gaya dasar: tema terang dan gelap, lebar baca yang nyaman, dan kartu per bagian.
const STYLE: &str = r#"
:root { color-scheme: light dark; --bg: #f7f7f8; --fg: #1d1d1f; --muted: #6b6b73;
  --card: #ffffff; --line: #e3e3e8; --accent: #2f6fdb; }
@media (prefers-color-scheme: dark) {
  :root:not([data-theme="light"]) { --bg: #161618; --fg: #ececf1; --muted: #9a9aa3;
    --card: #1f1f23; --line: #2e2e35; --accent: #6ea3ff; }
}
:root[data-theme="dark"] { --bg: #161618; --fg: #ececf1; --muted: #9a9aa3;
  --card: #1f1f23; --line: #2e2e35; --accent: #6ea3ff; }
body { margin: 0; background: var(--bg); color: var(--fg);
  font: 16px/1.5 system-ui, -apple-system, "Segoe UI", sans-serif; }
main { max-width: 760px; margin: 0 auto; padding: 24px 16px; }
h1 { font-size: 1.6rem; margin: 0 0 16px; }
h2 { font-size: 1rem; margin: 0 0 12px; color: var(--muted);
  text-transform: uppercase; letter-spacing: .04em; }
section { background: var(--card); border: 1px solid var(--line); border-radius: 10px;
  padding: 16px 18px; margin-bottom: 16px; }
ul { margin: 0; padding-left: 20px; }
li { margin: 6px 0; }
code { font: 14px ui-monospace, Consolas, monospace; color: var(--accent); }
p { margin: 0; }
"#;

#[derive(Props, Clone, PartialEq)]
struct FlowViewProps {
    name: String,
    modules: Vec<ModuleView>,
}

#[component]
fn FlowView(props: FlowViewProps) -> Element {
    let edge_list = edges(&props.modules);
    let issues = validate(&props.modules);
    rsx! {
        main {
            h1 { "{props.name}" }
            section {
                h2 { "Modul" }
                ul {
                    for module in props.modules.iter() {
                        li { key: "{module.id}",
                            strong { "{module.id}" }
                            " "
                            code { "{module.kind}" }
                            if !module.inputs.is_empty() {
                                ul {
                                    for (key, source) in module.inputs.iter() {
                                        li { key: "{key}", "{key} = " code { "{source}" } }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            section {
                h2 { "Alur data" }
                ul {
                    for edge in edge_list.iter() {
                        li { key: "{edge.from}-{edge.to}", "{edge.from} → {edge.to}" }
                    }
                }
            }
            section {
                h2 { "Masalah" }
                if issues.is_empty() {
                    p { "Tidak ada masalah lokal." }
                } else {
                    ul {
                        for (index, issue) in issues.iter().enumerate() {
                            li { key: "{index}", "{describe(issue)}" }
                        }
                    }
                }
            }
        }
    }
}

fn describe(issue: &LocalIssue) -> String {
    match issue {
        LocalIssue::EmptyId => "modul tanpa id".to_owned(),
        LocalIssue::DuplicateId(id) => format!("id modul ganda: {id}"),
        LocalIssue::UnknownSource { module, source } => {
            format!("modul {module} merujuk ke sumber tidak dikenal: {source}")
        }
    }
}
