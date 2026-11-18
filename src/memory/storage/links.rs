//! Storage links types (split out of `storage/mod.rs`).

use super::*;

/// A `[[wikilink]]` found in a document body.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct LinkRef {
    /// Path of the document containing the link.
    pub source_path: String,

    /// Link target exactly as written (without alias or heading part).
    pub target: String,

    /// 1-based line of the link in the source file.
    pub line: usize,

    /// Path of the document the target resolves to, if any.
    pub resolved_path: Option<String>,
}

/// Whether a `[[...]]` body names a note rather than being a template directive.
///
/// Also applied when links are read back, so tightening these rules takes effect on an
/// existing index without re-indexing.
pub fn plausible_target(target: &str) -> bool {
    !target.is_empty()
        && target.len() <= 120
        && !target.contains(": ")
        && !target.contains("{{")
        && !is_directive(target)
        && !target.contains(['\n', '[', ']'])
}

/// `[[BROWNFIELD ONLY]]`-style conditional markers: several words, all upper case.
fn is_directive(target: &str) -> bool {
    target.contains(' ')
        && target.chars().any(char::is_alphabetic)
        && !target.chars().any(char::is_lowercase)
}

/// Extract `[[target]]`, `[[target|alias]]` and `[[target#heading]]` links from Markdown.
///
/// Returns `(target, 1-based line within body)`. Links inside fenced code blocks are ignored.
pub fn extract_wikilinks(body: &str) -> Vec<(String, usize)> {
    let mut out = Vec::new();
    let mut in_fence = false;
    for (i, line) in body.lines().enumerate() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            in_fence = !in_fence;
            continue;
        }
        if in_fence {
            continue;
        }
        // Links inside inline code spans are examples, not links.
        let visible: String = line
            .split('`')
            .enumerate()
            .filter(|(n, _)| n % 2 == 0)
            .map(|(_, part)| part)
            .collect::<Vec<_>>()
            .join(" ");
        let mut rest = visible.as_str();
        while let Some(start) = rest.find("[[") {
            let after = &rest[start + 2..];
            let Some(end) = after.find("]]") else { break };
            let raw = &after[..end];
            let target = raw
                .split(['|', '#'])
                .next()
                .unwrap_or("")
                .trim()
                .to_string();
            // Template directives such as `[[LLM: do this]]` or `[[{{PLACEHOLDER}}]]` are
            // not links: real note names are short and contain no sentence punctuation.
            if plausible_target(&target) {
                out.push((target, i + 1));
            }
            rest = &after[end + 2..];
        }
    }
    out
}
