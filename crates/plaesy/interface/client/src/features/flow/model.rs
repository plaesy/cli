//! Model view flow untuk editor: modul, edge dari ekspresi input, dan validasi lokal sederhana.

/// Satu modul di kanvas.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModuleView {
    pub id: String,
    pub kind: String,
    /// Pasangan (nama input, ekspresi sumber).
    pub inputs: Vec<(String, String)>,
}

/// Edge dari modul sumber ke modul tujuan, diturunkan dari ekspresi `mID.path`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Edge {
    pub from: String,
    pub to: String,
}

/// Masalah validasi lokal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LocalIssue {
    EmptyId,
    DuplicateId(String),
    UnknownSource { module: String, source: String },
}

/// Edge dari ekspresi input yang berbentuk `mID.path`. `flow.input.x` tidak membuat edge.
#[must_use]
pub fn edges(modules: &[ModuleView]) -> Vec<Edge> {
    let mut out = Vec::new();
    for module in modules {
        for (_, expr) in &module.inputs {
            if let Some(source) = source_module(expr) {
                out.push(Edge {
                    from: source.to_owned(),
                    to: module.id.clone(),
                });
            }
        }
    }
    out
}

/// Validasi lokal: id tidak kosong, unik, dan setiap sumber `mID` ada di flow.
#[must_use]
pub fn validate(modules: &[ModuleView]) -> Vec<LocalIssue> {
    let mut issues = Vec::new();
    let mut seen = std::collections::BTreeSet::new();
    for module in modules {
        if module.id.is_empty() {
            issues.push(LocalIssue::EmptyId);
        } else if !seen.insert(module.id.clone()) {
            issues.push(LocalIssue::DuplicateId(module.id.clone()));
        }
    }
    let ids: std::collections::BTreeSet<&str> = modules.iter().map(|m| m.id.as_str()).collect();
    for module in modules {
        for (_, expr) in &module.inputs {
            if let Some(source) = source_module(expr) {
                if !ids.contains(source) {
                    issues.push(LocalIssue::UnknownSource {
                        module: module.id.clone(),
                        source: source.to_owned(),
                    });
                }
            }
        }
    }
    issues
}

/// Mengambil `mID` dari ekspresi `mID.path...`. `None` untuk ekspresi lain.
fn source_module(expr: &str) -> Option<&str> {
    let head = expr.trim().split('.').next()?;
    let digits = head.strip_prefix('m')?;
    (!digits.is_empty() && digits.chars().all(|c| c.is_ascii_digit())).then_some(head)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn module(id: &str, inputs: &[(&str, &str)]) -> ModuleView {
        ModuleView {
            id: id.to_owned(),
            kind: "action".to_owned(),
            inputs: inputs
                .iter()
                .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
                .collect(),
        }
    }

    #[test]
    fn edges_come_from_module_references_only() {
        let modules = vec![
            module("m1", &[("id", "flow.input.invoice_id")]),
            module("m2", &[("id", "m1.invoice.id")]),
        ];
        assert_eq!(
            edges(&modules),
            vec![Edge {
                from: "m1".into(),
                to: "m2".into()
            }]
        );
    }

    #[test]
    fn validate_flags_unknown_source_duplicates_and_empty_ids() {
        let modules = vec![
            module("m1", &[]),
            module("m1", &[("x", "m9.out")]),
            module("", &[]),
        ];
        let issues = validate(&modules);
        assert!(issues.contains(&LocalIssue::DuplicateId("m1".into())));
        assert!(issues.contains(&LocalIssue::EmptyId));
        assert!(issues.contains(&LocalIssue::UnknownSource {
            module: "m1".into(),
            source: "m9".into()
        }));
    }

    #[test]
    fn validate_accepts_a_consistent_flow() {
        let modules = vec![module("m1", &[]), module("m2", &[("x", "m1.out")])];
        assert!(validate(&modules).is_empty());
    }
}
