//! One parsed Markdown document: the line view and the masks the rules share.
//!
//! The line masks (front matter, fences, headings, lists, tables) are computed
//! from the raw lines. The AST-level facts (headings with their text, images,
//! links) come from `pulldown-cmark` events, with byte offsets mapped to lines.

use std::ops::Range;

use pulldown_cmark::{CowStr, Event, HeadingLevel, Options, Parser, Tag, TagEnd};

/// A heading found by the parser.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AstHeading {
    pub level: usize,
    /// 1-based line the heading starts on.
    pub line: usize,
    pub text: String,
    pub setext: bool,
}

/// An image or link found by the parser.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AstLink {
    pub line: usize,
    pub destination: String,
    /// Inner text: the alt text for an image, the link text for a link.
    pub text: String,
}

/// The document's line view and masks.
#[derive(Debug, Clone, Default)]
pub struct Doc {
    pub lines: Vec<String>,
    /// Front matter bounds, 0-based, end exclusive. `(0, 0)` when there is none.
    pub fm_start: usize,
    pub fm_end: usize,
    /// `fence[i]`: line `i` is inside a fenced code block.
    pub fence: Vec<bool>,
    /// `fence_lines[i]`: line `i` is a fence delimiter.
    pub fence_lines: Vec<bool>,
    /// `fence_opens[i]`: line `i` opens a fenced code block.
    pub fence_opens: Vec<bool>,
    /// Heading level of the heading anchored at line `i`, or 0.
    pub heading: Vec<usize>,
    pub heading_text: Vec<String>,
    /// Last line of the heading anchored at `i` (the underline, for setext).
    pub heading_end: Vec<usize>,
    /// Line `i` starts an outermost list item.
    pub list: Vec<bool>,
    /// Line `i` is part of a GFM table.
    pub table: Vec<bool>,
    /// The original source, for the parser-based rules.
    pub source: String,
}

/// Splits source into lines, dropping the empty element a trailing newline leaves.
#[must_use]
pub fn split_lines(src: &str) -> Vec<String> {
    let normalized = src.replace("\r\n", "\n");
    let mut lines: Vec<String> = normalized.split('\n').map(str::to_owned).collect();
    if lines.last().is_some_and(String::is_empty) {
        lines.pop();
    }
    lines
}

impl Doc {
    /// Builds the line view and every mask for `src`.
    #[must_use]
    pub fn parse(src: &str) -> Self {
        let mut doc = Self {
            source: src.to_owned(),
            ..Self::default()
        };
        doc.set_lines(split_lines(src));
        doc
    }

    /// Replaces the lines and recomputes every mask.
    pub fn set_lines(&mut self, lines: Vec<String>) {
        self.lines = lines;
        self.mark_front_matter();
        self.mark_fences();
        self.mark_headings();
        self.mark_lists();
        self.mark_tables();
    }

    #[must_use]
    pub fn n(&self) -> usize {
        self.lines.len()
    }

    #[must_use]
    pub fn in_front_matter(&self, i: usize) -> bool {
        i >= self.fm_start && i < self.fm_end
    }

    /// Reports whether line `i` is a fence delimiter. An out-of-range index is "no".
    #[must_use]
    pub fn is_fence_line(&self, i: usize) -> bool {
        self.fence_lines.get(i).copied().unwrap_or(false)
    }

    /// Reports whether line `i` is inside a code block (not the delimiters).
    #[must_use]
    pub fn in_code(&self, i: usize) -> bool {
        self.fence.get(i).copied().unwrap_or(false)
    }

    fn mark_front_matter(&mut self) {
        self.fm_start = 0;
        self.fm_end = 0;
        if self.lines.is_empty() || self.lines[0].trim() != "---" {
            return;
        }
        for (i, line) in self.lines.iter().enumerate().skip(1) {
            let t = line.trim();
            if t == "---" || t == "..." {
                self.fm_start = 1;
                self.fm_end = i;
                return;
            }
        }
    }

