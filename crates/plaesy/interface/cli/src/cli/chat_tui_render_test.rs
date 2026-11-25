use super::*;
use ratatui::style::{Color, Modifier, Style};

fn lines(text: &[&str]) -> Vec<String> {
    text.iter().map(|line| line.to_string()).collect()
}

fn text(line: &Line<'_>) -> String {
    line.spans
        .iter()
        .map(|span| span.content.as_ref())
        .collect()
}

#[test]
fn table_columns_are_aligned_under_a_header_rule() {
    let rows = render(
        &lines(&[
            "| Nama | Nilai |",
            "|---|---|",
            "| a | 10 |",
            "| panjang | 2 |",
        ]),
        40,
    );
    let texts: Vec<String> = rows.iter().map(text).collect();
    assert_eq!(
        texts,
        vec![
            "│ Nama    │ Nilai │",
            "├─────────┼───────┤",
            "│ a       │ 10    │",
            "│ panjang │ 2     │",
        ]
    );
}

#[test]
fn a_table_too_wide_for_the_log_is_shown_as_plain_lines() {
    let rows = render(&lines(&["| Nama | Nilai |", "|---|---|"]), 12);
    // Plain text, wrapped to 12 columns: the characters are unchanged, only the row breaks move.
    let texts: Vec<String> = rows.iter().map(text).collect();
    assert_eq!(texts.concat(), "| Nama | Nilai ||---|---|");
    assert!(rows.iter().all(|row| row.width() <= 12));
}

#[test]
fn table_cells_are_cut_with_an_ellipsis_to_fit_the_width() {
    let rows = render(
        &lines(&[
            "| Nama | Nilai |",
            "|---|---|",
            "| a | 10 |",
            "| panjang | 2 |",
        ]),
        14,
    );
    assert!(rows.iter().all(|row| row.width() <= 14));
    assert!(text(&rows[3]).contains("pan…"));
}

#[test]
fn table_columns_line_up_when_a_cell_holds_a_wide_emoji() {
    let rows = render(&lines(&["| status | ok |", "|---|---|", "| ✅ | x |"]), 40);
    assert_eq!(rows[0].width(), rows[2].width());
    assert!(text(&rows[2]).starts_with("│ ✅ "));
}

#[test]
fn diff_blocks_colour_additions_removals_and_hunks() {
    let rows = render(
        &lines(&["```diff", "+baru", "-lama", "@@ -1 +1 @@", "konteks", "```"]),
        40,
    );
    assert_eq!(rows[1].style.fg, Some(Color::Green));
    assert_eq!(rows[2].style.fg, Some(Color::Red));
    assert_eq!(rows[3].style.fg, Some(Color::Cyan));
    assert_eq!(rows[4].style.fg, None);
}

#[test]
fn a_table_inside_a_code_block_stays_as_typed() {
    let rows = render(&lines(&["```", "| a | b |", "```"]), 40);
    assert_eq!(text(&rows[1]), "| a | b |");
}

#[test]
fn rules_quotes_and_links_are_drawn() {
    let rows = render(&lines(&["---", "> kutip", "[docs](https://x.y)"]), 20);
    assert_eq!(text(&rows[0]), "─".repeat(20));
    assert_eq!(text(&rows[1]), "▎ kutip");
    assert_eq!(text(&rows[2]), "docs (https://x.y)");
}

#[test]
fn a_long_line_wraps_after_a_space_that_fits() {
    let rows = render(&lines(&["satu dua tiga empat"]), 10);
    let texts: Vec<String> = rows.iter().map(text).collect();
    assert_eq!(texts, vec!["satu dua ", "tiga empat"]);
}

#[test]
fn a_word_longer_than_the_width_is_split_inside() {
    let rows = render(&lines(&["abcdefghijkl"]), 5);
    let texts: Vec<String> = rows.iter().map(text).collect();
    assert_eq!(texts, vec!["abcde", "fghij", "kl"]);
}

#[test]
fn numbered_items_keep_their_number_bold_and_their_indent() {
    let rows = render(&lines(&["  3. tiga *miring*"]), 40);
    assert_eq!(text(&rows[0]), "  3. tiga miring");
    assert_eq!(rows[0].spans[1].style, Style::new().bold());
    assert!(rows[0]
        .spans
        .last()
        .is_some_and(|s| s.style.add_modifier == Modifier::ITALIC));
}

#[test]
fn italic_and_strikethrough_are_styled_but_snake_case_is_not() {
    let rows = render(&lines(&["~~lama~~ dan snake_case_nama dan **tebal**"]), 60);
    assert_eq!(text(&rows[0]), "lama dan snake_case_nama dan tebal");
    assert_eq!(rows[0].spans[0].style.add_modifier, Modifier::CROSSED_OUT);
}

#[test]
fn a_code_block_shows_its_language_as_a_label() {
    let rows = render(&lines(&["```rust", "let a = 1;", "```"]), 40);
    assert_eq!(text(&rows[0]), "─── rust");
    assert_eq!(text(&rows[1]), "let a = 1;");
}

#[test]
fn an_escaped_pipe_and_a_pipe_in_code_stay_in_their_cell() {
    let rows = render(
        &lines(&["| a \\| b | c |", "|---|---|", "| `x|y` | z |"]),
        40,
    );
    assert!(text(&rows[0]).contains("a | b"));
    assert!(text(&rows[2]).contains("x|y"));
    assert_eq!(rows.len(), 3);
}

#[test]
fn rust_code_gets_keywords_strings_and_comments_coloured() {
    let rows = render(
        &lines(&["```rust", "fn main() { let s = \"hi\"; // nota", "```"]),
        60,
    );
    assert_eq!(text(&rows[1]), "fn main() { let s = \"hi\"; // nota");
    let keyword = rows[1]
        .spans
        .iter()
        .find(|s| s.content == "fn")
        .expect("keyword span");
    assert_eq!(keyword.style, Style::new().fg(Color::Magenta).bold());
    let string = rows[1]
        .spans
        .iter()
        .find(|s| s.content == "\"hi\"")
        .expect("string span");
    assert_eq!(string.style.fg, Some(Color::Yellow));
    let comment = rows[1].spans.last().expect("comment span");
    assert_eq!(comment.content, "// nota");
    assert_eq!(comment.style.fg, Some(Color::DarkGray));
}

#[test]
fn python_comments_use_a_hash_and_unknown_languages_stay_green() {
    let rows = render(
        &lines(&[
            "```python",
            "def f(): # tanda",
            "```",
            "```text",
            "def f()",
            "```",
        ]),
        60,
    );
    assert_eq!(rows[1].spans.last().expect("comment").content, "# tanda");
    assert_eq!(rows[4].style.fg, Some(Color::Green));
}
