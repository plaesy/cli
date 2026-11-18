//! Unit tests for `tests` split out of `text_rules.rs`.
//! Kept as a separate file so they are easy to find; included via `#[path]`.

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
