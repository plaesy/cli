//! Unit tests for `tests` split out of `rss.rs`.
//! Kept as a separate file so they are easy to find; included via `#[path]`.

use super::*;

const RSS: &str = r#"<?xml version="1.0"?>
<rss version="2.0"><channel>
  <title>Example</title><description>An example feed</description>
  <item><title>First</title><link>https://example.com/1</link><pubDate>Mon</pubDate></item>
</channel></rss>"#;

const ATOM: &str = r#"<feed xmlns="http://www.w3.org/2005/Atom">
  <title>Atom Example</title>
  <entry><title>Entry</title><link href="https://example.com/a" rel="alternate"/><updated>2026-01-01</updated></entry>
</feed>"#;

#[test]
fn rss_items_are_read() {
    let feed = parse_rss2(RSS.as_bytes()).expect("rss");
    assert_eq!(feed.title, "Example");
    assert_eq!(feed.items.len(), 1);
    assert_eq!(feed.items[0].link, "https://example.com/1");
    assert_eq!(feed.items[0].published, "Mon");
}

#[test]
fn atom_entries_take_their_link_from_href() {
    let feed = parse_atom(ATOM.as_bytes()).expect("atom");
    assert_eq!(feed.items[0].link, "https://example.com/a");
    assert_eq!(feed.items[0].published, "2026-01-01");
}

#[test]
fn an_rss_parser_rejects_atom() {
    assert!(parse_rss2(ATOM.as_bytes()).is_err());
}

#[test]
fn long_summaries_are_cut_on_a_character_boundary() {
    let text = "é".repeat(250);
    let cut = truncate_chars(&text, MAX_SUMMARY_CHARS);
    assert!(cut.ends_with("..."));
    assert_eq!(cut.chars().count(), MAX_SUMMARY_CHARS + 3);
}
