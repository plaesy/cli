//! Binding event ke flow dan data ke komponen (ADR 0006). Pure, tanpa I/O.
//!
//! Sumber nilai: `literal`, `param` (parameter layar), `query` + `column` (baris hasil query),
//! dan `flow_output` + `flow_id` (keluaran flow). `flow_output` hanya tersedia setelah flow
//! selesai, jadi input aksi yang memakainya ditolak di [`fire`], sedangkan parameter navigasi
//! diselesaikan di [`resolve_navigation`] setelah keluaran flow diketahui.

use std::collections::BTreeMap;

use serde_json::Value;

use crate::navigation::components;

/// Konteks saat binding diselesaikan.
#[derive(Debug, Clone, Copy)]
pub struct Context<'a> {
    pub params: &'a BTreeMap<String, String>,
    /// Hasil query per nama query: setiap baris berupa objek kolom ke nilai.
    pub queries: &'a BTreeMap<String, Vec<Value>>,
}

/// Binding event pada satu komponen, dengan sumber nilai yang belum diselesaikan.
#[derive(Debug, Clone, PartialEq)]
pub struct EventBinding {
    pub screen: String,
    pub component: String,
    pub on: String,
    pub flow_id: String,
    pub flow_version: Option<u64>,
    pub inputs: BTreeMap<String, Value>,
    pub on_success: Option<NavSpec>,
    pub on_failure: Option<NavSpec>,
}

/// Navigasi yang belum diselesaikan.
#[derive(Debug, Clone, PartialEq)]
pub struct NavSpec {
    pub screen: String,
    pub params: BTreeMap<String, Value>,
}

/// Navigasi yang sudah diselesaikan: layar tujuan dan nilai parameternya.
#[derive(Debug, Clone, PartialEq)]
pub struct NavCall {
    pub screen: String,
    pub params: BTreeMap<String, Value>,
}

/// Pemanggilan flow yang siap dijalankan, dengan input yang sudah diselesaikan.
#[derive(Debug, Clone, PartialEq)]
pub struct FlowCall {
    pub flow_id: String,
    pub flow_version: Option<u64>,
    pub inputs: BTreeMap<String, Value>,
    pub on_success: Option<NavSpec>,
    pub on_failure: Option<NavSpec>,
}

/// Binding data: prop komponen yang diisi dari query.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataRef {
    pub screen: String,
    pub component: String,
    pub prop: String,
    pub query: String,
    pub column: Option<String>,
}

/// Galat saat binding diselesaikan.
#[derive(Debug, Clone, PartialEq)]
pub enum BindingError {
    /// Tidak ada binding untuk event itu pada komponen itu.
    NoSuchEvent { component: String, on: String },
    /// Bentuk sumber nilai tidak dikenal.
    UnknownSource(String),
    /// Parameter layar yang diminta tidak ada.
    MissingParam(String),
    /// Query atau kolomnya tidak punya baris.
    MissingQueryValue { query: String, column: String },
    /// Keluaran flow belum tersedia pada saat ini.
    FlowOutputPending { flow_id: String, output: String },
}

impl std::fmt::Display for BindingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoSuchEvent { component, on } => {
                write!(f, "tidak ada binding {on} pada {component}")
            }
            Self::UnknownSource(s) => write!(f, "sumber nilai tidak dikenal: {s}"),
            Self::MissingParam(p) => write!(f, "parameter {p} tidak ada"),
            Self::MissingQueryValue { query, column } => {
                write!(f, "query {query} tidak punya nilai untuk kolom {column}")
            }
            Self::FlowOutputPending { flow_id, output } => {
                write!(f, "keluaran {output} dari {flow_id} belum tersedia")
            }
        }
    }
}

impl std::error::Error for BindingError {}

/// Semua binding event di dokumen.
#[must_use]
pub fn event_bindings(doc: &Value) -> Vec<EventBinding> {
    let mut out = Vec::new();
    for screen in doc
        .get("screens")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let Some(screen_id) = screen.get("id").and_then(Value::as_str) else {
            continue;
        };
        for component in components(screen.get("root")) {
            let Some(component_id) = component.get("id").and_then(Value::as_str) else {
                continue;
            };
            for event in component
                .get("events")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
            {
                let (Some(on), Some(action)) =
                    (event.get("on").and_then(Value::as_str), event.get("action"))
                else {
                    continue;
                };
                let Some(flow_id) = action.get("flow_id").and_then(Value::as_str) else {
                    continue;
                };
                out.push(EventBinding {
                    screen: screen_id.to_owned(),
                    component: component_id.to_owned(),
                    on: on.to_owned(),
                    flow_id: flow_id.to_owned(),
                    flow_version: action.get("flow_version").and_then(Value::as_u64),
                    inputs: object_map(action.get("inputs")),
                    on_success: nav_spec(action.get("on_success")),
                    on_failure: nav_spec(action.get("on_failure")),
                });
            }
        }
    }
    out
}

