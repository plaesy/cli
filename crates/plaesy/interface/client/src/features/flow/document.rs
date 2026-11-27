//! Menulis ulang dokumen flow dari modul hasil edit. Field di luar `modules`, serta field lain di
//! setiap modul (retry, timeout, branch, input non-teks), dipertahankan apa adanya.

use super::model::ModuleView;
use serde_json::{json, Map, Value};

/// Dokumen `original` dengan daftar `modules` diganti sesuai `modules` hasil edit.
///
/// Modul yang id-nya masih ada mempertahankan field lamanya. Input bertipe teks diganti sesuai
/// edit; input lain tidak disentuh. Modul yang tidak ada lagi dihapus. Modul baru hanya membawa
/// `id`, `kind`, `operation` bila ada, dan `inputs`.
#[must_use]
pub fn patch_document(original: &Value, modules: &[ModuleView]) -> Value {
    let mut doc = original.clone();
    let existing: Vec<Value> = original
        .get("modules")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let patched: Vec<Value> = modules
        .iter()
        .map(|module| {
            let mut object = existing
                .iter()
                .find(|m| m.get("id").and_then(Value::as_str) == Some(module.id.as_str()))
                .and_then(Value::as_object)
                .cloned()
                .unwrap_or_default();
            object.insert("id".to_owned(), json!(module.id));
            object.insert("kind".to_owned(), json!(module.kind));
            if !module.operation.is_empty() {
                object.insert("operation".to_owned(), json!(module.operation));
            }
            patch_inputs(&mut object, module);
            Value::Object(object)
        })
        .collect();
    if let Some(fields) = doc.as_object_mut() {
        fields.insert("modules".to_owned(), Value::Array(patched));
    }
    doc
}

/// Mengganti input bertipe teks dengan hasil edit. Input non-teks dari dokumen lama tetap ada.
fn patch_inputs(object: &mut Map<String, Value>, module: &ModuleView) {
    let had_inputs = object.contains_key("inputs");
    let mut inputs = object
        .get("inputs")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    inputs.retain(|name, value| {
        !value.is_string() || module.inputs.iter().any(|(edited, _)| edited == name)
    });
    for (name, source) in &module.inputs {
        inputs.insert(name.clone(), json!(source));
    }
    if had_inputs || !inputs.is_empty() {
        object.insert("inputs".to_owned(), Value::Object(inputs));
    }
}

#[cfg(test)]
#[path = "document_test.rs"]
mod document_test;
