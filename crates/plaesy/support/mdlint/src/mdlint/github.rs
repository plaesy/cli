//! The three GitHub accessibility rules. They live in an npm-only package in the
//! original toolchain, so they are implemented here.

use super::doc::Doc;
use super::{RuleSet, Violation};

pub const GHA_EMPTY_ALT: &str = "GHA001";
pub const GHA_DEFAULT_ALT: &str = "GHA002";
pub const GHA_GENERIC_LINK: &str = "GHA003";

/// Link texts that do not describe their destination. Compared case-insensitively,
/// ignoring surrounding punctuation.
const GENERIC_LINK_TEXTS: [&str; 9] = [
    "click here",
    "here",
    "link",
    "more",
    "read more",
    "this",
    "learn more",
    "this link",
    "see more",
];

fn v(rule: &'static str, line: usize, message: impl Into<String>) -> Violation {
    Violation {
        rule,
        line,
        column: 1,
        message: message.into(),
    }
}

pub fn gha001(d: &Doc, _rs: &RuleSet) -> Vec<Violation> {
    let (images, _) = d.ast_links();
    images
        .iter()
        .filter(|img| img.text.trim().is_empty())
        .map(|img| v(GHA_EMPTY_ALT, img.line, "image has no alt text"))
        .collect()
}

pub fn gha002(d: &Doc, _rs: &RuleSet) -> Vec<Violation> {
    let (images, _) = d.ast_links();
    let mut out = Vec::new();
    for img in &images {
        let alt = img.text.trim();
        if alt.is_empty() {
            continue; // already reported by GHA001
        }
        let base = img.destination.rsplit('/').next().unwrap_or("");
        let stem = match base.rfind('.') {
            Some(dot) if dot > 0 => &base[..dot],
            _ => base,
        };
        if alt == base || alt == stem {
            out.push(v(
                GHA_DEFAULT_ALT,
                img.line,
                format!("alt text repeats the image file name ({base})"),
            ));
        }
    }
    out
}

pub fn gha003(d: &Doc, _rs: &RuleSet) -> Vec<Violation> {
    let (_, links) = d.ast_links();
    links
        .iter()
        .filter_map(|link| {
            let text = link
                .text
                .trim()
                .to_lowercase()
                .trim_matches(|c: char| ".,:;!?'\"()[]".contains(c))
                .to_owned();
            GENERIC_LINK_TEXTS.contains(&text.as_str()).then(|| {
                v(
                    GHA_GENERIC_LINK,
                    link.line,
                    format!("link text {text:?} does not describe the destination"),
                )
            })
        })
        .collect()
}
