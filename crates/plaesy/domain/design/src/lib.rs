//! Konteks domain design (ADR 0010, menggantikan ADR 0007). Validasi semantik `design-ir/1`:
//! token DTCG, tipe node, rujukan token, dan variant komponen.
//! Port dari `src/designir/validate.rs`. Codegen dan round trip tidak masuk crate ini (ADR 0003).

pub mod rules;
pub mod tokens;

use plaesy_domain_flow::SemanticError;
use serde_json::Value;

pub use plaesy_kernel::json;

/// Returns every semantic error in `doc`. An empty vector means the document passes.
pub fn validate(doc: &Value) -> Vec<SemanticError> {
    rules::validate(doc)
        .into_iter()
        .map(SemanticError::new)
        .collect()
}