    fn mark_fences(&mut self) {
        let n = self.n();
        self.fence = vec![false; n];
        self.fence_lines = vec![false; n];
        self.fence_opens = vec![false; n];
        let mut marker = String::new();
        for i in 0..n {
            if self.in_front_matter(i) {
                continue;
            }
            let line = &self.lines[i];
            let trimmed = line.trim_start_matches(' ');
            let indent = line.len() - trimmed.len();
            if indent > 3 {
                // Four or more spaces is indented code, not a fence. Inside an open
                // block the line is still that block's content.
                if !marker.is_empty() {
                    self.fence[i] = true;
                }
                continue;
            }
            if marker.is_empty() {
                if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
                    let ch = &trimmed[..1];
                    let count = trimmed.len() - trimmed.trim_start_matches(ch).len();
                    if count >= 3 {
                        marker = ch.repeat(count);
                        self.fence_lines[i] = true;
                        self.fence_opens[i] = true;
                    }
                }
                continue;
            }
            if trimmed.starts_with(&marker) {
                self.fence_lines[i] = true;
                marker.clear();
                continue;
            }
            self.fence[i] = true;
        }
    }

    fn mark_headings(&mut self) {
        let n = self.n();
        self.heading = vec![0; n];
        self.heading_text = vec![String::new(); n];
        self.heading_end = vec![0; n];
        for i in 0..n {
            if self.in_front_matter(i) || self.in_code(i) {
                continue;
            }
            let line = self.lines[i].clone();
            let trimmed = line.trim_start_matches(' ');
            if line.len() - trimmed.len() > 3 {
                continue;
            }
            if trimmed.starts_with('#') {
                let level = trimmed.bytes().take_while(|&b| b == b'#').count();
                if level <= 6 {
                    self.heading[i] = level;
                    self.heading_end[i] = i;
                    let rest = trimmed[level..].trim();
                    self.heading_text[i] = rest.trim_start_matches('#').trim().to_owned();
                }
                continue;
            }
            // Setext: the paragraph above an `=` or `-` underline is a heading.
            if i > 0
                && (trimmed.starts_with('=') || trimmed.starts_with('-'))
                && is_setext_underline(trimmed)
            {
                let start = self.setext_start(i);
                if let Some(start) = start.filter(|&s| self.heading[s] == 0) {
                    let level = if trimmed.starts_with('-') { 2 } else { 1 };
                    self.heading[start] = level;
                    self.heading_end[start] = i;
                    let text: Vec<&str> = self.lines[start..i]
                        .iter()
                        .map(|l| l.trim_end_matches([' ', '\t']))
                        .collect();
                    self.heading_text[start] = text.join(" ");
                }
            }
        }
    }

    /// The first line of the paragraph an underline at `underline` closes, if any.
    fn setext_start(&self, underline: usize) -> Option<usize> {
        let mut start = None;
        for j in (0..underline).rev() {
            let line = &self.lines[j];
            let trimmed = line.trim_start_matches(' ');
            if line.trim().is_empty() || self.in_code(j) || self.in_front_matter(j) {
                break;
            }
            if trimmed.starts_with('#')
                || is_fence_delimiter(trimmed)
                || trimmed.starts_with('>')
                || is_list_start(trimmed)
            {
                break;
            }
            start = Some(j);
        }
        start
    }

    fn mark_lists(&mut self) {
        self.list = vec![false; self.n()];
        for i in 0..self.n() {
            if self.in_front_matter(i) || self.in_code(i) {
                continue;
            }
            let line = &self.lines[i];
            let trimmed = line.trim_start_matches(' ');
            if line.len() - trimmed.len() > 3 {
                continue;
            }
            if is_list_start(trimmed) {
                self.list[i] = true;
            }
        }
    }

    fn mark_tables(&mut self) {
        let n = self.n();
        self.table = vec![false; n];
        for i in 0..n.saturating_sub(1) {
            if self.in_front_matter(i) || self.in_code(i) || !self.lines[i].contains('|') {
                continue;
            }
            if !is_table_delimiter(&self.lines[i + 1]) {
                continue;
            }
            let mut start = i;
            while start > 0
                && !self.in_front_matter(start)
                && !self.in_code(start)
                && self.lines[start - 1].contains('|')
                && !self.lines[start - 1].trim().is_empty()
            {
                start -= 1;
            }
            for j in start..n {
                if self.in_code(j)
                    || self.lines[j].trim().is_empty()
                    || !self.lines[j].contains('|')
                {
                    break;
                }
                self.table[j] = true;
            }
        }
    }

    /// Headings the parser sees, outside nothing: every heading in the document.
    #[must_use]
    pub fn ast_headings(&self) -> Vec<AstHeading> {
        let mut out = Vec::new();
        let mut open: Option<(usize, usize, String, bool)> = None;
        for (event, range) in parser_events(&self.source) {
            match event {
                Event::Start(Tag::Heading { level, .. }) => {
                    let setext = !self.source[range.start..].trim_start().starts_with('#');
                    open = Some((
                        level_number(level),
                        self.line_of(range.start),
                        String::new(),
                        setext,
                    ));
                }
                Event::Text(t) | Event::Code(t) => {
                    if let Some((_, _, text, _)) = open.as_mut() {
                        text.push_str(&t);
                    }
                }
                Event::End(TagEnd::Heading(_)) => {
                    if let Some((level, line, text, setext)) = open.take() {
                        out.push(AstHeading {
                            level,
                            line,
                            text,
                            setext,
                        });
                    }
                }
                _ => {}
            }
        }
        out
    }

    /// Images and links the parser sees, with their destinations and inner text.
    #[must_use]
    pub fn ast_links(&self) -> (Vec<AstLink>, Vec<AstLink>) {
        let mut images = Vec::new();
        let mut links = Vec::new();
        let mut open: Vec<(bool, AstLink)> = Vec::new();
        for (event, range) in parser_events(&self.source) {
            match event {
                Event::Start(Tag::Image { dest_url, .. }) => {
                    open.push((true, self.new_link(&range, &dest_url)));
                }
                Event::Start(Tag::Link { dest_url, .. }) => {
                    open.push((false, self.new_link(&range, &dest_url)));
                }
                Event::Text(t) | Event::Code(t) => {
                    if let Some((_, link)) = open.last_mut() {
                        link.text.push_str(&t);
                    }
                }
                Event::End(TagEnd::Image) => {
                    if let Some((_, link)) = open.pop() {
                        images.push(link);
                    }
                }
                Event::End(TagEnd::Link) => {
                    if let Some((_, link)) = open.pop() {
                        links.push(link);
                    }
                }
                _ => {}
            }
        }
        (images, links)
    }

    fn new_link(&self, range: &Range<usize>, dest: &CowStr<'_>) -> AstLink {
        AstLink {
            line: self.line_of(range.start),
            destination: dest.to_string(),
            text: String::new(),
        }
    }

    /// 1-based line containing byte `offset`.
    #[must_use]
    pub fn line_of(&self, offset: usize) -> usize {
        let end = offset.min(self.source.len());
        1 + self.source.as_bytes()[..end]
            .iter()
            .filter(|&&b| b == b'\n')
            .count()
    }
}

