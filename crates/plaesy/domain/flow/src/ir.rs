//! Pengenal dokumen `flow-ir/1`.

use serde_json::Value;

/// Nilai field `ir` untuk dokumen flow.
pub const IR_NAME: &str = "flow-ir/1";

/// Returns true when `doc` declares itself as `flow-ir/1`.
pub fn is_flow(doc: &Value) -> bool {
    doc.get("ir").and_then(Value::as_str) == Some(IR_NAME)
}
