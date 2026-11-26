//! Renderer web untuk editor flow Plaesy.
//!
//! Layar ini memakai model dan layout flow yang sama dengan domain editor. UI-nya sengaja
//! tetap lokal dan ringan: tidak ada jaringan atau state global, tetapi canvas, palette,
//! inspector, validasi, pemilihan node, dan kontrol zoom sudah menjadi permukaan editor nyata.

use crate::features::flow::layout::{layout, Position};
use crate::features::flow::model::{
    edges, modules_from_flow_json, validate, LocalIssue, ModuleView,
};
use dioxus::prelude::*;
use std::collections::BTreeMap;

pub fn run() {
    dioxus::launch(App);
}

#[component]
fn App() -> Element {
    let loaded = modules_from_flow_json(include_str!("kirim-faktur.json"));
    rsx! { style { {STYLE} } match loaded {
        Ok((name, modules)) => rsx! { FlowEditor { name, modules } },
        Err(message) => rsx! { main { class: "error-page", h1 { "Gagal memuat flow" }, p { "{message}" } } },
    } }
}

#[derive(Props, Clone, PartialEq)]
struct FlowEditorProps {
    name: String,
    modules: Vec<ModuleView>,
}

#[component]
fn FlowEditor(props: FlowEditorProps) -> Element {
    let edge_list = edges(&props.modules);
    let issues = validate(&props.modules);
    let ids: Vec<String> = props.modules.iter().map(|m| m.id.clone()).collect();
    let edge_pairs: Vec<(String, String)> = edge_list
        .iter()
        .map(|e| (e.from.clone(), e.to.clone()))
        .collect();
    let positions = layout(&ids, &edge_pairs).unwrap_or_else(|_| fallback_positions(&ids));
    let mut selected = use_signal(|| Some("m3".to_owned()));
    let mut zoom = use_signal(|| 0.8_f64);
    let mut status = use_signal(|| "Draft changes are local to this editor".to_owned());
    let selected_module = selected()
        .as_deref()
        .and_then(|id| props.modules.iter().find(|m| m.id == id));

    rsx! { div { class: "app-shell",
        header { class: "topbar",
            div { class: "brand-lockup", span { class: "brand-mark", aria_hidden: "true" }, strong { "Plaesy" } }
            span { class: "topbar-divider", aria_hidden: "true" }
            span { class: "breadcrumb", "Flows  /  {props.name}" }
            span { class: "version", "flow-ir/1 · version 3" }
            div { class: "top-actions",
                button { class: "button button-validate", onclick: move |_| status.set(if issues.is_empty() { "Validation passed · no local issues" } else { "Validation found local issues" }), "Validate" }
                button { class: "button button-save", onclick: move |_| status.set("Draft saved locally · ready for review"), "Save draft" }
                span { class: "concept-badge", "CONCEPT" }
            }
        }
        div { class: "workspace",
            aside { class: "palette",
                div { class: "eyebrow", "NODE PALETTE" }
                for (label, color) in [("Action", "#7BA7FF"), ("Branch", "#C6A9FF"), ("Approval", "#F7C879"), ("For each", "#71E6C1"), ("Subflow", "#F29D9D")] {
                    button { class: "palette-item", onclick: move |_| status.set(format!("{label} node selected · drop it on the canvas to add")), span { class: "palette-dot", style: "background:{color}" }, span { "{label}" } }
                }
                div { class: "palette-separator" }
                div { class: "eyebrow", "EXAMPLE FLOW" }
                div { class: "library-name", "kirim-faktur.json" }
                div { class: "library-path", "client / platform / web" }
                p { class: "palette-note", "Graph preview uses a repository fixture." }
            }
            main { class: "canvas-panel",
                div { class: "canvas-header",
                    div { h1 { "Send invoice" }, p { "10 modules · approval and failure path" } }
                    div { class: "zoom-controls", button { aria_label: "Zoom out", onclick: move |_| zoom.set((zoom() - 0.1).max(0.5)), "−" }, span { "{(zoom() * 100.0) as i32}%" }, button { aria_label: "Zoom in", onclick: move |_| zoom.set((zoom() + 0.1).min(1.2)), "+" } }
                }
                div { class: "canvas-viewport",
                    div { class: "canvas-stage", style: "transform:scale({zoom()}); transform-origin:top left;",
                        svg { class: "edge-layer", width: "760", height: "620", view_box: "0 0 760 620", aria_hidden: "true",
                            defs { marker { id: "arrow", marker_width: "8", marker_height: "8", ref_x: "6", ref_y: "3", orient: "auto", marker_units: "strokeWidth", path { d: "M0,0 L0,6 L6,3 z", fill: "#718CA0" } } }
                            for edge in edge_list.iter() { if let (Some(from), Some(to)) = (positions.get(&edge.from), positions.get(&edge.to)) {
                                line { x1: "{node_x(from) + 176}", y1: "{node_y(from) + 44}", x2: "{node_x(to)}", y2: "{node_y(to) + 44}", stroke: "#718CA0", stroke_width: "2", marker_end: "url(#arrow)" }
                            } }
                        }
                        for module in props.modules.iter() { {module_card(module, positions.get(&module.id), selected, status)} }
                    }
                }
                div { class: "status-line", "{status()}" }
            }
            aside { class: "inspector",
                h2 { "Inspector" }
                div { class: "inspector-divider" }
                div { class: "eyebrow", "SELECTED MODULE" }
                if let Some(module) = selected_module {
                    div { class: "selected-module", span { class: "selected-dot", style: "background:{kind_color(&module.kind)}" }, strong { "{module.id} · {module.kind}" } }
                    Field { label: "Operation", value: operation_label(module) }
                    Field { label: "Inputs", value: if module.inputs.is_empty() { "None".to_owned() } else { format!("{} mapped", module.inputs.len()) } }
                    Field { label: "On error", value: "fail".to_owned() }
                } else { p { class: "empty-inspector", "Select a module to inspect its inputs and runtime policy." } }
                div { class: "validation-card", div { class: "validation-title", "LOCAL VALIDATION" }, if issues.is_empty() {
                    p { "✓ IDs unique" } p { "✓ Input sources resolve" } p { "✓ No local issues found" }
                } else { for issue in issues.iter() { p { "• {describe(issue)}" } } } }
                small { class: "inspector-foot", "Editor concept · no release claim" }
            }
        }
    } }
}