/// The parser configured for GitHub-flavoured Markdown.
pub fn parser_events(src: &str) -> impl Iterator<Item = (Event<'_>, Range<usize>)> {
    let options =
        Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TASKLISTS;
    let fm_end = front_matter_end(src);
    Parser::new_ext(src, options)
        .into_offset_iter()
        .filter(move |(_, range)| range.start >= fm_end)
}

/// Byte offset just past a leading YAML front matter block (`---` ... `---` or `...`), or 0
/// when the document has none. Without this, the parser reads the closing `---` as a setext
/// underline and the front matter as a heading.
#[must_use]
pub fn front_matter_end(src: &str) -> usize {
    let mut lines = src.split_inclusive('\n');
    let Some(first) = lines.next() else { return 0 };
    if first.trim_end() != "---" {
        return 0;
    }
    let mut offset = first.len();
    for line in lines {
        offset += line.len();
        let t = line.trim_end();
        if t == "---" || t == "..." {
            return offset;
        }
    }
    0
}

fn level_number(level: HeadingLevel) -> usize {
    match level {
        HeadingLevel::H1 => 1,
        HeadingLevel::H2 => 2,
        HeadingLevel::H3 => 3,
        HeadingLevel::H4 => 4,
        HeadingLevel::H5 => 5,
        HeadingLevel::H6 => 6,
    }
}

