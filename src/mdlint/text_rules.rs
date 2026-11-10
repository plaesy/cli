//! Line-level rules: decidable from the raw lines and the masks, with no parser.

use serde::Deserialize;

use super::doc::Doc;
use super::{RuleSet, Violation};

fn v(rule: &'static str, line: usize, column: usize, message: impl Into<String>) -> Violation {
    Violation {
        rule,
        line,
        column,
        message: message.into(),
    }
}

// ---- MD009 no-trailing-spaces ------------------------------------------------

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct Md009Options {
    pub br_spaces: Option<usize>,
    pub list_item_empty_lines: Option<bool>,
    pub code_blocks: Option<bool>,
}

pub fn md009(d: &Doc, rs: &RuleSet) -> Vec<Violation> {
    let options = rs.opt::<Md009Options>("MD009");
    let allowed = options.br_spaces.unwrap_or(2);
    let code_blocks = options.code_blocks.unwrap_or(false);
    let mut out = Vec::new();
    for (i, line) in d.lines.iter().enumerate() {
        if d.in_front_matter(i) || (d.in_code(i) && !code_blocks) {
            continue;
        }
        let trimmed = line.trim_end_matches([' ', '\t']);
        let trailing = line.len() - trimmed.len();
        if trailing == 0 || trailing <= allowed {
            continue;
        }
        out.push(v(
            "MD009",
            i + 1,
            trimmed.len() + 1,
            format!("{trailing} trailing space(s)"),
        ));
    }
    out
}

// ---- MD010 no-hard-tabs --------------------------------------------------------

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct Md010Options {
    pub spaces_per_tab: Option<usize>,
    #[serde(rename = "code_blocks")]
    pub include_code_blocks: Option<bool>,
}

pub fn md010(d: &Doc, rs: &RuleSet) -> Vec<Violation> {
    let include_code = rs
        .opt::<Md010Options>("MD010")
        .include_code_blocks
        .unwrap_or(true);
    let mut out = Vec::new();
    for (i, line) in d.lines.iter().enumerate() {
        if d.in_front_matter(i) || (d.in_code(i) && !include_code) {
            continue;
        }
        if line.contains('\t') {
            out.push(v("MD010", i + 1, 1, "hard tab"));
        }
    }
    out
}

// ---- MD012 no-multiple-blanks ------------------------------------------------

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct Md012Options {
    pub maximum: Option<usize>,
}

pub fn md012(d: &Doc, rs: &RuleSet) -> Vec<Violation> {
    let max = rs.opt::<Md012Options>("MD012").maximum.unwrap_or(1);
    let mut out = Vec::new();
    let (mut run, mut run_start) = (0usize, 0usize);
    for (i, line) in d.lines.iter().enumerate() {
        if d.in_front_matter(i) {
            continue;
        }
        if line.trim().is_empty() {
            if run == 0 {
                run_start = i;
            }
            run += 1;
            continue;
        }
        if run > max {
            out.push(v(
                "MD012",
                run_start + 1,
                1,
                format!("{run} consecutive blank lines (maximum {max})"),
            ));
        }
        run = 0;
    }
    out
}

// ---- MD013 line-length -------------------------------------------------------

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct Md013Options {
    pub line_length: Option<usize>,
    pub heading_line_length: Option<usize>,
    pub code_block_line_length: Option<usize>,
    pub code_blocks: Option<bool>,
    pub headings: Option<bool>,
    pub tables: Option<bool>,
    pub strict: Option<bool>,
}

/// The limit that applies to line `i`, and whether MD013 applies to it at all.
/// Shared by the checker so the decision lives in one place.
pub fn md013_limit(d: &Doc, i: usize, o: &Md013Options) -> Option<usize> {
    if d.in_front_matter(i) {
        return None;
    }
    let line_length = o.line_length.unwrap_or(80);
    let heading_length = o.heading_line_length.unwrap_or(line_length);
    let code_length = o.code_block_line_length.unwrap_or(line_length);
    if d.heading[i] > 0 {
        return o.headings.unwrap_or(true).then_some(heading_length);
    }
    if d.in_code(i) {
        return o.code_blocks.unwrap_or(true).then_some(code_length);
    }
    if d.table[i] {
        return o.tables.unwrap_or(true).then_some(line_length);
    }
    Some(line_length)
}

