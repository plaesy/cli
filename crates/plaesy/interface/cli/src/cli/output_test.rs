//! Unit tests for `output.rs`. Pure functions only: no environment access.

use std::ffi::OsStr;

use super::*;

#[test]
fn no_color_disables_color_only_when_non_empty() {
    assert!(color_enabled(None, false));
    assert!(color_enabled(Some(OsStr::new("")), false));
    assert!(!color_enabled(Some(OsStr::new("1")), false));
}

#[test]
fn plain_disables_color() {
    assert!(!color_enabled(None, true));
}

#[test]
fn plain_turns_off_color_and_glyphs() {
    let style = Style::resolve(true, None);
    assert_eq!(
        style,
        Style {
            color: false,
            glyphs: false
        }
    );
}

#[test]
fn no_color_keeps_glyphs() {
    let style = Style::resolve(false, Some(OsStr::new("1")));
    assert!(!style.color);
    assert!(style.glyphs);
}

#[test]
fn glyph_falls_back_to_ascii_in_plain_mode() {
    let plain = Style::resolve(true, None);
    let fancy = Style::resolve(false, None);
    assert_eq!(plain.glyph("✓", "OK"), "OK");
    assert_eq!(fancy.glyph("✓", "OK"), "✓");
}

#[test]
fn paint_adds_escape_only_when_color_is_on() {
    let colored = Style::resolve(false, None);
    let plain = Style::resolve(true, None);
    assert_eq!(colored.paint("\x1b[0;32m", "ok"), "\x1b[0;32mok\x1b[0m");
    assert_eq!(plain.paint("\x1b[0;32m", "ok"), "ok");
}

#[test]
fn strip_ansi_removes_sequences_and_keeps_text() {
    let text = "\x1b[0;31m[ERROR]\x1b[0m done";
    assert_eq!(strip_ansi(text), "[ERROR] done");
    assert_eq!(strip_ansi("no escapes"), "no escapes");
}

#[test]
fn plain_style_output_contains_no_escape_bytes() {
    let style = Style::resolve(true, None);
    let line = style.paint("\x1b[1;33m", "WARNING");
    assert!(!line.contains('\x1b'));
    assert!(!style.glyph("⚠", "!").chars().any(|c| !c.is_ascii()));
}
