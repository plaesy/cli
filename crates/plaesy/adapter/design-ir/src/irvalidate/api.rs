//! Semantic rules for `api-ir/1` (ADR 0005). Delegates to `plaesy-domain-api`; the rules live there.

use serde_json::Value;

/// Returns the semantic errors for a `api-ir/1` document as messages. An empty vector means it passes.
pub fn validate(doc: &Value) -> Vec<String> {
    plaesy_domain_api::validate(doc)
        .into_iter()
        .map(|error| error.message)
        .collect()
}