pub fn md013(d: &Doc, rs: &RuleSet) -> Vec<Violation> {
    let o = rs.opt::<Md013Options>("MD013");
    let mut out = Vec::new();
    for (i, line) in d.lines.iter().enumerate() {
        let Some(limit) = md013_limit(d, i, &o) else {
            continue;
        };
        let chars: Vec<char> = line.chars().collect();
        if chars.len() > limit {
            let shown = if chars.len() > limit + 20 {
                limit + 20
            } else {
                chars.len()
            };
            let snippet: String = chars[..shown].iter().collect();
            out.push(v(
                "MD013",
                i + 1,
                1,
                format!(
                    "line {} characters (limit {limit}): {snippet}…",
                    chars.len()
                ),
            ));
        }
    }
    out
}

// ---- MD022 blanks-around-headings --------------------------------------------

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct Md022Options {
    pub lines_above: Option<usize>,
    pub lines_below: Option<usize>,
}

pub fn md022(d: &Doc, rs: &RuleSet) -> Vec<Violation> {
    let o = rs.opt::<Md022Options>("MD022");
    let (above, below) = (o.lines_above.unwrap_or(1), o.lines_below.unwrap_or(1));
    let mut out = Vec::new();
    for i in 0..d.n() {
        if d.heading[i] == 0 {
            continue;
        }
        if above > 0 {
            let first_content = i == 0 || (d.fm_end > 0 && i == d.fm_end);
            if !first_content && !d.lines[i - 1].trim().is_empty() {
                out.push(v("MD022", i + 1, 1, "no blank line above the heading"));
            }
        }
        let end = i.max(d.heading_end[i]);
        if below > 0
            && end + 1 < d.n()
            && !d.lines[end + 1].trim().is_empty()
            && d.heading[end + 1] == 0
        {
            out.push(v("MD022", i + 1, 1, "no blank line below the heading"));
        }
    }
    out
}

// ---- MD031 blanks-around-fences ----------------------------------------------

pub fn md031(d: &Doc, _rs: &RuleSet) -> Vec<Violation> {
    let mut out = Vec::new();
    for i in 0..d.n() {
        if !d.fence_lines[i] {
            continue;
        }
        if i > 0 && !d.fence_lines[i - 1] && !d.lines[i - 1].trim().is_empty() && !d.in_code(i - 1)
        {
            out.push(v(
                "MD031",
                i + 1,
                1,
                "fenced code block should be preceded by a blank line",
            ));
            continue;
        }
        if d.fence_opens[i] {
            continue;
        }
        if i + 1 < d.n() && !d.fence_lines[i + 1] && !d.lines[i + 1].trim().is_empty() {
            out.push(v(
                "MD031",
                i + 1,
                1,
                "fenced code block should be followed by a blank line",
            ));
        }
    }
    out
}

// ---- MD032 blanks-around-lists -----------------------------------------------

/// Whether a raw line is indented, which makes it a continuation of the block above.
pub fn is_indented(line: &str) -> bool {
    line.starts_with("  ") || line.starts_with('\t')
}

pub fn md032(d: &Doc, _rs: &RuleSet) -> Vec<Violation> {
    let mut out = Vec::new();
    for i in 0..d.n() {
        if !d.list[i] {
            continue;
        }
        if i > 0 {
            let prev = &d.lines[i - 1];
            let continues = d.list[i - 1]
                || is_indented(prev)
                || prev.trim_end_matches(' ').ends_with('>')
                || d.fence[i - 1]
                || d.fence_lines[i - 1];
            if !continues && !prev.trim().is_empty() && !d.in_code(i - 1) {
                out.push(v(
                    "MD032",
                    i + 1,
                    1,
                    "list should be preceded by a blank line",
                ));
            }
        }
        if i + 1 < d.n() {
            let next_line = &d.lines[i + 1];
            let next = next_line.trim();
            let follows = d.list[i + 1]
                || is_indented(next_line)
                || next.starts_with("- ")
                || next.starts_with("* ")
                || next.starts_with("> ")
                || d.fence_lines[i + 1]
                || d.fence[i + 1];
            if follows {
                continue;
            }
            if !next.is_empty() {
                out.push(v(
                    "MD032",
                    i + 1,
                    1,
                    "list should be followed by a blank line",
                ));
            }
        }
    }
    out
}