#[component]
fn Field(label: &'static str, value: String) -> Element {
    rsx! { div { class: "field", span { class: "field-label", "{label}" }, div { class: "field-value", "{value}" } } }
}

fn module_card(
    module: &ModuleView,
    position: Option<&Position>,
    mut selected: Signal<Option<String>>,
    mut status: Signal<String>,
) -> Element {
    let position = position.copied().unwrap_or(Position { x: 0, y: 0 });
    let module_id = module.id.clone();
    let kind = module.kind.clone();
    let style = format!(
        "left:{}px;top:{}px;border-top-color:{}",
        node_x(&position),
        node_y(&position),
        kind_color(&kind)
    );
    rsx! { button { class: if selected().as_deref() == Some(module.id.as_str()) { "module-card selected" } else { "module-card" }, style: "{style}", onclick: move |_| { selected.set(Some(module_id.clone())); status.set(format!("Selected {module_id}")); },
        span { class: "module-kind", style: "color:{kind_color(&module.kind)}", "{module.id} · {module.kind}" }
        strong { "{operation_label(module)}" } small { "{input_summary(module)}" }
    } }
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
fn operation_label(m: &ModuleView) -> String {
    match m.kind.as_str() {
        "action" if m.id == "m1" => "Get invoice",
        "action" if m.id == "m5" => "Check lines",
        "action" if m.id == "m9" => "Send invoice",
        "action" if m.id == "m10" => "Notify failure",
        "branch_one" => "Invoice status",
        "approval" => "Human review",
        "for_each" => "Check lines",
        "pass" => "Skip",
        "subflow" => "Finalize billing",
        "while" => "Wait for result",
        _ => m.kind.as_str(),
    }
    .to_owned()
}
fn input_summary(m: &ModuleView) -> String {
    m.inputs
        .first()
        .map(|(_, s)| s.clone())
        .unwrap_or_else(|| "No inputs configured".to_owned())
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

const STYLE: &str = r#"
:root { color-scheme:dark; font-family:Inter,ui-sans-serif,system-ui,-apple-system,"Segoe UI",sans-serif; background:#111B2B; color:#E9EEF7 } *{box-sizing:border-box} body{margin:0;min-width:980px;background:#111B2B} button{font:inherit;color:inherit;border:0;cursor:pointer}.app-shell{min-height:100vh;background:#111B2B}.topbar{height:72px;display:flex;align-items:center;gap:22px;padding:0 22px;background:#11192A;border-bottom:1px solid #2C3B4E}.brand-lockup{display:flex;align-items:center;gap:12px;font-size:22px;min-width:126px}.brand-mark{width:8px;height:29px;border-radius:4px;background:#69D9BA;display:inline-block}.topbar-divider{width:1px;height:38px;background:#2C3B4E}.breadcrumb{font-size:15px}.version{margin-left:8px;color:#8DA2B7;font-size:11px}.top-actions{margin-left:auto;display:flex;gap:12px;align-items:center}.button,.concept-badge{height:38px;padding:0 20px;border-radius:7px;font-size:13px}.button-validate{background:#27483D;color:#C9F3E0}.button-save{background:#263F60;color:#D3E7FC}.concept-badge{display:flex;align-items:center;background:#343248;color:#DCCCF7;font-size:11px;letter-spacing:.06em}.workspace{display:grid;grid-template-columns:226px minmax(600px,1fr) 307px;min-height:calc(100vh - 72px)}.palette,.inspector{background:#172438;padding:30px 20px}.palette{border-right:1px solid #2E3D4F}.inspector{border-left:1px solid #2E3D4F;padding:28px 24px}.eyebrow{color:#7D92A7;font-size:10px;letter-spacing:.08em;margin-bottom:16px}.palette-item{width:100%;height:51px;display:flex;align-items:center;gap:11px;padding:0 14px;margin-bottom:13px;border-radius:8px;background:#202E43;text-align:left;font-size:14px}.palette-item:hover{background:#293A4C}.palette-dot,.selected-dot{width:10px;height:10px;border-radius:3px;display:inline-block;flex:none}.palette-separator,.inspector-divider{height:1px;background:#293A4C;margin:22px 0}.library-name{font-size:13px;margin-top:28px}.library-path,.palette-note{color:#8DA3B9;font-size:11px}.palette-note{margin-top:45px;line-height:1.45}.canvas-panel{background:#202A37;min-width:0}.canvas-header{height:92px;display:flex;justify-content:space-between;align-items:center;padding:18px 26px;border-bottom:1px solid #344254}h1{margin:0;font-size:20px;font-weight:600}.canvas-header p{color:#9EB0C1;font-size:12px;margin:8px 0 0}.zoom-controls{display:flex;align-items:center;gap:12px;padding:7px 12px;border-radius:7px;background:#334456;font-size:12px}.zoom-controls button{width:22px;height:22px;border-radius:4px;background:#415367}.canvas-viewport{overflow:auto;height:calc(100vh - 214px);min-height:560px;padding:24px}.canvas-stage{position:relative;width:760px;height:620px;background-image:radial-gradient(#526273 1px,transparent 1px);background-size:22px 22px}.edge-layer{position:absolute;inset:0;overflow:visible;pointer-events:none}.module-card{position:absolute;z-index:1;width:176px;height:88px;padding:15px 14px;display:flex;flex-direction:column;align-items:flex-start;gap:7px;border-radius:9px;border-top:4px solid #7BA7FF;background:#273646;text-align:left;box-shadow:0 4px 10px #0b111b55}.module-card:hover,.module-card.selected{outline:2px solid #69D9BA;outline-offset:2px}.module-kind{font-size:10px;letter-spacing:.04em;text-transform:uppercase}.module-card strong{font-size:15px;font-weight:600}.module-card small{color:#A9B9CB;font-size:10px;max-width:148px;overflow:hidden;text-overflow:ellipsis;white-space:nowrap}.status-line{height:50px;padding:16px 26px;color:#8DA3B9;font-size:11px;border-top:1px solid #344254}.inspector h2{margin:0;font-size:19px;font-weight:600}.selected-module{display:flex;align-items:center;gap:10px;min-height:61px;padding:0 15px;margin:14px 0 25px;border-radius:8px;background:#333B44;font-size:14px}.field{margin:0 0 18px}.field-label{display:block;color:#9FB1C3;font-size:12px;margin-bottom:8px}.field-value{padding:11px 15px;border-radius:6px;background:#233247;font-size:13px;min-height:42px}.validation-card{margin-top:30px;padding:16px 15px;border-radius:8px;background:#203B39;color:#CBE9DC;font-size:12px;line-height:1.45}.validation-card p{margin:3px 0}.validation-title{color:#A9E2D2;font-size:10px;letter-spacing:.06em;margin-bottom:8px}.empty-inspector{color:#9FB1C3;font-size:13px;line-height:1.5}.inspector-foot{display:block;color:#7F92A5;font-size:10px;margin-top:110px}.error-page{padding:48px}@media(max-width:1100px){.workspace{grid-template-columns:200px minmax(560px,1fr) 260px}.palette{padding-left:14px;padding-right:14px}.inspector{padding-left:18px;padding-right:18px}}
"#;
