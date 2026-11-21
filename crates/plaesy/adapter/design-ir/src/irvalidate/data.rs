//! Semantic rules for `data-ir/1` (ADR 0004). Delegates to `plaesy-domain-data`; the rules live there.

use serde_json::Value;

/// Returns the semantic errors for a `data-ir/1` document as messages. An empty vector means it passes.
pub fn validate(doc: &Value) -> Vec<String> {
    plaesy_domain_data::validate(doc)
        .into_iter()
        .map(|error| error.message)
        .collect()
}
