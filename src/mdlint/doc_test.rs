//! Unit tests for `tests` split out of `doc.rs`.
//! Kept as a separate file so they are easy to find; included via `#[path]`.

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
