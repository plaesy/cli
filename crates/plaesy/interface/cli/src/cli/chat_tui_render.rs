//! Turns transcript lines into the rows the log draws. Markdown tables are aligned in columns,
//! diff blocks are coloured, rules and quotes are drawn, and every line is wrapped to the width of
//! the log. The scroll position therefore counts rows the person can see.

use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};

use super::{inline_spans, styled_line};

/// Narrowest a table column gets when the table is squeezed to fit.
const MIN_COLUMN: usize = 3;
/// Widest a table column gets before its text is cut with `…`.
const MAX_COLUMN: usize = 40;
/// Longest a horizontal rule is drawn.
const MAX_RULE: usize = 60;

/// The rows for `lines` in a log `width` columns wide.
pub fn render(lines: &[String], width: usize) -> Vec<Line<'static>> {
    logical(lines, width)
        .into_iter()
        .flat_map(|line| wrap(line, width))
        .collect()
}

/// One line per input line, before wrapping. Tables are the exception: one table is several lines.
fn logical(lines: &[String], width: usize) -> Vec<Line<'static>> {
    let mut out = Vec::with_capacity(lines.len());
    // Inside a fenced block: whether the block is a diff. `None` outside any block.
    let mut diff: Option<bool> = None;
    let mut index = 0;
    while index < lines.len() {
        let line = lines[index].as_str();
        let trimmed = line.trim_start();
        if let Some(info) = trimmed.strip_prefix("```") {
            let language = info.trim();
            let opening = diff.is_none();
            diff = match diff {
                Some(_) => None,
                None => Some(matches!(language, "diff" | "patch")),
            };
            if opening && !language.is_empty() {
                // The language is shown as a label above the block.
                out.push(
                    Line::from(format!("─── {language}")).style(Style::new().fg(Color::DarkGray)),
                );
            } else {
                out.push(green(line));
            }
            index += 1;
        } else if let Some(is_diff) = diff {
            out.push(if is_diff {
                diff_line(line)
            } else {
                green(line)
            });
            index += 1;
        } else if trimmed.starts_with('|') {
            let end = table_end(lines, index);
            out.extend(table(&lines[index..end], width));
            index = end;
        } else {
            out.push(text_line(line, width));
            index += 1;
        }
    }
    out
}

fn green(line: &str) -> Line<'static> {
    Line::from(line.to_owned()).style(Style::new().fg(Color::Green))
}

/// A line of a diff block: additions green, removals red, hunk headers cyan, file headers bold.
fn diff_line(line: &str) -> Line<'static> {
    let style = if line.starts_with("+++")
        || line.starts_with("---")
        || line.starts_with("diff ")
        || line.starts_with("index ")
    {
        Style::new().bold()
    } else if line.starts_with("@@") {
        Style::new().fg(Color::Cyan)
    } else if line.starts_with('+') {
        Style::new().fg(Color::Green)
    } else if line.starts_with('-') {
        Style::new().fg(Color::Red)
    } else {
        Style::new()
    };
    Line::from(line.to_owned()).style(style)
}

/// A line outside a block: a horizontal rule, or the line as `styled_line` shows it.
fn text_line(line: &str, width: usize) -> Line<'static> {
    if is_rule(line) {
        return Line::from("─".repeat(width.min(MAX_RULE))).style(Style::new().fg(Color::DarkGray));
    }
    styled_line(line)
}

fn is_rule(line: &str) -> bool {
    let text = line.trim();
    let Some(first) = text.chars().next() else {
        return false;
    };
    text.len() >= 3 && matches!(first, '-' | '*' | '_') && text.chars().all(|c| c == first)
}

/// The rows of the table starting at `start`: consecutive lines that begin with a pipe.
fn table_end(lines: &[String], start: usize) -> usize {
    start
        + lines[start..]
            .iter()
            .take_while(|line| line.trim_start().starts_with('|'))
            .count()
}

/// A markdown table as rows of aligned columns. A second row made of dashes is the header rule.
/// A table too wide for the log is shown as its plain lines.
fn table(raw: &[String], width: usize) -> Vec<Line<'static>> {
    let rows: Vec<Vec<String>> = raw.iter().map(|row| cells(row)).collect();
    let has_rule = rows.len() >= 2 && is_rule_row(&rows[1]);
    let columns = rows.iter().map(Vec::len).max().unwrap_or(0);
    let Some(widths) = column_widths(&rows, has_rule, columns, width) else {
        return raw.iter().map(|line| Line::from(line.clone())).collect();
    };
    rows.iter()
        .enumerate()
        .map(|(index, row)| {
            if has_rule && index == 1 {
                return rule_row(&widths);
            }
            let header = has_rule && index == 0;
            let mut spans = vec![Span::raw("│ ")];
            for (column, cell_width) in widths.iter().enumerate() {
                if column > 0 {
                    spans.push(Span::raw(" │ "));
                }
                let cell = row.get(column).map_or("", String::as_str);
                spans.extend(cell_spans(cell, *cell_width, header));
            }
            spans.push(Span::raw(" │"));
            Line::from(spans)
        })
        .collect()
}

/// The width of each column: its widest cell, shrunk from the widest column until the table fits.
/// `None` when even the narrowest columns do not fit.
fn column_widths(
    rows: &[Vec<String>],
    has_rule: bool,
    columns: usize,
    width: usize,
) -> Option<Vec<usize>> {
    if columns == 0 {
        return None;
    }
    let mut widths = vec![1; columns];
    for (index, row) in rows.iter().enumerate() {
        if has_rule && index == 1 {
            continue;
        }
        for (column, cell) in row.iter().enumerate() {
            widths[column] = widths[column].max(cell_width(cell).min(MAX_COLUMN));
        }
    }
    // Each column adds its width and three bars (` │ `), plus one bar at the start of the row.
    while 3 * columns + 1 + widths.iter().sum::<usize>() > width {
        let widest = (0..columns).max_by_key(|&column| widths[column])?;
        if widths[widest] <= MIN_COLUMN {
            return None;
        }
        widths[widest] -= 1;
    }
    Some(widths)
}

