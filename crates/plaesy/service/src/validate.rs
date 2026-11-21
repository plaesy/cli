//! Use case: validasi semantik satu dokumen IR, dengan dispatch berdasarkan field `ir`.

use plaesy_domain_flow::{ir as flow_ir, SemanticError};
use serde_json::Value;

/// Hasil validasi satu dokumen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidationReport {
    /// Nilai field `ir`, atau `None` bila tidak ada.
    pub ir: Option<String>,
    /// Pelanggaran aturan semantik. Kosong berarti lolos.
    pub errors: Vec<SemanticError>,
}

impl ValidationReport {
    pub fn is_valid(&self) -> bool {
        self.errors.is_empty()
    }
}

/// Validates `doc` against the semantic rules of the IR named in its `ir` field.
/// An unknown or missing `ir` yields one error and no dispatch.
pub fn validate_document(doc: &Value) -> ValidationReport {
    let ir = doc.get("ir").and_then(Value::as_str).map(str::to_string);
    let errors = match ir.as_deref() {
        Some(name) if name == flow_ir::IR_NAME => plaesy_domain_flow::validate(doc),
        Some("data-ir/1") => plaesy_domain_data::validate(doc),
        Some("api-ir/1") => plaesy_domain_api::validate(doc),
        Some("app-ir/1") => plaesy_domain_app::validate(doc),
        Some("design-ir/1") => plaesy_domain_design::validate(doc),
        other => vec![SemanticError::new(format!("ir tidak dikenal: {other:?}"))],
    };
    ValidationReport { ir, errors }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn unknown_ir_is_reported_once() {
        let report = validate_document(&json!({ "ir": "nope/1" }));
        assert!(!report.is_valid());
        assert_eq!(report.errors.len(), 1);
        assert_eq!(report.ir.as_deref(), Some("nope/1"));
    }

    #[test]
    fn missing_ir_is_reported() {
        let report = validate_document(&json!({}));
        assert!(!report.is_valid());
        assert_eq!(report.ir, None);
    }

    #[test]
    fn flow_document_dispatches_to_flow_rules() {
        let doc = json!({
            "ir": "flow-ir/1",
            "id": "flw_1",
            "inputs": {},
            "scope": [],
            "modules": [{ "id": "m1", "kind": "action", "default": ["ghost"] }]
        });
        let report = validate_document(&doc);
        assert!(report
            .errors
            .iter()
            .any(|e| e.message.contains("merujuk module yang tidak ada: ghost")));
    }

    #[test]
    fn official_flow_example_is_valid() {
        let text = include_str!("../../../../docs/adr/examples/flow-ir-1/kirim-faktur.json");
        let doc: Value = serde_json::from_str(text).unwrap();
        assert!(validate_document(&doc).is_valid());
    }
}