// ---- MD040 fenced-code-language ----------------------------------------------

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct Md040Options {
    pub allowed_languages: Option<Vec<String>>,
    pub language_only: Option<bool>,
}

/// The language word and the rest of a fence's info string.
fn fence_info(line: &str) -> (String, String) {
    let trimmed = line.trim_start_matches(' ');
    let rest = trimmed.trim_start_matches(['`', '~']).trim();
    if rest.is_empty() {
        return (String::new(), String::new());
    }
    let mut parts = rest.splitn(2, char::is_whitespace);
    let lang = parts.next().unwrap_or("").to_owned();
    let tail = parts.next().unwrap_or("").trim().to_owned();
    (lang, tail)
}

pub fn md040(d: &Doc, rs: &RuleSet) -> Vec<Violation> {
    let o = rs.opt::<Md040Options>("MD040");
    let language_only = o.language_only.unwrap_or(true);
    let allowed: Option<Vec<String>> = o
        .allowed_languages
        .map(|list| list.iter().map(|l| l.to_lowercase()).collect());
    let mut out = Vec::new();
    for (i, line) in d.lines.iter().enumerate() {
        if !d.fence_opens[i] {
            continue;
        }
        let (lang, rest) = fence_info(line);
        if lang.is_empty() {
            out.push(v("MD040", i + 1, 1, "fenced code block has no language"));
            continue;
        }
        if let Some(allowed) = &allowed {
            if !allowed.contains(&lang.to_lowercase()) {
                out.push(v(
                    "MD040",
                    i + 1,
                    1,
                    format!("language {lang} is not in allowed_languages"),
                ));
                continue;
            }
        }
        if language_only && !rest.is_empty() {
            out.push(v(
                "MD040",
                i + 1,
                1,
                format!("language_only: info string must be just the language, got {rest}"),
            ));
        }
    }
    out
}

// ---- MD047 single-trailing-newline -------------------------------------------

pub fn md047(d: &Doc, _rs: &RuleSet) -> Vec<Violation> {
    if d.source.is_empty() {
        return Vec::new();
    }
    if d.source.ends_with("\n\n") {
        return vec![v("MD047", d.n(), 1, "multiple trailing newlines")];
    }
    if !d.source.ends_with('\n') {
        return vec![v("MD047", d.n() + 1, 1, "no newline at end of file")];
    }
    Vec::new()
}

// ---- MD058 blanks-around-tables ----------------------------------------------

pub fn md058(d: &Doc, _rs: &RuleSet) -> Vec<Violation> {
    let mut out = Vec::new();
    let mut seen = std::collections::BTreeSet::new();
    for i in 0..d.n() {
        if !d.table[i] || seen.contains(&i) {
            continue;
        }
        if i > 0 && !d.table[i - 1] && !d.lines[i - 1].trim().is_empty() && !d.in_code(i - 1) {
            out.push(v(
                "MD058",
                i + 1,
                1,
                "table should be preceded by a blank line",
            ));
        }
        let mut j = i;
        while j + 1 < d.n() && d.table[j + 1] {
            j += 1;
        }
        seen.insert(j);
        if j + 1 < d.n() && !d.lines[j + 1].trim().is_empty() && !d.is_fence_line(j + 1) {
            out.push(v(
                "MD058",
                j + 1,
                1,
                "table should be followed by a blank line",
            ));
        }
    }
    out
}

// ---- MD004 unordered-list-style ----------------------------------------------

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct Md004Options {
    pub style: String,
}