fn rule_row(widths: &[usize]) -> Line<'static> {
    let body: Vec<String> = widths.iter().map(|w| "─".repeat(w + 2)).collect();
    Line::from(format!("├{}┤", body.join("┼"))).style(Style::new().fg(Color::DarkGray))
}

/// One cell, padded with spaces to `width` columns, or cut to fit. The header is bold.
fn cell_spans(cell: &str, width: usize, header: bool) -> Vec<Span<'static>> {
    let plain = cell_plain(cell);
    let mut spans = if text_width(&plain) > width {
        vec![Span::raw(cut(&plain, width))]
    } else {
        let mut spans = inline_spans(cell);
        spans.push(Span::raw(" ".repeat(width - text_width(&plain))));
        spans
    };
    if header {
        for span in &mut spans {
            span.style = span.style.bold();
        }
    }
    spans
}

/// The cells of a row, without the pipes at either end.
fn cells(row: &str) -> Vec<String> {
    let inner = row.trim();
    let inner = inner.strip_prefix('|').unwrap_or(inner);
    let inner = inner.strip_suffix('|').unwrap_or(inner);
    // A pipe is a separator unless it is escaped as `\|` or sits inside `code`.
    let mut out = vec![String::new()];
    let mut code = false;
    let mut chars = inner.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\\' && chars.peek() == Some(&'|') {
            chars.next();
            push_to_last(&mut out, '|');
        } else if c == '|' && !code {
            out.push(String::new());
        } else {
            if c == '`' {
                code = !code;
            }
            push_to_last(&mut out, c);
        }
    }
    out.into_iter().map(|cell| cell.trim().to_owned()).collect()
}

/// Adds a character to the cell being read, the last one in `out`.
fn push_to_last(out: &mut [String], c: char) {
    if let Some(cell) = out.last_mut() {
        cell.push(c);
    }
}

/// Whether a row is the header rule: every cell is dashes, optionally with colons for alignment.
fn is_rule_row(cells: &[String]) -> bool {
    !cells.is_empty()
        && cells.iter().all(|cell| {
            let dashes = cell.trim_matches(':');
            !dashes.is_empty() && dashes.chars().all(|c| c == '-')
        })
}

/// The text of a cell as it is shown, without the `**` and `` ` `` markers and with links as text.
fn cell_plain(cell: &str) -> String {
    inline_spans(cell)
        .iter()
        .map(|span| span.content.as_ref())
        .collect()
}

fn cell_width(cell: &str) -> usize {
    text_width(&cell_plain(cell))
}

/// `text` cut to at most `width` columns, ending in `…` when anything was left out.
fn cut(text: &str, width: usize) -> String {
    if text_width(text) <= width {
        return text.to_owned();
    }
    let mut out = String::new();
    let mut used = 0;
    for c in text.chars() {
        let w = char_width(c);
        // One column is kept for the ellipsis.
        if used + w + 1 > width {
            break;
        }
        out.push(c);
        used += w;
    }
    out.push('…');
    out
}

/// Splits a line into rows at most `width` columns wide. A row breaks after a space that fits in
/// it, and inside a word only when the word alone is wider than a row.
fn wrap(line: Line<'static>, width: usize) -> Vec<Line<'static>> {
    let width = width.max(1);
    let cells: Vec<(char, Style)> = line
        .spans
        .iter()
        .flat_map(|span| {
            let style = line.style.patch(span.style);
            span.content.chars().map(move |c| (c, style))
        })
        .collect();
    if cells.iter().map(|(c, _)| char_width(*c)).sum::<usize>() <= width {
        return vec![line];
    }
    let mut rows: Vec<&[(char, Style)]> = Vec::new();
    let mut start = 0;
    while start < cells.len() {
        let mut used = 0;
        let mut end = start;
        let mut space = None;
        while end < cells.len() && used + char_width(cells[end].0) <= width {
            used += char_width(cells[end].0);
            if cells[end].0 == ' ' {
                space = Some(end);
            }
            end += 1;
        }
        if end == cells.len() {
            rows.push(&cells[start..end]);
            break;
        }
        let (row_end, next) = match space {
            Some(at) if at > start => (at + 1, at + 1),
            _ if end == start => (start + 1, start + 1),
            _ => (end, end),
        };
        rows.push(&cells[start..row_end]);
        start = next;
    }
    rows.into_iter().map(to_line).collect()
}

/// Joins cells that share a style back into spans.
fn to_line(cells: &[(char, Style)]) -> Line<'static> {
    let mut spans: Vec<Span<'static>> = Vec::new();
    for (c, style) in cells {
        if let Some(last) = spans.last_mut().filter(|last| last.style == *style) {
            last.content.to_mut().push(*c);
        } else {
            spans.push(Span::styled(c.to_string(), *style));
        }
    }
    Line::from(spans)
}

/// Columns one character takes on screen. ASCII is one column, so the common case stays fast.
fn char_width(c: char) -> usize {
    if c.is_ascii() {
        1
    } else {
        Span::raw(c.to_string()).width()
    }
}

fn text_width(text: &str) -> usize {
    text.chars().map(char_width).sum()
}

#[cfg(test)]
#[path = "chat_tui_render_test.rs"]
mod chat_tui_render_test;
