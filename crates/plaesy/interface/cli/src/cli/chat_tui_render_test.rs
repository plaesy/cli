use super::*;
use ratatui::style::Color;

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
