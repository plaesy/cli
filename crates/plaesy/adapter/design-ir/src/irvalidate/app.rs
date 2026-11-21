//! Semantic rules for `app-ir/1` (ADR 0006). Delegates to `plaesy-domain-app`; the rules live there.

use serde_json::Value;

/// Returns the semantic errors for a `app-ir/1` document as messages. An empty vector means it passes.
pub fn validate(doc: &Value) -> Vec<String> {
    plaesy_domain_app::validate(doc)
        .into_iter()
        .map(|error| error.message)
        .collect()
}