/// Reports whether a trimmed line is a setext underline (`===` or `---`).
#[must_use]
pub fn is_setext_underline(s: &str) -> bool {
    let Some(first) = s.chars().next() else {
        return false;
    };
    if first != '=' && first != '-' {
        return false;
    }
    s.chars().all(|c| c == first)
}

/// Reports whether a line opens or closes a fenced code block.
#[must_use]
pub fn is_fence_delimiter(trimmed: &str) -> bool {
    let Some(first) = trimmed.chars().next() else {
        return false;
    };
    if first != '`' && first != '~' {
        return false;
    }
    trimmed.chars().take_while(|&c| c == first).count() >= 3
}

/// Reports whether a trimmed line starts a list item (`- `, `1. `, and so on).
#[must_use]
pub fn is_list_start(trimmed: &str) -> bool {
    let bytes = trimmed.as_bytes();
    let Some(&first) = bytes.first() else {
        return false;
    };
    match first {
        b'-' | b'*' | b'+' => bytes.len() > 1 && bytes[1] == b' ',
        b'0'..=b'9' => {
            let digits = bytes.iter().take_while(|b| b.is_ascii_digit()).count();
            let rest = &trimmed[digits..];
            rest.starts_with(". ") || rest.starts_with(") ")
        }
        _ => false,
    }
}

/// Reports whether a line is a GFM table delimiter row (`| --- | :-: |`).
#[must_use]
pub fn is_table_delimiter(line: &str) -> bool {
    let trimmed = line.trim();
    if !trimmed.contains('-') {
        return false;
    }
    let cells: Vec<&str> = trimmed.split('|').collect();
    if cells.len() < 2 {
        return false;
    }
    cells
        .iter()
        .map(|c| c.trim())
        .filter(|c| !c.is_empty())
        .all(|c| c.trim_matches(['-', ':']).is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn front_matter_is_not_parsed_as_a_heading() {
        assert_eq!(front_matter_end("---\ntitle: x\n---\n# H\n"), 17);
        assert_eq!(front_matter_end("# H\n"), 0);
    }

    #[test]
    fn front_matter_and_fences_are_masked() {
        let doc = Doc::parse("---\ntitle: x\n---\n\n```rust\nlet x = 1;\n```\n");
        assert!(doc.in_front_matter(1));
        assert!(doc.in_code(5));
        assert!(doc.fence_opens[4]);
        assert!(!doc.fence_opens[6]);
    }

    #[test]
    fn setext_heading_is_anchored_at_its_first_line() {
        let doc = Doc::parse("Intro\n\nTitle\n=====\n");
        assert_eq!(doc.heading[2], 1);
        assert_eq!(doc.heading_end[2], 3);
        assert_eq!(doc.heading_text[2], "Title");
    }

    #[test]
    fn parser_reports_headings_and_links_with_lines() {
        let doc = Doc::parse("# One\n\n![pic](img/logo.png)\n\n[here](x.md)\n");
        let headings = doc.ast_headings();
        assert_eq!(headings[0].text, "One");
        let (images, links) = doc.ast_links();
        assert_eq!(images[0].destination, "img/logo.png");
        assert_eq!(links[0].text, "here");
    }

    #[test]
    fn list_and_table_detection() {
        assert!(is_list_start("- item"));
        assert!(!is_list_start("---"));
        assert!(is_list_start("12. item"));
        assert!(is_table_delimiter("|---|:--:|"));
        assert!(!is_table_delimiter("plain text"));
    }
}
