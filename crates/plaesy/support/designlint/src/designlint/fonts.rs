//! `fonts` helpers split out of `designlint.rs`.

use super::*;

/// Default fonts the redesign rules name as defaults to replace (T-01).
const BANNED_DEFAULT_FONTS: [&str; 7] = [
    "inter",
    "arial",
    "roboto",
    "helvetica",
    "system-ui",
    "-apple-system",
    "segoe ui",
];

fn font_family_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)font-family\s*:\s*([^;}\n]+)").expect("static regex"))
}

fn font_family_js_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r#"fontFamily\s*:\s*['"]([^'"]+)['"]"#).expect("static regex"))
}

pub(super) fn lint_fonts(report: &mut Report, file: &str, text: &str) {
    for re in [font_family_re(), font_family_js_re()] {
        for caps in re.captures_iter(text) {
            let whole = caps.get(0).expect("group 0");
            let first = caps[1]
                .split(',')
                .next()
                .unwrap_or("")
                .trim()
                .trim_matches(['"', '\'', ' ']);
            if BANNED_DEFAULT_FONTS.contains(&first.to_lowercase().as_str()) {
                report.findings.push(Finding {
                    file: file.to_owned(),
                    line: line_of(text, whole.start()),
                    rule: rule::FONT,
                    severity: Severity::Warn,
                    message: format!("default font {first:?} is first in the stack: choose a face with character (for example Geist or Outfit) or justify it in the Keystone reason"),
                });
            }
        }
    }
}

/// Font size in px, weight, and an explanation of any assumption. `None` when
/// the size or weight cannot be read, so the rule is skipped rather than guessed.
pub(super) fn font_metrics(
    props: &std::collections::BTreeMap<String, String>,
) -> Option<(f64, u32, String)> {
    let mut size = ROOT_PX;
    let mut weight = 400u32;
    let mut basis = String::new();
    let shorthand = props.get("font").map(String::as_str).unwrap_or("");

    if let Some(value) = props.get("font-size") {
        let (px, relative) = parse_size(value)?;
        size = px;
        if relative {
            basis = format!(
                " (size taken as {px:.0}px from {value:?}, assuming a {ROOT_PX:.0}px root)"
            );
        }
    } else if !shorthand.is_empty() {
        let lower = shorthand.to_lowercase();
        let m = size_in_shorthand_re().captures(&lower)?;
        size = parse_size(&m[1])?.0;
    }

    if let Some(value) = props.get("font-weight") {
        weight = parse_weight(value)?;
    } else if !shorthand.is_empty() {
        let lower = shorthand.to_lowercase();
        if lower.contains("bolder") || lower.contains("lighter") {
            return None;
        }
        if let Some(m) = weight_in_shorthand_re().captures(&lower) {
            weight = parse_weight(&m[1]).unwrap_or(400);
        }
    }
    Some((size, weight, basis))
}

fn size_in_shorthand_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"(?:^|\s)([0-9]*\.?[0-9]+(?:px|rem|em))(?:/|\s|$)").expect("static regex")
    })
}
