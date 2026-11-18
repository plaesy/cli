//! Unit tests for `tests` split out of `fix.rs`.
//! Kept as a separate file so they are easy to find; included via `#[path]`.

use super::*;
use crate::mdlint::{compile, Config};

fn fixed(src: &str) -> String {
    let set = compile(&Config::default()).unwrap();
    fix_source(&set, src)
}

#[test]
fn trailing_spaces_and_missing_final_newline_are_fixed() {
    assert_eq!(fixed("text   \nmore"), "text\nmore\n");
}

#[test]
fn trailing_spaces_inside_code_blocks_are_preserved_by_default() {
    let src = "```text\ncode   \n```\n";
    assert_eq!(fixed(src), src);
}

#[test]
fn two_space_hard_breaks_are_preserved_by_default() {
    assert_eq!(fixed("line  \n"), "line  \n");
}

#[test]
fn a_heading_gets_blank_lines_around_it() {
    assert_eq!(fixed("para\n# Title\npara\n"), "para\n\n# Title\n\npara\n");
}

#[test]
fn a_fix_pass_that_changes_nothing_is_idempotent() {
    let once = fixed("# Title\n\ntext\n");
    assert_eq!(fixed(&once), once);
}
