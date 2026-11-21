//! Semantic rules for `flow-ir/1` (ADR 0002). Delegates to `plaesy-domain-flow`; the rules live there.

use serde_json::Value;

/// Returns the semantic errors for a `flow-ir/1` document as messages. An empty vector means it passes.
pub fn validate(doc: &Value) -> Vec<String> {
    plaesy_domain_flow::validate(doc)
        .into_iter()
        .map(|error| error.message)
        .collect()
}