/// Binding data ke prop komponen yang memakai `query`.
#[must_use]
pub fn data_refs(doc: &Value) -> Vec<DataRef> {
    let mut out = Vec::new();
    for screen in doc
        .get("screens")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let Some(screen_id) = screen.get("id").and_then(Value::as_str) else {
            continue;
        };
        for component in components(screen.get("root")) {
            let Some(component_id) = component.get("id").and_then(Value::as_str) else {
                continue;
            };
            for (prop, source) in component
                .get("props")
                .and_then(Value::as_object)
                .into_iter()
                .flatten()
            {
                let Some(query) = source.get("query").and_then(Value::as_str) else {
                    continue;
                };
                out.push(DataRef {
                    screen: screen_id.to_owned(),
                    component: component_id.to_owned(),
                    prop: prop.clone(),
                    query: query.to_owned(),
                    column: source
                        .get("column")
                        .and_then(Value::as_str)
                        .map(str::to_owned),
                });
            }
        }
    }
    out
}

/// Menyelesaikan satu sumber nilai. `flow_output` selalu ditolak di sini karena belum tersedia.
///
/// # Errors
///
/// [`BindingError`] bila sumber tidak dikenal, parameter atau baris tidak ada, atau keluaran flow belum ada.
pub fn resolve_value(source: &Value, ctx: &Context<'_>) -> Result<Value, BindingError> {
    if let Some(literal) = source.get("literal") {
        return Ok(literal.clone());
    }
    if let Some(name) = source.get("param").and_then(Value::as_str) {
        return ctx
            .params
            .get(name)
            .map(|v| Value::String(v.clone()))
            .ok_or_else(|| BindingError::MissingParam(name.to_owned()));
    }
    if let Some(query) = source.get("query").and_then(Value::as_str) {
        let column = source.get("column").and_then(Value::as_str);
        let rows = ctx.queries.get(query).cloned().unwrap_or_default();
        return match column {
            None => Ok(Value::Array(rows)),
            Some(column) => rows
                .first()
                .and_then(|row| row.get(column))
                .cloned()
                .ok_or_else(|| BindingError::MissingQueryValue {
                    query: query.to_owned(),
                    column: column.to_owned(),
                }),
        };
    }
    if let Some(output) = source.get("flow_output").and_then(Value::as_str) {
        let flow_id = source
            .get("flow_id")
            .and_then(Value::as_str)
            .unwrap_or("?")
            .to_owned();
        return Err(BindingError::FlowOutputPending {
            flow_id,
            output: output.to_owned(),
        });
    }
    Err(BindingError::UnknownSource(source.to_string()))
}

/// Menyelesaikan input aksi event dan mengembalikan pemanggilan flow yang siap dijalankan.
///
/// # Errors
///
/// [`BindingError::NoSuchEvent`] bila tidak ada binding itu, atau galat dari [`resolve_value`].
pub fn fire(
    doc: &Value,
    screen: &str,
    component: &str,
    on: &str,
    ctx: &Context<'_>,
) -> Result<FlowCall, BindingError> {
    let binding = event_bindings(doc)
        .into_iter()
        .find(|b| b.screen == screen && b.component == component && b.on == on)
        .ok_or_else(|| BindingError::NoSuchEvent {
            component: component.to_owned(),
            on: on.to_owned(),
        })?;
    let mut inputs = BTreeMap::new();
    for (name, source) in &binding.inputs {
        inputs.insert(name.clone(), resolve_value(source, ctx)?);
    }
    Ok(FlowCall {
        flow_id: binding.flow_id,
        flow_version: binding.flow_version,
        inputs,
        on_success: binding.on_success,
        on_failure: binding.on_failure,
    })
}

/// Menyelesaikan navigasi setelah flow selesai. `flow_output` diambil dari `flow_outputs`.
///
/// # Errors
///
/// Galat dari [`resolve_value`], atau keluaran flow yang tidak ada di `flow_outputs`.
pub fn resolve_navigation(
    spec: &NavSpec,
    ctx: &Context<'_>,
    flow_outputs: &BTreeMap<String, Value>,
) -> Result<NavCall, BindingError> {
    let mut params = BTreeMap::new();
    for (name, source) in &spec.params {
        let value = match source.get("flow_output").and_then(Value::as_str) {
            Some(output) => flow_outputs.get(output).cloned().ok_or_else(|| {
                BindingError::FlowOutputPending {
                    flow_id: source
                        .get("flow_id")
                        .and_then(Value::as_str)
                        .unwrap_or("?")
                        .to_owned(),
                    output: output.to_owned(),
                }
            })?,
            None => resolve_value(source, ctx)?,
        };
        params.insert(name.clone(), value);
    }
    Ok(NavCall {
        screen: spec.screen.clone(),
        params,
    })
}

fn object_map(value: Option<&Value>) -> BTreeMap<String, Value> {
    value
        .and_then(Value::as_object)
        .into_iter()
        .flatten()
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect()
}

fn nav_spec(value: Option<&Value>) -> Option<NavSpec> {
    let value = value?;
    Some(NavSpec {
        screen: value.get("screen")?.as_str()?.to_owned(),
        params: object_map(value.get("params")),
    })
}

#[cfg(test)]
#[path = "bindings_test.rs"]
mod bindings_test;
