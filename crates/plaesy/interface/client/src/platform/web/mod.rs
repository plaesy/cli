//! Renderer web untuk klien Plaesy.
//!
//! Di sini hanya glue browser dan state Dioxus: shell (navigasi dan guard), daftar flow, dan
//! editor. Aturan editor ada di `features::flow` (reducer, validasi, dokumen, inspect) dan
//! penyimpanan sesi di `features::store`.

use crate::adapter::http::document_path;
use crate::features::data::document::{
    document_name, load_request as data_load_request, save_request as data_save_request,
    tables_from_document, DATA_KIND,
};
use crate::features::data::model::{
    validate as validate_data, LocalIssue as DataIssue, RelationView, TableView,
};
use crate::features::flow::changes::{describe_change, document_changes};
use crate::features::flow::document::patch_document;
use crate::features::flow::inspect::{connect_candidates, flagged_modules, source_expression};
use crate::features::flow::layout::{layout, Position};
use crate::features::flow::model::{
    edges, modules_from_flow_json, next_module_id, validate, LocalIssue, ModuleView,
};
use crate::features::flow::reducer::{apply as apply_action, selection_after, Action, ActionError};
use crate::features::flow::services::{self, load_request, FLOW_KIND};
use crate::features::store::MemoryStore;
use crate::features::ApiClient;
use crate::shared::canvas::viewport::Viewport;
use crate::shell::{Guard, Page};
use dioxus::prelude::*;
use serde_json::Value;
use std::collections::BTreeMap;
use std::rc::Rc;

const FIXTURE: &str = include_str!("kirim-faktur.json");

pub fn run() {
    dioxus::launch(App);
}

#[component]
fn App() -> Element {
    // Penyimpanan sesi diisi dengan dokumen contoh sekali saat aplikasi dimuat.
    let store = use_hook(|| Rc::new(seeded_store()));
    let mut page = use_signal(|| Page::FlowEditor(seed_flow_id()));
    let guard = Guard::new(&["flow:read", "data:read", "documents:read"]);
    let current = page();
    let content = if guard.allows(&current) {
        match current {
            Page::FlowList => rsx! {
                FlowList {
                    store: store.clone(),
                    on_open: move |id: String| page.set(Page::FlowEditor(id)),
                }
            },
            Page::FlowEditor(id) => {
                let key_text = id.clone();
                rsx! {
                    FlowEditor { key: "{key_text}", store: store.clone(), flow_id: id }
                }
            }
            Page::DataList => rsx! {
                DataList {
                    store: store.clone(),
                    on_open: move |id: String| page.set(Page::DataEditor(id)),
                }
            },
            Page::DataEditor(id) => {
                let key_text = id.clone();
                rsx! {
                    DataEditor { key: "{key_text}", store: store.clone(), doc_id: id }
                }
            }
            Page::Document { .. } | Page::NotFound => rsx! {
                main { class: "error-page", h1 { "Halaman tidak ditemukan" } }
            },
        }
    } else {
        rsx! { main { class: "error-page", h1 { "Akses ditolak" } } }
    };

    rsx! { style { {STYLE} } style { {EXTRA_STYLE} }
        div { class: "app-shell",
            header { class: "topbar",
                div { class: "brand-lockup", span { class: "brand-mark", "aria-hidden": "true" }, strong { "Plaesy" } }
                span { class: "topbar-divider", "aria-hidden": "true" }
                nav { class: "nav-tabs", "aria-label": "Navigasi utama",
                    button { class: "nav-tab", onclick: move |_| page.set(Page::FlowList), "Flow" }
                    button { class: "nav-tab", onclick: move |_| page.set(Page::DataList), "Data" }
                }
            }
            {content}
        }
    }
}

/// Id flow dari dokumen contoh. Dipakai sebagai halaman pertama.
fn seed_flow_id() -> String {
    serde_json::from_str::<Value>(FIXTURE)
        .ok()
        .and_then(|doc| doc.get("id").and_then(Value::as_str).map(str::to_owned))
        .unwrap_or_else(|| "flw_seed".to_owned())
}

fn seeded_store() -> MemoryStore {
    let store = MemoryStore::default();
    let _ = services::save(&store, &seed_flow_id(), FIXTURE);
    let _ = store.send(data_save_request(&seed_data_id(), DATA_FIXTURE));
    store
}

/// Id dokumen data contoh (`crm.json`).
fn seed_data_id() -> String {
    serde_json::from_str::<Value>(DATA_FIXTURE)
        .ok()
        .and_then(|doc| doc.get("id").and_then(Value::as_str).map(str::to_owned))
        .unwrap_or_else(|| "dat_seed".to_owned())
}

