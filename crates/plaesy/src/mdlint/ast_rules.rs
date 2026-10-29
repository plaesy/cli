//! Rules that need the parsed document: heading structure and text.

use serde::Deserialize;

use super::doc::Doc;
use super::text_rules::starts_own_paragraph;
use super::{RuleSet, Violation};

fn v(rule: &'static str, line: usize, column: usize, message: impl Into<String>) -> Violation {
    Violation {
        rule,
        line,
        column,
        message: message.into(),
    }
}

// ---- MD001 heading-increment -------------------------------------------------

pub fn md001(d: &Doc, _rs: &RuleSet) -> Vec<Violation> {
    let mut previous = 0usize;
    let mut out = Vec::new();
    for h in d.ast_headings() {
        if previous > 0 && h.level > previous + 1 {
            out.push(v(
                "MD001",
                h.line,
                1,
                format!("heading level jumps from h{previous} to h{}", h.level),
            ));
        }
        previous = h.level;
    }
    out
}

// ---- MD003 heading-style -----------------------------------------------------

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct Md003Options {
    pub style: String,
}

pub fn md003(d: &Doc, rs: &RuleSet) -> Vec<Violation> {
    let style = rs.opt::<Md003Options>("MD003").style;
    let style = if style.is_empty() {
        "consistent".to_owned()
    } else {
        style
    };
    let mut first: Option<&'static str> = None;
    let mut out = Vec::new();
    for h in d.ast_headings() {
        let kind = if h.setext { "setext" } else { "atx" };
        if style == "consistent" {
            let expected = *first.get_or_insert(kind);
            if kind != expected {
                out.push(v(
                    "MD003",
                    h.line,
                    1,
                    format!("heading style {kind} is inconsistent with {expected}"),
                ));
            }
            continue;
        }
        if kind != style {
            out.push(v(
                "MD003",
                h.line,
                1,
                format!("heading style {kind} (style: {style})"),
            ));
        }
    }
    out
}

// ---- MD024 no-duplicate-heading ----------------------------------------------

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct Md024Options {
    pub siblings_only: Option<bool>,
}

pub fn md024(d: &Doc, rs: &RuleSet) -> Vec<Violation> {
    let siblings_only = rs
        .opt::<Md024Options>("MD024")
        .siblings_only
        .unwrap_or(true);
    let mut seen = std::collections::BTreeSet::new();
    // The current text at each heading level, so headings are compared within their section.
    let mut path: std::collections::BTreeMap<usize, String> = Default::default();
    let mut out = Vec::new();
    for h in d.ast_headings() {
        let text = h.text.trim().to_lowercase();
        let key = if siblings_only {
            let mut parts: Vec<String> = (1..h.level)
                .filter_map(|lvl| path.get(&lvl).cloned())
                .collect();
            parts.push(text.clone());
            parts.join("\u{0}")
        } else {
            text.clone()
        };
        let message = if siblings_only {
            format!(
                "duplicate heading \"{text}\" (a sibling under the same parent already uses it)"
            )
        } else {
            format!("duplicate heading \"{text}\" (this level already has it)")
        };
        path.insert(h.level, text);
        if !seen.insert(key) {
            out.push(v("MD024", h.line, 1, message));
        }
    }
    out
}

// ---- MD025 single-title ------------------------------------------------------

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct Md025Options {
    pub level: Option<usize>,
    pub front_matter_title: String,
}

/// Value of the front matter's title field, when the document has one.
fn front_matter_title(d: &Doc, pattern: &str) -> Option<String> {
    if d.fm_end == 0 {
        return None;
    }
    let field = if pattern.is_empty() { "title" } else { pattern };
    if pattern == "^$" {
        return None;
    }
    for line in &d.lines[d.fm_start..d.fm_end] {
        if let Some((key, value)) = line.split_once(':') {
            if key.trim() == field {
                return Some(value.trim().trim_matches(['"', '\'']).to_owned());
            }
        }
    }
    None
}

pub fn md025(d: &Doc, rs: &RuleSet) -> Vec<Violation> {
    let o = rs.opt::<Md025Options>("MD025");
    let level = o.level.unwrap_or(1);
    let fm_counts =
        front_matter_title(d, &o.front_matter_title).is_some() && o.front_matter_title != "^$";
    let mut count = usize::from(fm_counts);
    let mut out = Vec::new();
    for h in d.ast_headings() {
        // The YAML front matter is parsed as body text, so its `#` comments look like headings.
        if d.in_front_matter(h.line - 1) {
            continue;
        }
        if h.level == level {
            count += 1;
            if count > 1 && !fm_counts {
                out.push(v(
                    "MD025",
                    h.line,
                    1,
                    format!("multiple h{level} headings (a document may have only one)"),
                ));
            }
        }
    }
    if count == 0 && !fm_counts {
        out.push(v(
            "MD025",
            1,
            1,
            format!("no h{level} heading in the document"),
        ));
    }
    out
}