pub fn md004(d: &Doc, rs: &RuleSet) -> Vec<Violation> {
    let style = rs.opt::<Md004Options>("MD004").style;
    let style = if style.is_empty() {
        "consistent".to_owned()
    } else {
        style
    };
    let mut first: Option<char> = None;
    let mut out = Vec::new();
    for (i, line) in d.lines.iter().enumerate() {
        if d.in_front_matter(i) || d.in_code(i) {
            continue;
        }
        let trimmed = line.trim_start_matches(' ');
        let mut chars = trimmed.chars();
        let (Some(marker), Some(space)) = (chars.next(), chars.next()) else {
            continue;
        };
        if !matches!(marker, '-' | '*' | '+') || space != ' ' {
            continue;
        }
        if style == "consistent" {
            let expected = *first.get_or_insert(marker);
            if marker != expected {
                out.push(v(
                    "MD004",
                    i + 1,
                    1,
                    format!("unordered list marker {marker} is inconsistent with {expected} (style: consistent)"),
                ));
            }
            continue;
        }
        let wanted = match style.as_str() {
            "dash" => "-",
            "asterisk" => "*",
            "plus" => "+",
            other => other,
        };
        if marker.to_string() != wanted {
            out.push(v(
                "MD004",
                i + 1,
                1,
                format!("unordered list marker {marker} (style: {style})"),
            ));
        }
    }
    out
}

// ---- MD007 unordered-list-indent ---------------------------------------------

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct Md007Options {
    pub indent: Option<usize>,
    pub start_indented: Option<bool>,
    pub start_indent: Option<usize>,
}

/// Whether a line is an unordered list item whose indentation MD007 objects to.
/// Returns the leading-space count and the verdict.
#[must_use]
pub fn md007_offender(
    line: &str,
    indent: usize,
    start_indented: bool,
    start_indent: usize,
) -> (usize, bool) {
    if indent < 1 {
        return (0, false);
    }
    let trimmed = line.trim_start_matches(' ');
    let lead = line.len() - trimmed.len();
    if lead == 0 || trimmed.len() < 2 {
        return (lead, false);
    }
    let bytes = trimmed.as_bytes();
    if !matches!(bytes[0], b'-' | b'*' | b'+') || bytes[1] != b' ' {
        return (lead, false);
    }
    let want = if lead <= start_indent && start_indented {
        start_indent
    } else {
        indent
    };
    if lead % indent == 0 || lead % want == 0 {
        return (lead, false);
    }
    (lead, true)
}

pub fn md007(d: &Doc, rs: &RuleSet) -> Vec<Violation> {
    let o = rs.opt::<Md007Options>("MD007");
    let indent = o.indent.unwrap_or(2);
    let start_indented = o.start_indented.unwrap_or(false);
    let start_indent = o.start_indent.unwrap_or(indent);
    let mut out = Vec::new();
    let mut list_stack: Vec<(usize, bool)> = Vec::new();
    for (i, line) in d.lines.iter().enumerate() {
        if d.in_front_matter(i) || d.in_code(i) {
            continue;
        }

        let trimmed = line.trim_start_matches(' ');
        if trimmed.is_empty() {
            continue;
        }
        let lead = line.len() - trimmed.len();
        let bytes = trimmed.as_bytes();
        let unordered =
            bytes.len() >= 2 && matches!(bytes[0], b'-' | b'*' | b'+') && bytes[1] == b' ';
        let ordered = bytes
            .iter()
            .position(|byte| *byte == b'.' || *byte == b')')
            .is_some_and(|marker| {
                marker > 0
                    && bytes[marker + 1..].starts_with(b" ")
                    && bytes[..marker].iter().all(u8::is_ascii_digit)
            });

        if unordered || ordered {
            while list_stack
                .last()
                .is_some_and(|(parent_indent, _)| *parent_indent >= lead)
            {
                list_stack.pop();
            }

            // MD007 does not govern nested lists when any parent is ordered: marker width adds
            // real indentation (for example, three spaces below `1. item`).
            let has_ordered_parent = list_stack.iter().any(|(_, parent_ordered)| *parent_ordered);
            if unordered && !has_ordered_parent {
                let (lead, offending) = md007_offender(line, indent, start_indented, start_indent);
                if offending {
                    out.push(v(
                        "MD007",
                        i + 1,
                        lead + 1,
                        format!(
                            "unordered list indentation {lead} spaces (expected a multiple of {indent})"
                        ),
                    ));
                }
            }
            list_stack.push((lead, ordered));
        } else if lead == 0 {
            // A new unindented block ends the prior list, so a later list starts a fresh context.
            list_stack.clear();
        }
    }
    out
}