#[component]
fn FlowList(store: Rc<MemoryStore>, on_open: EventHandler<String>) -> Element {
    let prefix = document_path(FLOW_KIND, "");
    let flows: Vec<(String, String)> = store
        .paths_with_prefix(&prefix)
        .iter()
        .filter_map(|path| {
            let id = path.strip_prefix(prefix.as_str())?.to_owned();
            let text = store.send(load_request(&id)).ok()?;
            let name = modules_from_flow_json(&text).map_or_else(|_| id.clone(), |(name, _)| name);
            Some((id, name))
        })
        .collect();
    let count = flows.len();
    rsx! {
        main { class: "canvas-panel list-page",
            div { class: "canvas-header", div { h1 { "Flow" }, p { "{count} flow tersimpan di sesi ini" } } }
            div { class: "list-body",
                if flows.is_empty() {
                    p { class: "empty-inspector", "Belum ada flow." }
                } else {
                    ul { class: "flow-list", "aria-label": "Daftar flow",
                        for (id, name) in flows {
                            li {
                                button { class: "palette-item", onclick: move |_| on_open.call(id.clone()),
                                    span { "{name}" }
                                    small { class: "library-path", "{id}" }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn FlowEditor(store: Rc<MemoryStore>, flow_id: String) -> Element {
    let loaded = use_hook(|| load_flow(&store, &flow_id));
    match loaded {
        Ok(flow) => rsx! {
            FlowWorkspace { store: store.clone(), flow_id: flow_id.clone(), flow }
        },
        Err(message) => rsx! {
            main { class: "error-page", h1 { "Gagal memuat flow" }, p { "{message}" } }
        },
    }
}

const DATA_FIXTURE: &str = include_str!("crm.json");

#[component]
fn DataList(store: Rc<MemoryStore>, on_open: EventHandler<String>) -> Element {
    let prefix = document_path(DATA_KIND, "");
    let documents: Vec<(String, String)> = store
        .paths_with_prefix(&prefix)
        .iter()
        .filter_map(|path| {
            let id = path.strip_prefix(prefix.as_str())?.to_owned();
            let text = store.send(data_load_request(&id)).ok()?;
            let doc: Value = serde_json::from_str(&text).ok()?;
            let (_, name) = document_name(&doc)?;
            Some((id, name))
        })
        .collect();
    let count = documents.len();
    rsx! {
        main { class: "canvas-panel list-page",
            div { class: "canvas-header", div { h1 { "Data" }, p { "{count} dokumen data tersimpan di sesi ini · baca saja" } } }
            div { class: "list-body",
                if documents.is_empty() {
                    p { class: "empty-inspector", "Belum ada dokumen data." }
                } else {
                    ul { class: "flow-list", "aria-label": "Daftar dokumen data",
                        for (id, name) in documents {
                            li {
                                button { class: "palette-item", onclick: move |_| on_open.call(id.clone()),
                                    span { "{name}" }
                                    small { class: "library-path", "{id}" }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn DataEditor(store: Rc<MemoryStore>, doc_id: String) -> Element {
    let loaded = use_hook(|| load_data(&store, &doc_id));
    match loaded {
        Ok(view) => rsx! { DataDocumentView { view } },
        Err(message) => rsx! {
            main { class: "error-page", h1 { "Gagal memuat data" }, p { "{message}" } }
        },
    }
}

/// Dokumen data yang sudah dibaca: tabel, relasi, dan masalah validasi lokal.
#[derive(Debug, Clone, PartialEq)]
struct LoadedData {
    name: String,
    tables: Vec<TableView>,
    relations: Vec<RelationView>,
    issues: Vec<String>,
}

fn load_data(store: &MemoryStore, doc_id: &str) -> Result<LoadedData, String> {
    let text = store.send(data_load_request(doc_id))?;
    let doc: Value = serde_json::from_str(&text).map_err(|error| error.to_string())?;
    let (tables, relations) = tables_from_document(&doc)?;
    let name = document_name(&doc).map_or_else(|| doc_id.to_owned(), |(_, name)| name);
    let issues = validate_data(&tables, &relations)
        .iter()
        .map(describe_data_issue)
        .collect();
    Ok(LoadedData {
        name,
        tables,
        relations,
        issues,
    })
}

#[component]
fn DataDocumentView(view: LoadedData) -> Element {
    let table_count = view.tables.len();
    let relation_count = view.relations.len();
    let relation_lines: Vec<String> = view.relations.iter().map(relation_line).collect();
    rsx! {
        main { class: "canvas-panel data-page",
            div { class: "canvas-header",
                div { h1 { "{view.name}" }, p { "{table_count} tabel · {relation_count} relasi · baca saja" } }
            }
            div { class: "list-body data-body",
                for table in view.tables.iter() {
                    {table_card(table)}
                }
                div { class: "data-side",
                    h3 { "Relasi" }
                    if relation_lines.is_empty() {
                        p { class: "empty-inspector", "Tidak ada relasi." }
                    } else {
                        ul { class: "edge-list",
                            for line in relation_lines.iter() {
                                li { "{line}" }
                            }
                        }
                    }
                    h3 { "Masalah" }
                    if view.issues.is_empty() {
                        p { class: "empty-inspector", "Tidak ada masalah lokal." }
                    } else {
                        ul { class: "edge-list",
                            for issue in view.issues.iter() {
                                li { "{issue}" }
                            }
                        }
                    }
                }
            }
        }
    }
}

fn table_card(table: &TableView) -> Element {
    let name = table.name.clone();
    let key = if table.primary_key.is_empty() {
        "tanpa kunci utama".to_owned()
    } else {
        table.primary_key.join(", ")
    };
    let columns: Vec<String> = table
        .columns
        .iter()
        .map(|c| {
            let nullable = if c.nullable { " · nullable" } else { "" };
            format!("{} · {}{nullable}", c.name, c.data_type)
        })
        .collect();
    rsx! {
        article { class: "data-table",
            h2 { "{name}" }
            p { class: "library-path", "Kunci utama: {key}" }
            ul { class: "edge-list",
                for line in columns.iter() {
                    li { "{line}" }
                }
            }
        }
    }
}

fn relation_line(relation: &RelationView) -> String {
    let (parent, parent_cols) = &relation.parent;
    let (child, child_cols) = &relation.child;
    format!(
        "{}: {parent}({}) → {child}({})",
        relation.id,
        parent_cols.join(", "),
        child_cols.join(", ")
    )
}

fn describe_data_issue(issue: &DataIssue) -> String {
    match issue {
        DataIssue::DuplicateTable(table) => format!("Tabel {table} dobel"),
        DataIssue::DuplicateColumn { table, column } => {
            format!("Kolom {column} dobel di {table}")
        }
        DataIssue::UnknownPrimaryKey { table, column } => {
            format!("Kunci utama {column} di {table} tidak ada")
        }
        DataIssue::UnknownRelationTable(table) => {
            format!("Relasi merujuk tabel {table} yang tidak ada")
        }
    }
}

/// Dokumen flow yang sudah dibaca dari penyimpanan.
#[derive(Debug, Clone, PartialEq)]
struct LoadedFlow {
    name: String,
    original: Value,
    modules: Vec<ModuleView>,
}

fn load_flow(store: &MemoryStore, flow_id: &str) -> Result<LoadedFlow, String> {
    let text = store.send(load_request(flow_id))?;
    let original: Value = serde_json::from_str(&text).map_err(|error| error.to_string())?;
    let (name, modules) = modules_from_flow_json(&text)?;
    Ok(LoadedFlow {
        name,
        original,
        modules,
    })
}

/// Keadaan editor. Setiap field adalah signal sehingga struct ini bisa disalin ke handler.
#[derive(Clone, Copy)]
struct Editor {
    modules: Signal<Vec<ModuleView>>,
    original: Signal<Value>,
    selected: Signal<Option<String>>,
    viewport: Signal<Viewport>,
    history: Signal<Vec<Vec<ModuleView>>>,
    redo: Signal<Vec<Vec<ModuleView>>>,
    status: Signal<String>,
    connect_input: Signal<String>,
    connect_module: Signal<String>,
    connect_path: Signal<String>,
}

impl Editor {
    fn apply(mut self, action: Action) {
        let current = (self.modules)();
        match apply_action(&current, &action) {
            Ok(next) => {
                let selection = selection_after((self.selected)().as_deref(), &action);
                self.history.write().push(current);
                self.redo.write().clear();
                self.modules.set(next);
                self.selected.set(selection);
                self.status.set(action_message(&action));
            }
            Err(error) => self.status.set(error_message(&error)),
        }
    }

    fn add_module(self, kind: &str) {
        let id = next_module_id(&(self.modules)());
        self.apply(Action::AddModule(ModuleView {
            id,
            kind: kind.to_owned(),
            operation: String::new(),
            inputs: Vec::new(),
        }));
    }

    fn select(mut self, id: String) {
        self.status.set(format!("Dipilih {id}"));
        self.selected.set(Some(id));
    }

    fn set_connect_input(mut self, value: String) {
        self.connect_input.set(value);
    }

    fn set_connect_module(mut self, value: String) {
        self.connect_module.set(value);
    }

    fn set_connect_path(mut self, value: String) {
        self.connect_path.set(value);
    }

    /// Menyambung input `to` ke `mSUMBER.path`. Memakai isian dari inspector.
    fn connect(mut self, to: &str) {
        let input = (self.connect_input)().trim().to_owned();
        if input.is_empty() {
            self.status
                .set("Isi nama input yang akan disambung".to_owned());
            return;
        }
        let Some(source) = source_expression(&(self.connect_module)(), &(self.connect_path)())
        else {
            self.status
                .set("Pilih modul sumber dan isi path-nya".to_owned());
            return;
        };
        self.apply(Action::Connect {
            to: to.to_owned(),
            input,
            source,
        });
    }

    // Guard `write()` diambil ke `let` dulu. Di dalam `match` ia hidup sampai akhir arm, dan
    // `self.restore` memakai `self` saat masih dipinjam (E0503).
    fn undo_step(mut self) {
        let previous = self.history.write().pop();
        match previous {
            Some(snapshot) => {
                let current = (self.modules)();
                self.redo.write().push(current);
                self.restore(snapshot, "Undo");
            }
            None => self.status.set("Tidak ada yang bisa di-undo".to_owned()),
        }
    }

    fn redo_step(mut self) {
        let next = self.redo.write().pop();
        match next {
            Some(snapshot) => {
                let current = (self.modules)();
                self.history.write().push(current);
                self.restore(snapshot, "Redo");
            }
            None => self.status.set("Tidak ada yang bisa di-redo".to_owned()),
        }
    }

    fn restore(mut self, snapshot: Vec<ModuleView>, label: &str) {
        let selection = (self.selected)().filter(|id| snapshot.iter().any(|m| &m.id == id));
        self.modules.set(snapshot);
        self.selected.set(selection);
        self.status.set(format!("{label} berhasil"));
    }

    fn zoom_by(mut self, factor: f64) {
        self.viewport.write().zoom_at(0.0, 0.0, factor);
    }

    fn check(mut self) {
        let issues = validate(&(self.modules)());
        let message = if issues.is_empty() {
            "Validasi lolos: tidak ada masalah lokal"
        } else {
            "Validasi menemukan masalah, lihat panel Masalah"
        };
        self.status.set(message.to_owned());
    }

    fn save(mut self, store: &MemoryStore, flow_id: &str) {
        let document = patch_document(&(self.original)(), &(self.modules)());
        let body = document.to_string();
        match services::save(store, flow_id, &body) {
            Ok(_) => {
                self.original.set(document);
                self.status.set(
                    "Tersimpan di penyimpanan sesi (hilang saat halaman dimuat ulang)".to_owned(),
                );
            }
            Err(error) => self.status.set(format!("Gagal menyimpan: {error}")),
        }
    }
}

#[component]
fn FlowWorkspace(store: Rc<MemoryStore>, flow_id: String, flow: LoadedFlow) -> Element {
    let editor = Editor {
        modules: use_signal(|| flow.modules.clone()),
        original: use_signal(|| flow.original.clone()),
        selected: use_signal(|| flow.modules.first().map(|m| m.id.clone())),
        viewport: use_signal(Viewport::default),
        history: use_signal(Vec::new),
        redo: use_signal(Vec::new),
        status: use_signal(|| "Perubahan baru tersimpan setelah Save draft".to_owned()),
        connect_input: use_signal(String::new),
        connect_module: use_signal(String::new),
        connect_path: use_signal(String::new),
    };

    let modules = (editor.modules)();
    let selected = (editor.selected)();
    let viewport = (editor.viewport)();
    let status = (editor.status)();
    let edge_list = edges(&modules);
    let issues = validate(&modules);
    let flagged = flagged_modules(&issues);
    let change_lines: Vec<String> = document_changes(&(editor.original)(), &modules)
        .iter()
        .map(describe_change)
        .collect();
    let ids: Vec<String> = modules.iter().map(|m| m.id.clone()).collect();
    let edge_pairs: Vec<(String, String)> = edge_list
        .iter()
        .map(|e| (e.from.clone(), e.to.clone()))
        .collect();
    let positions = layout(&ids, &edge_pairs).unwrap_or_else(|_| fallback_positions(&ids));
    let selected_module = selected
        .as_deref()
        .and_then(|id| modules.iter().find(|m| m.id == id));
    let candidates = selected
        .as_deref()
        .map(|id| connect_candidates(&modules, id))
        .unwrap_or_default();
    let zoom_percent = (viewport.zoom * 100.0).round() as i64;
    let stage_style = format!(
        "transform:translate({}px,{}px) scale({});transform-origin:top left;",
        viewport.pan_x, viewport.pan_y, viewport.zoom
    );
    let module_count = modules.len();
    let edge_count = edge_list.len();
    let title = flow.name.clone();
    let save_store = store.clone();
    let save_id = flow_id.clone();

    rsx! { div { class: "workspace",
        aside { class: "palette",
            div { class: "eyebrow", "NODE PALETTE" }
            for (label, kind, color) in [
                ("Action", "action", "#7BA7FF"),
                ("Branch", "branch_one", "#C6A9FF"),
                ("Approval", "approval", "#F7C879"),
                ("For each", "for_each", "#71E6C1"),
                ("Subflow", "subflow", "#F29D9D"),
            ] {
                button { class: "palette-item", onclick: move |_| editor.add_module(kind),
                    span { class: "palette-dot", style: "background:{color}" }
                    span { "{label}" }
                }
            }
            div { class: "palette-separator" }
            div { class: "eyebrow", "FLOW" }
            div { class: "library-name", "{title}" }
            div { class: "library-path", "{flow_id}" }
        }
        main { class: "canvas-panel",
            div { class: "canvas-header",
                div { h1 { "{title}" }, p { "{module_count} modul · {edge_count} alur" } }
                div { class: "top-actions",
                    div { class: "zoom-controls",
                        button { "aria-label": "Zoom out", onclick: move |_| editor.zoom_by(1.0 / 1.1), "−" }
                        span { "{zoom_percent}%" }
                        button { "aria-label": "Zoom in", onclick: move |_| editor.zoom_by(1.1), "+" }
                    }
                    button { class: "button button-save", onclick: move |_| editor.undo_step(), "Undo" }
                    button { class: "button button-save", onclick: move |_| editor.redo_step(), "Redo" }
                    button { class: "button button-validate", onclick: move |_| editor.check(), "Validate" }
                    button { class: "button button-save", onclick: move |_| editor.save(&save_store, &save_id), "Save draft" }
                }
            }
            section { class: "canvas-section", "aria-label": "Modul",
                h2 { "Modul" }
                div { class: "canvas-viewport",
                    div { class: "canvas-stage", style: "{stage_style}",
                        svg { class: "edge-layer", width: "760", height: "620", view_box: "0 0 760 620", "aria-hidden": "true",
                            defs { marker { id: "arrow", marker_width: "8", marker_height: "8", ref_x: "6", ref_y: "3", orient: "auto", marker_units: "strokeWidth", path { d: "M0,0 L0,6 L6,3 z", fill: "#718CA0" } } }
                            for edge in edge_list.iter() { if let (Some(from), Some(to)) = (positions.get(&edge.from), positions.get(&edge.to)) {
                                line { x1: "{node_x(from) + 176}", y1: "{node_y(from) + 44}", x2: "{node_x(to)}", y2: "{node_y(to) + 44}", stroke: "#718CA0", stroke_width: "2", marker_end: "url(#arrow)" }
                            } }
                        }
                        for module in modules.iter() { {module_card(module, positions.get(&module.id), editor, selected.as_deref(), flagged.contains(&module.id))} }
                    }
                }
            }
            section { class: "flow-edges", "aria-label": "Alur",
                h2 { "Alur" }
                if edge_list.is_empty() {
                    p { class: "empty-inspector", "Belum ada alur antar modul." }
                } else {
                    ul { class: "edge-list",
                        for edge in edge_list.iter() {
                            li { "{edge.from} → {edge.to}" }
                        }
                    }
                }
            }
            div { class: "status-line", "{status}" }
        }
        aside { class: "inspector",
            h2 { "Inspector" }
            div { class: "inspector-divider" }
            div { class: "eyebrow", "SELECTED MODULE" }
            if let Some(module) = selected_module {
                {inspector(module, editor, &candidates)}
            } else {
                p { class: "empty-inspector", "Pilih modul untuk melihat operasi dan input." }
            }
            div { class: "changes-panel", "aria-label": "Perubahan belum disimpan",
                h3 { "Perubahan belum disimpan" }
                if change_lines.is_empty() {
                    p { class: "empty-inspector", "Tidak ada perubahan dari dokumen yang tersimpan." }
                } else {
                    ul { class: "edge-list",
                        for line in change_lines.iter() {
                            li { "{line}" }
                        }
                    }
                }
            }
            section { class: "validation-card", "aria-label": "Masalah",
                h2 { "Masalah" }
                if issues.is_empty() {
                    p { "✓ Tidak ada masalah lokal" }
                } else {
                    for issue in issues.iter() { p { "• {describe(issue)}" } }
                }
            }
        }
    } }
}

fn module_card(
    module: &ModuleView,
    position: Option<&Position>,
    editor: Editor,
    selected: Option<&str>,
    flagged: bool,
) -> Element {
    let position = position.copied().unwrap_or(Position { x: 0, y: 0 });
    let module_id = module.id.clone();
    let is_selected = selected == Some(module.id.as_str());
    let class = match (is_selected, flagged) {
        (true, true) => "module-card selected flagged",
        (true, false) => "module-card selected",
        (false, true) => "module-card flagged",
        (false, false) => "module-card",
    };
    let style = format!(
        "left:{}px;top:{}px;border-top-color:{}",
        node_x(&position),
        node_y(&position),
        kind_color(&module.kind)
    );
    let title = operation_label(module);
    let detail = input_summary(module);
    rsx! { button { class: class, style: "{style}", onclick: move |_| editor.select(module_id.clone()),
        span { class: "module-kind", style: "color:{kind_color(&module.kind)}", "{module.id} · {module.kind}" }
        strong { "{title}" }
        small { "{detail}" }
    } }
}

fn inspector(module: &ModuleView, editor: Editor, candidates: &[String]) -> Element {
    let module_id = module.id.clone();
    let rename_from = module_id.clone();
    let remove_id = module_id.clone();
    let connect_to = module_id.clone();
    let operation = operation_label(module);
    let inputs = input_summary(module);
    let connect_input = (editor.connect_input)();
    let connect_path = (editor.connect_path)();
    rsx! {
        div { class: "selected-module",
            span { class: "selected-dot", style: "background:{kind_color(&module.kind)}" }
            strong { "{module.id} · {module.kind}" }
        }
        div { class: "field",
            label { class: "field-label", "for": "module-id", "Id modul" }
            input { id: "module-id", class: "field-input", value: "{module_id}",
                onchange: move |event| editor.apply(Action::RenameModule { from: rename_from.clone(), to: event.value() }),
            }
        }
        Field { label: "Operasi", value: operation }
        Field { label: "Input", value: inputs }
        div { class: "connect-form",
            span { class: "field-label", "Hubungkan input" }
            input { id: "connect-input", class: "field-input", placeholder: "nama input", "aria-label": "Nama input", value: "{connect_input}",
                oninput: move |event| editor.set_connect_input(event.value()),
            }
            select { id: "connect-module", class: "field-input", "aria-label": "Modul sumber",
                onchange: move |event| editor.set_connect_module(event.value()),
                option { value: "", "Pilih modul sumber" }
                for candidate in candidates.iter() {
                    option { value: "{candidate}", "{candidate}" }
                }
            }
            input { id: "connect-path", class: "field-input", placeholder: "path, mis. invoice.id", "aria-label": "Path sumber", value: "{connect_path}",
                oninput: move |event| editor.set_connect_path(event.value()),
            }
            button { class: "button button-save", onclick: move |_| editor.connect(&connect_to), "Sambungkan" }
        }
        button { class: "button button-danger", onclick: move |_| editor.apply(Action::RemoveModule(remove_id.clone())), "Hapus modul" }
    }
}

#[component]
fn Field(label: &'static str, value: String) -> Element {
    rsx! { div { class: "field", span { class: "field-label", "{label}" }, div { class: "field-value", "{value}" } } }
}

fn action_message(action: &Action) -> String {
    match action {
        Action::AddModule(module) => format!("Modul {} ditambahkan", module.id),
        Action::RemoveModule(id) => format!("Modul {id} dihapus"),
        Action::RenameModule { from, to } => format!("Modul {from} diganti menjadi {to}"),
        Action::Connect { to, input, .. } => format!("Input {input} di {to} diubah"),
    }
}

fn error_message(error: &ActionError) -> String {
    match error {
        ActionError::NotFound(id) => format!("Modul {id} tidak ditemukan"),
        ActionError::AlreadyExists(id) => format!("Id {id} sudah dipakai"),
    }
}

fn fallback_positions(ids: &[String]) -> BTreeMap<String, Position> {
    ids.iter()
        .enumerate()
        .map(|(i, id)| {
            (
                id.clone(),
                Position {
                    x: (i % 4) as i64 * 220,
                    y: (i / 4) as i64 * 120,
                },
            )
        })
        .collect()
}
fn node_x(p: &Position) -> i64 {
    28 + p.x * 3 / 4
}
fn node_y(p: &Position) -> i64 {
    52 + p.y * 3 / 5
}
/// Label operasi dari dokumen. Bila dokumen tidak punya `operation`, pakai `kind`.
fn operation_label(m: &ModuleView) -> String {
    if m.operation.is_empty() {
        m.kind.clone()
    } else {
        m.operation.clone()
    }
}
fn input_summary(m: &ModuleView) -> String {
    if m.inputs.is_empty() {
        "Tanpa input".to_owned()
    } else {
        m.inputs
            .iter()
            .map(|(name, source)| format!("{name} ← {source}"))
            .collect::<Vec<_>>()
            .join(", ")
    }
}
fn kind_color(kind: &str) -> &'static str {
    match kind {
        "action" => "#7BA7FF",
        "branch_one" => "#C6A9FF",
        "approval" => "#F7C879",
        "for_each" => "#71E6C1",
        "subflow" => "#F29D9D",
        _ => "#9EB0C1",
    }
}
fn describe(issue: &LocalIssue) -> String {
    match issue {
        LocalIssue::EmptyId => "Module has no id".to_owned(),
        LocalIssue::DuplicateId(id) => format!("Duplicate id: {id}"),
        LocalIssue::UnknownSource { module, source } => {
            format!("{module} references unknown {source}")
        }
    }
}

const EXTRA_STYLE: &str = r#"
.nav-tabs{display:flex;gap:8px}.nav-tab{height:34px;padding:0 14px;border-radius:7px;background:#202E43;font-size:13px}.nav-tab:hover{background:#293A4C}
.list-page .list-body{padding:24px 26px}.flow-list{list-style:none;margin:0;padding:0;display:grid;gap:10px;max-width:520px}.flow-list .palette-item{height:auto;padding:12px 14px;flex-direction:column;align-items:flex-start;gap:4px}
.canvas-section h2{margin:0;padding:14px 26px 0;font-size:12px;letter-spacing:.06em;text-transform:uppercase;color:#9EB0C1;font-weight:600}
.flow-edges{padding:14px 26px 18px;border-top:1px solid #344254}.flow-edges h2{margin:0;font-size:12px;letter-spacing:.06em;text-transform:uppercase;color:#9EB0C1;font-weight:600}
.edge-list{margin:10px 0 0;padding-left:18px;font-size:12px;color:#CFDBEA;line-height:1.7}
.button-danger{background:#4A2A2F;color:#F6C9CE;margin-top:8px}
.field-input{width:100%;height:42px;padding:0 15px;border-radius:6px;background:#233247;color:#E9EEF7;border:1px solid #344254;font:inherit;font-size:13px}
.validation-card h2{margin:0 0 8px;font-size:10px;letter-spacing:.06em;text-transform:uppercase;color:#A9E2D2;font-weight:600}
.module-card.flagged{outline:2px solid #F26D6D;outline-offset:2px}
.connect-form{display:grid;gap:8px;margin:18px 0}
.data-body{display:grid;grid-template-columns:repeat(auto-fill,minmax(280px,1fr));gap:16px;align-items:start}.data-table{padding:16px;border-radius:9px;background:#273646}.data-table h2{margin:0 0 6px;font-size:15px;font-weight:600;color:#E9EEF7}.data-side{grid-column:1/-1}.data-side h3{margin:16px 0 8px;font-size:10px;letter-spacing:.06em;text-transform:uppercase;color:#9EB0C1;font-weight:600}
.changes-panel{margin-top:24px}.changes-panel h3{margin:0 0 8px;font-size:10px;letter-spacing:.06em;text-transform:uppercase;color:#9EB0C1;font-weight:600}
"#;

const STYLE: &str = r#"
:root { color-scheme:dark; font-family:Inter,ui-sans-serif,system-ui,-apple-system,"Segoe UI",sans-serif; background:#111B2B; color:#E9EEF7 } *{box-sizing:border-box} body{margin:0;min-width:980px;background:#111B2B} button{font:inherit;color:inherit;border:0;cursor:pointer}.app-shell{min-height:100vh;background:#111B2B}.topbar{height:72px;display:flex;align-items:center;gap:22px;padding:0 22px;background:#11192A;border-bottom:1px solid #2C3B4E}.brand-lockup{display:flex;align-items:center;gap:12px;font-size:22px;min-width:126px}.brand-mark{width:8px;height:29px;border-radius:4px;background:#69D9BA;display:inline-block}.topbar-divider{width:1px;height:38px;background:#2C3B4E}.breadcrumb{font-size:15px}.version{margin-left:8px;color:#8DA2B7;font-size:11px}.top-actions{margin-left:auto;display:flex;gap:12px;align-items:center}.button,.concept-badge{height:38px;padding:0 20px;border-radius:7px;font-size:13px}.button-validate{background:#27483D;color:#C9F3E0}.button-save{background:#263F60;color:#D3E7FC}.concept-badge{display:flex;align-items:center;background:#343248;color:#DCCCF7;font-size:11px;letter-spacing:.06em}.workspace{display:grid;grid-template-columns:226px minmax(600px,1fr) 307px;min-height:calc(100vh - 72px)}.palette,.inspector{background:#172438;padding:30px 20px}.palette{border-right:1px solid #2E3D4F}.inspector{border-left:1px solid #2E3D4F;padding:28px 24px}.eyebrow{color:#7D92A7;font-size:10px;letter-spacing:.08em;margin-bottom:16px}.palette-item{width:100%;height:51px;display:flex;align-items:center;gap:11px;padding:0 14px;margin-bottom:13px;border-radius:8px;background:#202E43;text-align:left;font-size:14px}.palette-item:hover{background:#293A4C}.palette-dot,.selected-dot{width:10px;height:10px;border-radius:3px;display:inline-block;flex:none}.palette-separator,.inspector-divider{height:1px;background:#293A4C;margin:22px 0}.library-name{font-size:13px;margin-top:28px}.library-path,.palette-note{color:#8DA3B9;font-size:11px}.palette-note{margin-top:45px;line-height:1.45}.canvas-panel{background:#202A37;min-width:0}.canvas-header{height:92px;display:flex;justify-content:space-between;align-items:center;padding:18px 26px;border-bottom:1px solid #344254}h1{margin:0;font-size:20px;font-weight:600}.canvas-header p{color:#9EB0C1;font-size:12px;margin:8px 0 0}.zoom-controls{display:flex;align-items:center;gap:12px;padding:7px 12px;border-radius:7px;background:#334456;font-size:12px}.zoom-controls button{width:22px;height:22px;border-radius:4px;background:#415367}.canvas-viewport{overflow:auto;height:calc(100vh - 214px);min-height:560px;padding:24px}.canvas-stage{position:relative;width:760px;height:620px;background-image:radial-gradient(#526273 1px,transparent 1px);background-size:22px 22px}.edge-layer{position:absolute;inset:0;overflow:visible;pointer-events:none}.module-card{position:absolute;z-index:1;width:176px;height:88px;padding:15px 14px;display:flex;flex-direction:column;align-items:flex-start;gap:7px;border-radius:9px;border-top:4px solid #7BA7FF;background:#273646;text-align:left;box-shadow:0 4px 10px #0b111b55}.module-card:hover,.module-card.selected{outline:2px solid #69D9BA;outline-offset:2px}.module-kind{font-size:10px;letter-spacing:.04em;text-transform:uppercase}.module-card strong{font-size:15px;font-weight:600}.module-card small{color:#A9B9CB;font-size:10px;max-width:148px;overflow:hidden;text-overflow:ellipsis;white-space:nowrap}.status-line{height:50px;padding:16px 26px;color:#8DA3B9;font-size:11px;border-top:1px solid #344254}.inspector h2{margin:0;font-size:19px;font-weight:600}.selected-module{display:flex;align-items:center;gap:10px;min-height:61px;padding:0 15px;margin:14px 0 25px;border-radius:8px;background:#333B44;font-size:14px}.field{margin:0 0 18px}.field-label{display:block;color:#9FB1C3;font-size:12px;margin-bottom:8px}.field-value{padding:11px 15px;border-radius:6px;background:#233247;font-size:13px;min-height:42px}.validation-card{margin-top:30px;padding:16px 15px;border-radius:8px;background:#203B39;color:#CBE9DC;font-size:12px;line-height:1.45}.validation-card p{margin:3px 0}.validation-title{color:#A9E2D2;font-size:10px;letter-spacing:.06em;margin-bottom:8px}.empty-inspector{color:#9FB1C3;font-size:13px;line-height:1.5}.inspector-foot{display:block;color:#7F92A5;font-size:10px;margin-top:110px}.error-page{padding:48px}@media(max-width:1100px){.workspace{grid-template-columns:200px minmax(560px,1fr) 260px}.palette{padding-left:14px;padding-right:14px}.inspector{padding-left:18px;padding-right:18px}}
"#;
