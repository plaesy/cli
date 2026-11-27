//! Bantuan untuk inspector dan kanvas: daftar sumber untuk Connect, bentuk ekspresi sumber, dan
//! modul yang perlu ditandai dari daftar masalah validasi. Murni: tanpa I/O dan tanpa UI.

use super::model::{LocalIssue, ModuleView};
use std::collections::BTreeSet;

/// Modul lain yang bisa dipakai sebagai sumber untuk `target`, dalam urutan dokumen.
#[must_use]
pub fn connect_candidates(modules: &[ModuleView], target: &str) -> Vec<String> {
    modules
        .iter()
        .filter(|m| m.id != target)
        .map(|m| m.id.clone())
        .collect()
}

/// Ekspresi sumber `mID.path`. `None` bila modul atau path kosong setelah dipangkas.
#[must_use]
pub fn source_expression(module: &str, path: &str) -> Option<String> {
    let module = module.trim();
    let path = path.trim();
    if module.is_empty() || path.is_empty() {
        None
    } else {
        Some(format!("{module}.{path}"))
    }
}

/// Modul yang ditandai di kanvas: id yang dobel, dan modul yang punya sumber tak dikenal.
/// Id kosong tidak punya modul untuk ditandai, jadi dilewati.
#[must_use]
pub fn flagged_modules(issues: &[LocalIssue]) -> BTreeSet<String> {
    issues
        .iter()
        .filter_map(|issue| match issue {
            LocalIssue::DuplicateId(id) => Some(id.clone()),
            LocalIssue::UnknownSource { module, .. } => Some(module.clone()),
            LocalIssue::EmptyId => None,
        })
        .collect()
}

#[cfg(test)]
#[path = "inspect_test.rs"]
mod inspect_test;