// ---- MD029 ordered-list-prefix -----------------------------------------------

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct Md029Options {
    pub style: String,
}

/// One ordered list: the lines its markers sit on and the numbers they carry.
#[derive(Debug, Clone, Default)]
pub struct ListRun {
    pub indent: usize,
    pub lines: Vec<usize>,
    pub nums: Vec<usize>,
}

fn ordered_marker(line: &str) -> Option<usize> {
    let trimmed = line.trim_start_matches(' ');
    let digits = trimmed.bytes().take_while(u8::is_ascii_digit).count();
    if digits == 0 || digits >= trimmed.len() {
        return None;
    }
    let delim = trimmed.as_bytes()[digits];
    if (delim != b'.' && delim != b')') || trimmed.as_bytes().get(digits + 1) != Some(&b' ') {
        return None;
    }
    trimmed[..digits].parse().ok()
}

/// Groups the document's ordered-list markers into lists, the way the rule reads them.
#[must_use]
pub fn ordered_runs(d: &Doc) -> Vec<ListRun> {
    let mut stack: Vec<ListRun> = Vec::new();
    let mut out: Vec<ListRun> = Vec::new();
    for i in 0..d.n() {
        if d.in_front_matter(i) || d.in_code(i) {
            out.extend(stack.drain(..).rev());
            continue;
        }
        let line = &d.lines[i];
        let indent = line.len() - line.trim_start_matches(' ').len();
        if let Some(num) = ordered_marker(line) {
            while stack.last().is_some_and(|top| top.indent > indent) {
                out.push(stack.pop().expect("checked non-empty"));
            }
            if let Some(top) = stack.last_mut().filter(|top| top.indent == indent) {
                top.lines.push(i + 1);
                top.nums.push(num);
                continue;
            }
            stack.push(ListRun {
                indent,
                lines: vec![i + 1],
                nums: vec![num],
            });
            continue;
        }
        let continues = stack.last().is_some_and(|top| indent > top.indent);
        if line.trim().is_empty() || continues {
            continue;
        }
        out.extend(stack.drain(..).rev());
    }
    out.extend(stack.drain(..).rev());
    out
}

fn check_list_run(style: &str, run: &ListRun) -> Vec<Violation> {
    let report = |i: usize, message: String| v("MD029", run.lines[i], 1, message);
    let mut out = Vec::new();
    match style {
        "one" => {
            for (i, &n) in run.nums.iter().enumerate() {
                if n != 1 {
                    out.push(report(
                        i,
                        "ordered list item must start at 1 (style: one)".into(),
                    ));
                }
            }
        }
        "zero" => {
            for (i, &n) in run.nums.iter().enumerate() {
                if n != 0 {
                    out.push(report(
                        i,
                        "ordered list item must start at 0 (style: zero)".into(),
                    ));
                }
            }
        }
        "ordered" => {
            for (i, &n) in run.nums.iter().enumerate() {
                if n != i + 1 {
                    out.push(report(
                        i,
                        format!(
                            "ordered list item numbered {n}, expected {} (style: ordered)",
                            i + 1
                        ),
                    ));
                }
            }
        }
        "one_or_ordered" => {
            let all_one = run.nums.iter().all(|&n| n == 1);
            let increments = run.nums.iter().enumerate().all(|(i, &n)| n == i + 1);
            if all_one || increments {
                return out;
            }
            for (i, &n) in run.nums.iter().enumerate() {
                if n != 1 && n != i + 1 {
                    out.push(report(
                        i,
                        format!("ordered list item numbered {n} (style: one_or_ordered)"),
                    ));
                }
            }
        }
        other => {
            for (i, &n) in run.nums.iter().enumerate() {
                if n > 1 {
                    out.push(report(
                        i,
                        format!("ordered list item numbered {n} (style: {other})"),
                    ));
                }
            }
        }
    }
    out
}

