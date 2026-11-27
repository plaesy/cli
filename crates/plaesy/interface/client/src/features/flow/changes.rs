//! Perubahan dokumen flow yang belum disimpan: diff JSON dari dokumen asli ke hasil edit, dan
//! kalimat singkat untuk panel editor. Murni: tanpa I/O dan tanpa UI.

use super::document::patch_document;
use super::model::ModuleView;
use crate::shared::diff::json::{diff, Change};
use serde_json::Value;

/// Panjang maksimum nilai yang ditampilkan utuh dalam satu baris.
const MAX_VALUE_CHARS: usize = 40;

/// Perubahan dari `original` ke dokumen yang dihasilkan dari `modules`. Tidak menyimpan apa pun.
#[must_use]
pub fn document_changes(original: &Value, modules: &[ModuleView]) -> Vec<Change> {
    diff(original, &patch_document(original, modules))
}

/// Kalimat singkat untuk satu perubahan, dengan nilai yang dipangkas bila terlalu panjang.
#[must_use]
pub fn describe_change(change: &Change) -> String {
    match change {
        Change::Added { path, .. } => format!("Ditambah {path}"),
        Change::Removed { path, .. } => format!("Dihapus {path}"),
        Change::Changed { path, from, to } => {
            format!("{path}: {} → {}", short(from), short(to))
        }
    }
}

fn short(value: &Value) -> String {
    let text = value.to_string();
    if text.chars().count() <= MAX_VALUE_CHARS {
        text
    } else {
        let head: String = text.chars().take(MAX_VALUE_CHARS).collect();
        format!("{head}…")
    }
}

#[cfg(test)]
#[path = "changes_test.rs"]
mod changes_test;
