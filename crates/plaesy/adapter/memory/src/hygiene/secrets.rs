//! `secrets` helpers split out of `mod.rs`.

use super::*;

/// Return `(kind, 1-based line)` for likely credentials. Values are never returned.
pub fn secret_references(body: &str) -> Vec<(String, usize)> {
    let mut findings = Vec::new();
    let mut in_fence = false;
    for (index, line) in body.lines().enumerate() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            in_fence = !in_fence;
            continue;
        }
        if in_fence {
            continue;
        }
        let lower = line.to_ascii_lowercase();
        let kind = if lower.contains("-----begin ") && lower.contains("private key-----") {
            Some("private_key")
        } else if has_credential_assignment(line, &lower, &["api_key", "api-key", "api key"]) {
            Some("api_key")
        } else if has_credential_assignment(line, &lower, &["password", "passwd"]) {
            Some("password")
        } else if has_credential_assignment(
            line,
            &lower,
            &["access_token", "access-token", "access token"],
        ) || bearer_value_is_present(line, &lower)
        {
            Some("access_token")
        } else if has_credential_assignment(
            line,
            &lower,
            &["client_secret", "client-secret", "client secret"],
        ) {
            Some("client_secret")
        } else {
            provider_credential_kind(line)
        };
        if let Some(kind) = kind {
            findings.push((kind.to_string(), index + 1));
        }
    }
    findings
}

/// Detect recognizable provider token formats without retaining or returning matched values.
/// These signatures complement assignment-name heuristics; they are not a complete secret scan.
fn provider_credential_kind(line: &str) -> Option<&'static str> {
    const PATTERNS: &[(&str, &str)] = &[
        ("aws_access_key_id", r"\b(?:AKIA|ASIA)[A-Z0-9]{16}\b"),
        (
            "github_token",
            r"\b(?:gh[pousr]_[A-Za-z0-9]{20,}|github_pat_[A-Za-z0-9_]{20,})\b",
        ),
        ("slack_token", r"\bxox[baprs]-[A-Za-z0-9-]{20,}\b"),
        (
            "stripe_secret_key",
            r"\b(?:sk|rk)_(?:live|test)_[A-Za-z0-9]{16,}\b",
        ),
        ("google_api_key", r"\bAIza[0-9A-Za-z_-]{35}\b"),
        (
            "gitlab_token",
            r"\b(?:glpat|gloas|gldt|glrt|glrtr|glcbt|glptt|glft|glimt|glagent|glwt|glsoat|glffct)-[A-Za-z0-9_-]{16,}\b",
        ),
        (
            "anthropic_api_key",
            r"\bsk-ant-api[0-9]{2}-[A-Za-z0-9_-]{20,}\b",
        ),
        (
            "openai_api_key",
            r"\bsk-(?:proj-|svcacct-)?[A-Za-z0-9_-]{32,}\b",
        ),
    ];
    static COMPILED: std::sync::OnceLock<Vec<(&str, Regex)>> = std::sync::OnceLock::new();
    COMPILED
        .get_or_init(|| {
            PATTERNS
                .iter()
                .map(|(kind, pattern)| {
                    (
                        *kind,
                        Regex::new(pattern).expect("static provider token pattern"),
                    )
                })
                .collect()
        })
        .iter()
        .find_map(|(kind, regex)| regex.is_match(line).then_some(*kind))
}

fn has_credential_assignment(line: &str, lower: &str, markers: &[&str]) -> bool {
    markers.iter().any(|marker| {
        lower.match_indices(marker).any(|(start, _)| {
            let previous = lower[..start].chars().next_back();
            if previous.is_some_and(|c| c == '/' || c == '.' || c.is_ascii_alphanumeric()) {
                return false;
            }
            let after_marker = &line[start + marker.len()..];
            let after_key = after_marker.trim_start_matches([' ', '\t', '\'', '"', '`']);
            after_key
                .strip_prefix(['=', ':'])
                .is_some_and(|value| !is_placeholder_value(value))
        })
    })
}

fn bearer_value_is_present(line: &str, lower: &str) -> bool {
    let Some(start) = lower.find("bearer ") else {
        return false;
    };
    let token = line[start + "bearer ".len()..]
        .split(|c: char| c.is_whitespace() || c == ',' || c == ';' || c == ')')
        .next()
        .unwrap_or_default()
        .trim_matches(|c: char| !c.is_ascii_alphanumeric() && c != '_' && c != '-' && c != '.');
    if token.is_empty() || is_placeholder_value(token) || token.len() < 20 {
        return false;
    }
    let classes = [
        token.chars().any(|c| c.is_ascii_lowercase()),
        token.chars().any(|c| c.is_ascii_uppercase()),
        token.chars().any(|c| c.is_ascii_digit()),
        token.chars().any(|c| !c.is_ascii_alphanumeric()),
    ]
    .into_iter()
    .filter(|present| *present)
    .count();
    classes >= 2 || token.len() >= 32
}

fn is_placeholder_value(value: &str) -> bool {
    let raw = value.trim_start().trim_start_matches(['"', '\'', '`']);
    if raw.starts_with(['<', '$', '[']) || raw.starts_with("{{") || raw.starts_with("${") {
        return true;
    }
    let value = value
        .trim_start()
        .trim_start_matches(['=', ':'])
        .trim_start()
        .trim_matches(|c: char| !c.is_ascii_alphanumeric() && c != '_' && c != '-');
    let value = value
        .split(|c: char| c.is_whitespace() || c == ',' || c == ';' || c == '}')
        .next()
        .unwrap_or_default()
        .trim_matches(|c: char| !c.is_ascii_alphanumeric() && c != '_' && c != '-');
    let lower = value.to_ascii_lowercase();
    // Common documentation placeholders and schema type names are not credentials.
    lower.is_empty()
        || matches!(
            lower.as_str(),
            "redacted"
                | "placeholder"
                | "example"
                | "none"
                | "null"
                | "n_a"
                | "na"
                | "todo"
                | "changeme"
                | "change_me"
                | "test"
                | "dummy"
                | "token"
                | "key"
                | "api_server_key"
                | "api-server-key"
                | "access_token"
                | "access-token"
                | "bearer_token"
                | "bearer-token"
                | "string"
                | "boolean"
                | "integer"
                | "number"
                | "object"
                | "array"
                | "x"
        )
        || [
            "your-", "your_", "replace-", "replace_", "example-", "example_",
        ]
        .iter()
        .any(|prefix| lower.starts_with(prefix))
}