pub fn md029(d: &Doc, rs: &RuleSet) -> Vec<Violation> {
    let style = rs.opt::<Md029Options>("MD029").style;
    let style = if style.is_empty() {
        "one_or_ordered".to_owned()
    } else {
        style
    };
    ordered_runs(d)
        .iter()
        .flat_map(|run| check_list_run(&style, run))
        .collect()
}

// ---- MD026 no-trailing-punctuation-in-heading --------------------------------

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct Md026Options {
    pub punctuation: Option<String>,
}

pub fn md026(d: &Doc, rs: &RuleSet) -> Vec<Violation> {
    let punctuation = rs
        .opt::<Md026Options>("MD026")
        .punctuation
        .unwrap_or_else(|| ".,;:!。，；：！".to_owned());
    if punctuation.is_empty() {
        return Vec::new();
    }
    let mut out = Vec::new();
    for i in 0..d.n() {
        if d.heading[i] == 0 {
            continue;
        }
        let text = d.heading_text[i].trim_matches('#').trim();
        let Some(last) = text.chars().last() else {
            continue;
        };
        if !punctuation.contains(last) {
            continue;
        }
        if text.chars().all(|c| punctuation.contains(c)) {
            continue;
        }
        out.push(v("MD026", i + 1, 1, "heading ends with punctuation"));
    }
    out
}

/// Reports whether `i` starts its own paragraph, which is what MD036 asks about.
pub fn starts_own_paragraph(d: &Doc, i: usize) -> bool {
    if i == 0 {
        return true;
    }
    if d.heading[i - 1] > 0 || d.fence_lines[i - 1] || d.table[i - 1] {
        return true;
    }
    let prev = d.lines[i - 1].trim();
    if prev.is_empty() || prev.starts_with('>') {
        return true;
    }
    matches!(prev, "---" | "***" | "___")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mdlint::RuleSet;

    fn run(rule: fn(&Doc, &RuleSet) -> Vec<Violation>, src: &str) -> Vec<Violation> {
        rule(&Doc::parse(src), &RuleSet::default())
    }

    #[test]
    fn trailing_spaces_and_hard_tabs_are_reported() {
        assert_eq!(run(md009, "text   \n").len(), 1);
        assert_eq!(run(md010, "a\tb\n").len(), 1);
    }

    #[test]
    fn trailing_spaces_in_code_blocks_are_ignored_by_default() {
        assert!(run(md009, "```text\ncode   \n```\n").is_empty());
    }

    #[test]
    fn md009_preserves_two_space_hard_breaks_by_default() {
        assert!(run(md009, "line  \n").is_empty());
        assert_eq!(run(md009, "line   \n").len(), 1);
    }

    #[test]
    fn ordered_list_one_or_ordered_accepts_all_ones_and_counting() {
        assert!(run(md029, "1. a\n1. b\n").is_empty());
        assert!(run(md029, "1. a\n2. b\n3. c\n").is_empty());
        assert_eq!(run(md029, "1. a\n5. b\n").len(), 1);
    }

    #[test]
    fn missing_language_on_an_opening_fence_is_reported_once() {
        assert_eq!(run(md040, "```\ncode\n```\n").len(), 1);
    }

    #[test]
    fn list_indent_offender_uses_the_configured_width() {
        assert!(!md007_offender("  - x", 2, false, 2).1);
        assert!(md007_offender("   - x", 2, false, 2).1);
    }

    #[test]
    fn md007_respects_indentation_under_ordered_lists() {
        assert!(run(md007, "1. parent\n   - nested\n      - deeper\n").is_empty());
        assert_eq!(run(md007, "- parent\n   - nested\n").len(), 1);
    }

    #[test]
    fn heading_with_trailing_punctuation_is_reported() {
        assert_eq!(run(md026, "# Title.\n").len(), 1);
        assert!(run(md026, "# Title\n").is_empty());
    }
}
