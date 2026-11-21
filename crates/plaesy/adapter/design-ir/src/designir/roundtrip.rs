//! Structural round trip for `design-ir/1` (ADR 0010, "Fidelity").
//!
//! `check` generates the Rust module for a document, reads it back, and compares the
//! design ID, the token values, and the node rows (ID, parent, kind, scope) in order.
//! It reports every mismatch. It does not render anything.

use serde_json::Value;

use super::codegen::{generate, node_rows, token_rows, NodeRow};
use super::{str_of, DesignError};

/// Result of one round trip.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Report {
    pub design_id: String,
    pub tokens: usize,
    pub nodes: usize,
    /// Human-readable differences. Empty means the round trip is structurally exact.
    pub mismatches: Vec<String>,
}

/// Contents of a generated module, read back from its text.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Extracted {
    pub design_id: String,
    pub tokens: Vec<(String, String)>,
    pub nodes: Vec<NodeRow>,
}

/// Runs IR -> Rust module -> IR for `doc` and compares the two sides.
pub fn check(doc: &Value) -> Result<Report, DesignError> {
    let source = generate(doc)?;
    let extracted = extract(&source)?;
    let expected_tokens = token_rows(doc);
    let expected_nodes = node_rows(doc);

    let mut mismatches = Vec::new();
    let expected_id = str_of(doc, "id").unwrap_or("");
    if extracted.design_id != expected_id {
        mismatches.push(format!(
            "design id: {expected_id} != {}",
            extracted.design_id
        ));
    }
    compare_lists(
        "token",
        &expected_tokens,
        &extracted.tokens,
        &mut mismatches,
    );
    compare_lists("node", &expected_nodes, &extracted.nodes, &mut mismatches);

    Ok(Report {
        design_id: extracted.design_id,
        tokens: expected_tokens.len(),
        nodes: expected_nodes.len(),
        mismatches,
    })
}

fn compare_lists<T: PartialEq + std::fmt::Debug>(
    what: &str,
    expected: &[T],
    actual: &[T],
    mismatches: &mut Vec<String>,
) {
    if expected.len() != actual.len() {
        mismatches.push(format!(
            "jumlah {what}: {} != {}",
            expected.len(),
            actual.len()
        ));
    }
    for (i, (e, a)) in expected.iter().zip(actual.iter()).enumerate() {
        if e != a {
            mismatches.push(format!("{what} ke-{i}: {e:?} != {a:?}"));
        }
    }
}

/// Reads a module produced by `codegen::generate` back into its parts.
pub fn extract(source: &str) -> Result<Extracted, DesignError> {
    let mut out = Extracted::default();
    let mut section = "";
    for raw in source.lines() {
        let line = raw.trim();
        if let Some(rest) = line.strip_prefix("pub const DESIGN_ID: &str = ") {
            let (value, _) = parse_literal(rest.trim_end_matches(';').trim())?;
            out.design_id = value;
        } else if line.starts_with("pub const TOKENS") {
            section = "tokens";
        } else if line.starts_with("pub const NODES") {
            section = "nodes";
        } else if line == "];" {
            section = "";
        } else if section == "tokens" && line.starts_with('(') {
            out.tokens.push(parse_token_line(line)?);
        } else if section == "nodes" && line.starts_with("NodeSpec {") {
            out.nodes.push(parse_node_line(line)?);
        }
    }
    Ok(out)
}

fn malformed(line: &str) -> DesignError {
    DesignError::Malformed(format!("baris tidak dikenali: {line}"))
}

fn parse_token_line(line: &str) -> Result<(String, String), DesignError> {
    let inner = line
        .strip_prefix('(')
        .and_then(|s| s.strip_suffix("),"))
        .ok_or_else(|| malformed(line))?;
    let (name, rest) = parse_literal(inner)?;
    let rest = rest
        .trim_start()
        .strip_prefix(',')
        .ok_or_else(|| malformed(line))?;
    let (value, tail) = parse_literal(rest.trim_start())?;
    if !tail.trim().is_empty() {
        return Err(malformed(line));
    }
    Ok((name, value))
}

fn parse_node_line(line: &str) -> Result<NodeRow, DesignError> {
    let mut rest = line
        .strip_prefix("NodeSpec { id: ")
        .ok_or_else(|| malformed(line))?;
    let (id, after) = parse_literal(rest)?;
    rest = expect(after, ", parent: ", line)?;
    let (parent, after) = if let Some(r) = rest.strip_prefix("None") {
        (None, r)
    } else {
        let r = rest.strip_prefix("Some(").ok_or_else(|| malformed(line))?;
        let (p, r) = parse_literal(r)?;
        (Some(p), expect(r, ")", line)?)
    };
    rest = expect(after, ", kind: ", line)?;
    let (kind, after) = parse_literal(rest)?;
    rest = expect(after, ", scope: ", line)?;
    let (scope, after) = parse_literal(rest)?;
    expect(after, " },", line)?;
    Ok(NodeRow {
        id,
        parent,
        kind,
        scope,
    })
}

fn expect<'a>(input: &'a str, prefix: &str, line: &str) -> Result<&'a str, DesignError> {
    input.strip_prefix(prefix).ok_or_else(|| malformed(line))
}

/// Parses one Rust string literal written by `Debug`. Supports the escapes that
/// `Debug` produces for ordinary text: `\\`, `\"`, `\n`, `\r`, `\t`, `\'`.
pub fn parse_literal(input: &str) -> Result<(String, &str), DesignError> {
    let mut chars = input.char_indices();
    match chars.next() {
        Some((_, '"')) => {}
        _ => {
            return Err(DesignError::Malformed(format!(
                "literal string diharapkan: {input}"
            )))
        }
    }
    let mut out = String::new();
    while let Some((i, c)) = chars.next() {
        match c {
            '"' => return Ok((out, &input[i + 1..])),
            '\\' => {
                let Some((_, esc)) = chars.next() else {
                    break;
                };
                out.push(match esc {
                    'n' => '\n',
                    'r' => '\r',
                    't' => '\t',
                    other => other,
                });
            }
            other => out.push(other),
        }
    }
    Err(DesignError::Malformed(format!(
        "literal string tidak ditutup: {input}"
    )))
}
