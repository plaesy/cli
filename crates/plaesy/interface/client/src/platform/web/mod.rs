//! Renderer web (ADR 0011) dengan Dioxus. Hanya dikompilasi dengan fitur `web`.
//!
//! Layar ini read-only: menampilkan modul, alur data, dan masalah validasi lokal dari
//! `features::flow`. Editing dan pengambilan dokumen dari server belum ada.

use crate::features::flow::model::{edges, validate, LocalIssue, ModuleView};
use dioxus::prelude::*;

/// Titik masuk web. Dipanggil dari `main` aplikasi pemanggil.
pub fn run() {
    dioxus::launch(App);
}

#[component]
fn App() -> Element {
    let modules = sample_modules();
    rsx! {
        FlowView { name: "contoh-flow".to_owned(), modules }
    }
}

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

/// Contoh modul untuk ditampilkan sampai layar memuat dokumen dari server.
fn sample_modules() -> Vec<ModuleView> {
    vec![
        ModuleView {
            id: "m1".to_owned(),
            kind: "action".to_owned(),
            inputs: vec![("id".to_owned(), "flow.input.id".to_owned())],
        },
        ModuleView {
            id: "m2".to_owned(),
            kind: "pass".to_owned(),
            inputs: vec![("total".to_owned(), "m1.amount".to_owned())],
        },
    ]
}
