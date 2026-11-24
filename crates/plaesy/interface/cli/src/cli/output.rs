//! Output style for the CLI: whether to color and whether to draw decorative glyphs.
//!
//! `--plain` turns off both. `NO_COLOR` (<https://no-color.org>) turns off color only when it is
//! set to a non-empty value. Every function here is pure: the caller reads the environment and
//! passes the value in.

use std::ffi::OsStr;

const RESET: &str = "\x1b[0m";

/// Whether ANSI color is allowed. A `NO_COLOR` that is present and non-empty disables it.
#[must_use]
pub fn color_enabled(no_color: Option<&OsStr>, plain: bool) -> bool {
    !plain && no_color.is_none_or(|value| value.is_empty())
}

/// The output choices for one run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Style {
    /// Emit ANSI color sequences.
    pub color: bool,
    /// Emit decorative glyphs such as check marks and arrows.
    pub glyphs: bool,
}

impl Style {
    /// Resolves the style from the `--plain` flag and the `NO_COLOR` value.
    #[must_use]
    pub fn resolve(plain: bool, no_color: Option<&OsStr>) -> Self {
        Self {
            color: color_enabled(no_color, plain),
            glyphs: !plain,
        }
    }

    /// Returns `fancy` when glyphs are allowed, otherwise the plain-text `ascii` fallback.
    #[must_use]
    pub fn glyph(self, fancy: &'static str, ascii: &'static str) -> &'static str {
        if self.glyphs { fancy } else { ascii }
    }

    /// Wraps `text` in the ANSI `code` when color is on, otherwise returns `text` unchanged.
    #[must_use]
    pub fn paint(self, code: &str, text: &str) -> String {
        if self.color {
            format!("{code}{text}{RESET}")
        } else {
            text.to_string()
        }
    }
}

/// Removes ANSI CSI sequences (`ESC [ ... <letter>`) from `text`.
#[must_use]
pub fn strip_ansi(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        if c == '\x1b' && chars.clone().next() == Some('[') {
            chars.next();
            for d in chars.by_ref() {
                if d.is_ascii_alphabetic() {
                    break;
                }
            }
        } else {
            out.push(c);
        }
    }
    out
}

#[cfg(test)]
#[path = "output_test.rs"]
mod output_test;