// ---- MD036 no-emphasis-as-heading --------------------------------------------

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct Md036Options {
    pub punctuation: Option<String>,
}

const DEFAULT_MD036_PUNCTUATION: &str = ".,;:!?。，；：！？";

pub fn md036(d: &Doc, rs: &RuleSet) -> Vec<Violation> {
    let punctuation = rs
        .opt::<Md036Options>("MD036")
        .punctuation
        .unwrap_or_else(|| DEFAULT_MD036_PUNCTUATION.to_owned());
    let mut out = Vec::new();
    for i in 0..d.n() {
        if d.in_front_matter(i) || d.in_code(i) || d.heading[i] > 0 {
            continue;
        }
        let trimmed = d.lines[i].trim();
        if !trimmed.starts_with('*') || !trimmed.ends_with('*') || trimmed.len() < 4 {
            continue;
        }
        let inner = trimmed.trim_matches('*').trim();
        if inner.is_empty() || inner.contains('*') {
            continue;
        }
        if inner
            .chars()
            .last()
            .is_some_and(|c| !punctuation.is_empty() && punctuation.contains(c))
        {
            continue;
        }
        if !starts_own_paragraph(d, i) {
            continue;
        }
        if i + 1 < d.n() && d.lines[i + 1].trim().is_empty() {
            out.push(v("MD036", i + 1, 1, "emphasis used instead of a heading"));
        }
    }
    out
}

// ---- MD041 first-line-heading ------------------------------------------------

pub fn md041(d: &Doc, _rs: &RuleSet) -> Vec<Violation> {
    if d.n() == 0 {
        return Vec::new();
    }
    let mut start = d.fm_end;
    while start < d.n() && d.lines[start].trim().is_empty() {
        start += 1;
    }
    if start >= d.n() {
        return Vec::new();
    }
    let trimmed = d.lines[start].trim();
    if trimmed.starts_with('#')
        || d.heading[start] > 0
        || crate::mdlint::doc::is_setext_underline(trimmed)
    {
        return Vec::new();
    }
    vec![v(
        "MD041",
        start + 1,
        1,
        "first content line is not a top-level heading",
    )]
}

// ---- MD046 code-block-style --------------------------------------------------

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct Md046Options {
    pub style: String,
}

pub fn md046(d: &Doc, rs: &RuleSet) -> Vec<Violation> {
    let style = rs.opt::<Md046Options>("MD046").style;
    let style = if style.is_empty() {
        "consistent".to_owned()
    } else {
        style
    };
    let mut out = Vec::new();
    for i in 0..d.n() {
        if d.in_front_matter(i) || d.in_code(i) {
            continue;
        }
        let line = &d.lines[i];
        if line.starts_with("    ") && !line.trim().is_empty() {
            let prev_blank = i == 0 || d.lines[i - 1].trim().is_empty();
            let prev_list = i > 0 && d.list[i - 1];
            if prev_blank && !prev_list {
                out.push(v(
                    "MD046",
                    i + 1,
                    1,
                    format!("indented code block (style: {style})"),
                ));
            }
        }
    }
    out
}

// ---- MD048 code-fence-style --------------------------------------------------

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct Md048Options {
    pub style: String,
}

pub fn md048(d: &Doc, rs: &RuleSet) -> Vec<Violation> {
    let style = rs.opt::<Md048Options>("MD048").style;
    let style = if style.is_empty() {
        "backtick".to_owned()
    } else {
        style
    };
    let mut out = Vec::new();
    for i in 0..d.n() {
        if !d.fence_lines[i] {
            continue;
        }
        let trimmed = d.lines[i].trim_start_matches(' ');
        let Some(first) = trimmed.chars().next() else {
            continue;
        };
        if style == "tilde" && first != '~' {
            out.push(v(
                "MD048",
                i + 1,
                1,
                "code fence style is backtick (style: tilde)",
            ));
        }
        if style == "backtick" && first == '~' {
            out.push(v(
                "MD048",
                i + 1,
                1,
                "code fence style is tilde (style: backtick)",
            ));
        }
    }
    out
}
