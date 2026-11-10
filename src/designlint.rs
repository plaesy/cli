//! Deterministic, offline checks on produced frontend output (HTML, CSS, JSX, and
//! similar) against the frontend-design and redesign rules.
//!
//! Nothing is rendered. Checks that need a browser are out of scope. Their
//! evidence is the screenshots at each required width, which are checked only for
//! presence. Colour checks read CSS rules, inline styles, and literal JSX style
//! objects. Each text colour that cannot be resolved is counted in
//! [`Report::contrast_skipped`].
//! Port of `scripts/internal/designlint` (Go).

use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use regex::Regex;

/// Rule identifiers. They are stable: fixtures and evals refer to them by name.
pub mod rule {
    pub const IMAGE_HOST: &str = "DL-IMG-HOST";
    pub const FIGURE: &str = "DL-INVENTED-FIGURE";
    pub const FONT: &str = "DL-DEFAULT-FONT";
    pub const CONTRAST: &str = "DL-CONTRAST";
    pub const VIEWPORTS: &str = "DL-VIEWPORTS";
    pub const TOKEN: &str = "DL-TOKEN";
    pub const GRADIENT_TEXT: &str = "DL-GRADIENT-TEXT";
    pub const SIDE_STRIPE: &str = "DL-SIDE-STRIPE";
    pub const GLOW_SHADOW: &str = "DL-GLOW-SHADOW";
    pub const HARD_SHADOW: &str = "DL-HARD-SHADOW";
    pub const EMOJI: &str = "DL-EMOJI";
    pub const INDEX_NUMBER: &str = "DL-INDEX-NUMBER";
}

/// Viewport widths the render procedure must cover.
pub const REQUIRED_WIDTHS: [u32; 3] = [1440, 768, 390];

/// Browser default font size. `em` and `%` resolve against it.
const ROOT_PX: f64 = 16.0;

/// How serious a finding is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Fail,
    Warn,
    Info,
}

impl fmt::Display for Severity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Fail => "FAIL",
            Self::Warn => "WARN",
            Self::Info => "INFO",
        })
    }
}

/// One violation, or a note about something that could not be checked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finding {
    pub file: String,
    pub line: usize,
    pub rule: &'static str,
    pub severity: Severity,
    pub message: String,
}

/// Options for a run.
#[derive(Debug, Clone, Default)]
pub struct Options {
    /// Directory of screenshots at each required width. `None` reports coverage as INFO.
    pub evidence_dir: Option<PathBuf>,
}

/// Outcome of a run.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Report {
    pub files: usize,
    pub contrast_checked: usize,
    pub contrast_skipped: usize,
    pub findings: Vec<Finding>,
}

impl Report {
    /// Reports whether any finding is a FAIL.
    #[must_use]
    pub fn failed(&self) -> bool {
        self.findings.iter().any(|f| f.severity == Severity::Fail)
    }

    /// Counts findings with the given rule and severity.
    #[must_use]
    pub fn count(&self, rule: &str, severity: Severity) -> usize {
        self.findings
            .iter()
            .filter(|f| f.rule == rule && f.severity == severity)
            .count()
    }
}

/// An sRGB colour, channels 0..255.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Rgb {
    r: f64,
    g: f64,
    b: f64,
}

/// A colour with alpha 0..1.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Colour {
    c: Rgb,
    a: f64,
}

impl Colour {
    /// Key used to detect conflicting declarations of one token.
    fn key(self) -> String {
        format!(
            "{:02x}{:02x}{:02x}/{:.3}",
            self.c.r.round() as u8,
            self.c.g.round() as u8,
            self.c.b.round() as u8,
            self.a
        )
    }

    /// `#rrggbb`, with the opacity appended when it is not 1.
    fn hex(self) -> String {
        let mut s = format!(
            "#{:02x}{:02x}{:02x}",
            self.c.r.round() as u8,
            self.c.g.round() as u8,
            self.c.b.round() as u8
        );
        if self.a < 1.0 {
            s.push_str(&format!(" at {:.0}% opacity", self.a * 100.0));
        }
        s
    }
}

fn hex_colour_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"#([0-9a-fA-F]{8}|[0-9a-fA-F]{6}|[0-9a-fA-F]{4}|[0-9a-fA-F]{3})(?-u:\b)")
            .expect("static regex")
    })
}

fn rgb_colour_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"(?i)rgba?\(\s*([0-9.]+)\s*[, ]\s*([0-9.]+)\s*[, ]\s*([0-9.]+)\s*(?:[,/]\s*([0-9.]+)(%?)\s*)?\)")
            .expect("static regex")
    })
}

/// Parses the first colour in `value`: `#rgb`, `#rgba`, `#rrggbb`, `#rrggbbaa`,
/// `rgb()`/`rgba()` with 0..255 channels, or one of the named keywords.
fn parse_colour(value: &str) -> Option<Colour> {
    let value = value.trim();
    if let Some(m) = hex_colour_re().captures(value) {
        let mut hex = m[1].to_owned();
        if hex.len() == 3 || hex.len() == 4 {
            hex = hex.chars().flat_map(|c| [c, c]).collect();
        }
        let n = u64::from_str_radix(&hex, 16).ok()?;
        if hex.len() == 8 {
            return Some(Colour {
                c: Rgb {
                    r: ((n >> 24) & 0xff) as f64,
                    g: ((n >> 16) & 0xff) as f64,
                    b: ((n >> 8) & 0xff) as f64,
                },
                a: (n & 0xff) as f64 / 255.0,
            });
        }
        return Some(Colour {
            c: Rgb {
                r: ((n >> 16) & 0xff) as f64,
                g: ((n >> 8) & 0xff) as f64,
                b: (n & 0xff) as f64,
            },
            a: 1.0,
        });
    }
    if let Some(m) = rgb_colour_re().captures(value) {
        let mut channels = [0.0f64; 3];
        for (slot, cap) in channels.iter_mut().zip(1..=3) {
            let v: f64 = m[cap].parse().ok()?;
            if !(0.0..=255.0).contains(&v) {
                return None;
            }
            *slot = v;
        }
        let mut alpha = 1.0;
        if let Some(a) = m.get(4) {
            let mut a: f64 = a.as_str().parse().ok()?;
            if m.get(5).is_some_and(|p| p.as_str() == "%") {
                a /= 100.0;
            }
            alpha = a.clamp(0.0, 1.0);
        }
        return Some(Colour {
            c: Rgb {
                r: channels[0],
                g: channels[1],
                b: channels[2],
            },
            a: alpha,
        });
    }
    match value.to_lowercase().as_str() {
        "black" => Some(Colour {
            c: Rgb {
                r: 0.0,
                g: 0.0,
                b: 0.0,
            },
            a: 1.0,
        }),
        "white" => Some(Colour {
            c: Rgb {
                r: 255.0,
                g: 255.0,
                b: 255.0,
            },
            a: 1.0,
        }),
        "transparent" => Some(Colour {
            c: Rgb {
                r: 0.0,
                g: 0.0,
                b: 0.0,
            },
            a: 0.0,
        }),
        _ => None,
    }
}

/// Draws `fg` over an opaque `bg`.
fn composite(fg: Colour, bg: Colour) -> Rgb {
    Rgb {
        r: fg.c.r * fg.a + bg.c.r * (1.0 - fg.a),
        g: fg.c.g * fg.a + bg.c.g * (1.0 - fg.a),
        b: fg.c.b * fg.a + bg.c.b * (1.0 - fg.a),
    }
}

/// WCAG 2.x relative luminance.
fn luminance(c: Rgb) -> f64 {
    let lin = |v: f64| {
        let s = v / 255.0;
        if s <= 0.03928 {
            s / 12.92
        } else {
            ((s + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * lin(c.r) + 0.7152 * lin(c.g) + 0.0722 * lin(c.b)
}

/// WCAG contrast ratio between two opaque colours, from 1 to 21.
fn contrast_ratio(a: Rgb, b: Rgb) -> f64 {
    let (mut la, mut lb) = (luminance(a), luminance(b));
    if la < lb {
        std::mem::swap(&mut la, &mut lb);
    }
    (la + 0.05) / (lb + 0.05)
}

fn line_of(text: &str, index: usize) -> usize {
    text[..index].matches('\n').count() + 1
}

/// Replaces `re` matches with spaces, keeping newlines so offsets and lines stay true.
fn blank_matches(text: &str, re: &Regex) -> String {
    re.replace_all(text, |caps: &regex::Captures<'_>| {
        caps[0]
            .chars()
            .map(|c| if c == '\n' { '\n' } else { ' ' })
            .collect::<String>()
    })
    .into_owned()
}

fn css_comment_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?s)/\*.*?\*/").expect("static regex"))
}

fn html_noise_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"(?is)<(script|style)[^>]*>.*?</(script|style)>").expect("static regex")
    })
}

fn tag_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"<[^>]*>").expect("static regex"))
}

/// Visible HTML text: scripts, styles, and tags blanked out.
fn visible_html(text: &str) -> String {
    let without_noise = html_noise_re().replace_all(text, |caps: &regex::Captures<'_>| {
        "\n".repeat(caps[0].matches('\n').count())
    });
    blank_matches(&without_noise, tag_re())
}

/// Collects every lintable file under `paths`, sorted and unique.
///
/// # Errors
///
/// I/O failures reading a path or a directory.
fn collect_files(paths: &[PathBuf]) -> io::Result<Vec<PathBuf>> {
    let mut out = Vec::new();
    for path in paths {
        if !path.is_dir() {
            out.push(path.clone());
            continue;
        }
        walk(path, path, &mut out)?;
    }
    out.sort();
    out.dedup();
    Ok(out)
}

fn walk(root: &Path, dir: &Path, out: &mut Vec<PathBuf>) -> io::Result<()> {
    let mut entries: Vec<PathBuf> = fs::read_dir(dir)?.flatten().map(|e| e.path()).collect();
    entries.sort();
    for path in entries {
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        if path.is_dir() {
            if matches!(name.as_str(), ".git" | "node_modules")
                || (dir == root && matches!(name.as_str(), "dist" | "build" | ".next"))
            {
                continue;
            }
            walk(root, &path, out)?;
        } else if is_linted(&path) {
            out.push(path);
        }
    }
    Ok(())
}

fn extension(path: &Path) -> String {
    path.extension()
        .map(|e| format!(".{}", e.to_string_lossy().to_lowercase()))
        .unwrap_or_default()
}

fn is_linted(path: &Path) -> bool {
    matches!(
        extension(path).as_str(),
        ".html" | ".htm" | ".css" | ".scss" | ".jsx" | ".tsx" | ".vue" | ".svelte"
    )
}

fn is_html(path: &Path) -> bool {
    matches!(extension(path).as_str(), ".html" | ".htm")
}

/// Placeholder picture hosts, forbidden by the redesign rules (C-11).
const BANNED_IMAGE_HOSTS: [&str; 8] = [
    "picsum.photos",
    "placehold.co",
    "placeholder.com",
    "dummyimage.com",
    "placekitten.com",
    "placeimg.com",
    "loremflickr.com",
    "source.unsplash.com",
];

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

fn lint_image_hosts(report: &mut Report, file: &str, text: &str) {
    let lower = text.to_lowercase();
    for host in BANNED_IMAGE_HOSTS {
        for (at, _) in lower.match_indices(host) {
            report.findings.push(Finding {
                file: file.to_owned(),
                line: line_of(text, at),
                rule: rule::IMAGE_HOST,
                severity: Severity::Fail,
                message: format!("picture from placeholder host {host:?}: use licence-checked imagery with a recorded credit, or a flat colour field"),
            });
        }
    }
}

fn figure_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"99\.99%|\$100\.00|(?-u:\b)50%").expect("static regex"))
}

/// Round, placeholder-looking figures in visible HTML text. A WARN, because a brief may state 50%.
fn lint_figures(report: &mut Report, file: &str, text: &str) {
    let visible = visible_html(text);
    for m in figure_re().find_iter(&visible) {
        report.findings.push(Finding {
            file: file.to_owned(),
            line: line_of(&visible, m.start()),
            rule: rule::FIGURE,
            severity: Severity::Warn,
            message: format!("figure {:?} looks placeholder-like: confirm it comes from the brief, or mark it as a sample for the client to confirm", m.as_str()),
        });
    }
}

fn font_family_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)font-family\s*:\s*([^;}\n]+)").expect("static regex"))
}

fn font_family_js_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r#"fontFamily\s*:\s*['"]([^'"]+)['"]"#).expect("static regex"))
}

fn lint_fonts(report: &mut Report, file: &str, text: &str) {
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

/// Colour tokens declared across the run. A token declared with more than one
/// value is a conflict, and it is not resolved.
#[derive(Debug, Default)]
struct Tokens {
    colours: std::collections::BTreeMap<String, Colour>,
    conflicts: std::collections::BTreeMap<String, usize>,
}

fn token_decl_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"--([A-Za-z0-9_-]+)\s*:\s*([^;}{]+)").expect("static regex"))
}

fn collect_tokens(contents: &[String]) -> Tokens {
    let mut seen: std::collections::BTreeMap<String, std::collections::BTreeSet<String>> =
        Default::default();
    let mut tokens = Tokens::default();
    for text in contents {
        for m in token_decl_re().captures_iter(text) {
            let Some(colour) = parse_colour(&m[2]) else {
                continue;
            };
            seen.entry(m[1].to_owned())
                .or_default()
                .insert(colour.key());
            tokens.colours.entry(m[1].to_owned()).or_insert(colour);
        }
    }
    for (name, values) in seen {
        if values.len() > 1 {
            tokens.conflicts.insert(name.clone(), values.len());
            tokens.colours.remove(&name);
        }
    }
    tokens
}

impl Tokens {
    /// The colour a value stands for: a literal, or `var(--name)`. Gradients and images do not resolve.
    fn resolve(&self, value: &str) -> Option<Colour> {
        let lower = value.to_lowercase();
        if lower.contains("gradient") || lower.contains("url(") {
            return None;
        }
        if lower.starts_with("var(") {
            let inner = value.trim()[4..].trim().trim_end_matches(')').trim();
            let name = inner.trim_start_matches("--");
            let name = name.split(',').next().unwrap_or("").trim();
            return self.colours.get(name).copied();
        }
        parse_colour(value)
    }
}

/// Splits `s` at `sep` wherever `sep` is outside parentheses and quotes.
fn split_top(s: &str, sep: char) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut depth = 0i32;
    let mut quote: Option<char> = None;
    let mut start = 0;
    for (i, c) in s.char_indices() {
        match quote {
            Some(q) => {
                if c == q {
                    quote = None;
                }
            }
            None => match c {
                '"' | '\'' => quote = Some(c),
                '(' => depth += 1,
                ')' => depth -= 1,
                _ if c == sep && depth == 0 => {
                    parts.push(&s[start..i]);
                    start = i + c.len_utf8();
                }
                _ => {}
            },
        }
    }
    parts.push(&s[start..]);
    parts
}

/// Reads `prop: value; prop: value` into a map of lower-case property names.
fn css_decls(body: &str) -> std::collections::BTreeMap<String, String> {
    split_top(body, ';')
        .into_iter()
        .filter_map(|decl| {
            let (prop, value) = decl.split_once(':')?;
            let prop = prop.trim().to_lowercase();
            (!prop.is_empty()).then(|| (prop, value.trim().to_owned()))
        })
        .collect()
}

/// Reads a React style object body into CSS property names. A bare number for
/// `fontSize` means pixels.
fn jsx_decls(body: &str) -> std::collections::BTreeMap<String, String> {
    split_top(body, ',')
        .into_iter()
        .filter_map(|pair| {
            let (key, value) = pair.split_once(':')?;
            let key = camel_to_kebab(key.trim());
            let mut value = value.trim().trim_matches(['\'', '"']).to_owned();
            if key == "font-size" && value.parse::<f64>().is_ok() {
                value.push_str("px");
            }
            (!key.is_empty()).then_some((key, value))
        })
        .collect()
}

fn camel_to_kebab(s: &str) -> String {
    let mut out = String::new();
    for c in s.chars() {
        if c.is_ascii_uppercase() {
            out.push('-');
            out.push(c.to_ascii_lowercase());
        } else {
            out.push(c);
        }
    }
    out
}

fn block_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"([^{}]+)\{([^{}]*)\}").expect("static regex"))
}

fn inline_style_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r#"(?i)(?-u:\b)style\s*=\s*(?:"([^"]*)"|'([^']*)')"#).expect("static regex")
    })
}

fn jsx_style_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?-u:\b)style\s*=\s*\{\{([^{}]*)\}\}").expect("static regex"))
}

fn style_brace_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?-u:\b)style\s*=\s*\{").expect("static regex"))
}

fn vue_style_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r":style\s*=").expect("static regex"))
}

/// Checks every rule and inline style that can be read.
fn lint_contrast(report: &mut Report, file: &str, text: &str, tokens: &Tokens) {
    let clean = blank_matches(text, css_comment_re());
    for caps in block_re().captures_iter(&clean) {
        let whole = caps.get(0).expect("group 0");
        check_rule(
            report,
            file,
            line_of(&clean, whole.start()),
            &css_decls(&caps[2]),
            tokens,
        );
    }
    for caps in inline_style_re().captures_iter(text) {
        let whole = caps.get(0).expect("group 0");
        let body = caps
            .get(1)
            .or_else(|| caps.get(2))
            .map_or("", |m| m.as_str());
        check_rule(
            report,
            file,
            line_of(text, whole.start()),
            &css_decls(body),
            tokens,
        );
    }
    let mut literal_starts = std::collections::BTreeSet::new();
    for caps in jsx_style_re().captures_iter(text) {
        let whole = caps.get(0).expect("group 0");
        literal_starts.insert(whole.start());
        check_rule(
            report,
            file,
            line_of(text, whole.start()),
            &jsx_decls(&caps[1]),
            tokens,
        );
    }
    for m in style_brace_re().find_iter(text) {
        if !literal_starts.contains(&m.start()) {
            report.contrast_skipped += 1;
        }
    }
    report.contrast_skipped += vue_style_re().find_iter(text).count();
}

/// Applies WCAG 2.2 AA to one set of declarations, or counts it as skipped.
fn check_rule(
    report: &mut Report,
    file: &str,
    line: usize,
    props: &std::collections::BTreeMap<String, String>,
    tokens: &Tokens,
) {
    let Some(fg_value) = props.get("color") else {
        return;
    };
    let Some(bg_value) = props
        .get("background-color")
        .or_else(|| props.get("background"))
    else {
        report.contrast_skipped += 1; // the background comes from elsewhere in the cascade
        return;
    };
    let (Some(fg), Some(bg)) = (tokens.resolve(fg_value), tokens.resolve(bg_value)) else {
        report.contrast_skipped += 1;
        return;
    };
    if bg.a < 1.0 {
        report.contrast_skipped += 1;
        return;
    }
    let Some((size, weight, mut basis)) = font_metrics(props) else {
        report.contrast_skipped += 1;
        return;
    };

    let mut fg = fg;
    if fg.a < 1.0 {
        basis.push_str(&format!(
            " (text is {:.0}% opaque, measured as composited over {})",
            fg.a * 100.0,
            bg.hex()
        ));
        fg = Colour {
            c: composite(fg, bg),
            a: 1.0,
        };
    }
    report.contrast_checked += 1;
    let ratio = contrast_ratio(fg.c, bg.c);
    let (need, kind) = if size >= 24.0 || (size >= 18.66 && weight >= 700) {
        (3.0, "large text")
    } else {
        (4.5, "normal text")
    };
    if ratio < need {
        report.findings.push(Finding {
            file: file.to_owned(),
            line,
            rule: rule::CONTRAST,
            severity: Severity::Fail,
            message: format!(
                "{} on {} is {ratio:.2}:1; WCAG 2.2 AA needs {need:.1}:1 for {kind}{basis}",
                fg.hex(),
                bg.hex()
            ),
        });
    }
}

/// Font size in px, weight, and an explanation of any assumption. `None` when
/// the size or weight cannot be read, so the rule is skipped rather than guessed.
fn font_metrics(props: &std::collections::BTreeMap<String, String>) -> Option<(f64, u32, String)> {
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

fn weight_in_shorthand_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?:^|\s)([1-9]00|bold|normal)(?:\s|$)").expect("static regex"))
}

fn size_token_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^([0-9]*\.?[0-9]+)(px|rem|em|%)?$").expect("static regex"))
}

/// Reads a CSS length as px. The flag is true when it was relative to the root size.
/// `clamp()` resolves to its smallest bound, so the strictest threshold applies.
fn parse_size(value: &str) -> Option<(f64, bool)> {
    let value = value.trim().to_lowercase();
    if let Some(inner) = value
        .strip_prefix("clamp(")
        .and_then(|v| v.strip_suffix(')'))
    {
        let args = split_top(inner, ',');
        if args.len() != 3 {
            return None;
        }
        let (lo, lo_rel) = parse_size(args[0])?;
        let mut best = lo;
        let mut relative = lo_rel;
        if let Some((hi, hi_rel)) = parse_size(args[2]) {
            best = best.min(hi);
            relative |= hi_rel;
        }
        return Some((best, relative));
    }
    let m = size_token_re().captures(&value)?;
    let n: f64 = m[1].parse().ok()?;
    match m.get(2).map(|u| u.as_str()) {
        Some("px") => Some((n, false)),
        Some("rem") | Some("em") => Some((n * ROOT_PX, true)),
        Some("%") => Some((n * ROOT_PX / 100.0, true)),
        None if n == 0.0 => Some((0.0, false)),
        _ => None,
    }
}

fn parse_weight(value: &str) -> Option<u32> {
    match value.trim().to_lowercase().as_str() {
        "normal" => Some(400),
        "bold" => Some(700),
        other => {
            let n: u32 = other.parse().ok()?;
            (100..=900)
                .contains(&n)
                .then_some(n)
                .filter(|n| n % 100 == 0)
        }
    }
}

/// Whether a file name carries `width` as its own number. `1768.png` does not count as 768.
fn viewport_width(name: &str, width: u32) -> bool {
    let re = Regex::new(&format!("(?:^|[^0-9]){width}(?:[^0-9]|$)")).expect("width regex");
    re.is_match(name)
}

fn lint_viewports(report: &mut Report, evidence: Option<&Path>) {
    let Some(dir) = evidence else {
        report.findings.push(Finding {
            file: "(no evidence)".into(),
            line: 0,
            rule: rule::VIEWPORTS,
            severity: Severity::Info,
            message:
                "no render evidence supplied: viewport coverage (1440, 768, 390) was not checked"
                    .into(),
        });
        return;
    };
    if !dir.is_dir() {
        report.findings.push(Finding {
            file: dir.display().to_string(),
            line: 0,
            rule: rule::VIEWPORTS,
            severity: Severity::Fail,
            message:
                "evidence directory does not exist: renders at 1440, 768 and 390 px are required"
                    .into(),
        });
        return;
    }
    let mut names = Vec::new();
    collect_images(dir, &mut names);
    for width in REQUIRED_WIDTHS {
        if !names.iter().any(|n| viewport_width(n, width)) {
            report.findings.push(Finding {
                file: dir.display().to_string(),
                line: 0,
                rule: rule::VIEWPORTS,
                severity: Severity::Fail,
                message: format!("no screenshot at {width} px in the evidence directory (file name must contain {width} as its own number)"),
            });
        }
    }
}

fn collect_images(dir: &Path, names: &mut Vec<String>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_images(&path, names);
        } else if matches!(
            extension(&path).as_str(),
            ".png" | ".jpg" | ".jpeg" | ".webp"
        ) {
            names.push(entry.file_name().to_string_lossy().into_owned());
        }
    }
}

fn glow_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"(?i)box-shadow\s*:\s*0(?:px)?\s+0(?:px)?\s+([0-9]*\.?[0-9]+)px\s+(?:-?[0-9.]+px\s+)?(?:rgba?\(|hsla?\(|#)")
            .expect("static regex")
    })
}

fn hard_shadow_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"(?i)box-shadow\s*:\s*(-?[0-9]*\.?[0-9]+)px\s+(-?[0-9]*\.?[0-9]+)px\s+0(?:px)?\s+(?:-?[0-9.]+px\s+)?(?:rgba?\(|hsla?\(|#)")
            .expect("static regex")
    })
}

fn side_stripe_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"(?i)border-(?:left|right|inline-start|inline-end)\s*:\s*([0-9]*\.?[0-9]+)px")
            .expect("static regex")
    })
}

fn gradient_text_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r#"(?i)(?:-webkit-)?background-clip\s*:\s*text(?-u:\b)|backgroundClip\s*:\s*['"]text['"]"#)
            .expect("static regex")
    })
}

fn emoji_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"[\x{1F300}-\x{1FAFF}\x{2600}-\x{27BF}]").expect("static regex"))
}

fn index_number_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(
            r"(?-u:\b)0[1-9]\s*/\s*0[1-9](?-u:\b)|\(0[0-9]{2}\)|(?-u:\b)Step\s+[0-9]+(?-u:\b)",
        )
        .expect("static regex")
    })
}

/// The CSS pattern checks. Each hit is a WARN, because a brief can justify it.
fn lint_patterns(report: &mut Report, file: &str, text: &str) {
    let text = blank_matches(text, css_comment_re());
    for m in gradient_text_re().find_iter(&text) {
        report.findings.push(warn(file, &text, m.start(), rule::GRADIENT_TEXT,
            "gradient text (background-clip: text): emphasis should come from weight or size; keep a gradient only if the brief pins it".into()));
    }
    for caps in side_stripe_re().captures_iter(&text) {
        let px: f64 = caps[1].parse().unwrap_or(0.0);
        if px <= 1.0 {
            continue; // a 1px hairline is a divider, not a stripe
        }
        let whole = caps.get(0).expect("group 0");
        report.findings.push(warn(file, &text, whole.start(), rule::SIDE_STRIPE,
            format!("side-stripe border of {px}px on one edge: use a full border, a background tint, or no accent")));
    }
    for caps in glow_re().captures_iter(&text) {
        let whole = caps.get(0).expect("group 0");
        report.findings.push(warn(file, &text, whole.start(), rule::GLOW_SHADOW,
            "zero-offset coloured halo (box-shadow with no x/y offset): shadows need an offset and a soft blur to read as depth".into()));
    }
    for caps in hard_shadow_re().captures_iter(&text) {
        let x: f64 = caps[1].parse().unwrap_or(f64::NAN);
        let y: f64 = caps[2].parse().unwrap_or(f64::NAN);
        if x.is_nan() || y.is_nan() || (x == 0.0 && y == 0.0) {
            continue;
        }
        let whole = caps.get(0).expect("group 0");
        report.findings.push(warn(file, &text, whole.start(), rule::HARD_SHADOW,
            "hard offset shadow (zero blur): a block shadow is a costume unless the whole design is built on it".into()));
    }
}

fn warn(file: &str, text: &str, index: usize, rule: &'static str, message: String) -> Finding {
    Finding {
        file: file.to_owned(),
        line: line_of(text, index),
        rule,
        severity: Severity::Warn,
        message,
    }
}

/// House-rule checks a text scan can decide: emoji used as icons, and index numbering.
fn lint_tells(report: &mut Report, file: &str, text: &str, html: bool) {
    for m in emoji_re().find_iter(text) {
        report.findings.push(warn(file, text, m.start(), rule::EMOJI,
            format!("symbol {:?} used as an icon or decoration: draw icons from one set; emoji and glyphs do not share a stroke", m.as_str())));
    }
    if !html {
        return;
    }
    let visible = visible_html(text);
    for m in index_number_re().find_iter(&visible) {
        report.findings.push(warn(file, &visible, m.start(), rule::INDEX_NUMBER,
            format!("index numbering {:?}: keep it only if the content really is a sequence the reader needs", m.as_str())));
    }
}

/// Lints every file under `paths` and returns the report, findings sorted by file, line, and rule.
///
/// # Errors
///
/// I/O failures reading a path or a file under it.
pub fn lint(paths: &[PathBuf], options: &Options) -> io::Result<Report> {
    let files = collect_files(paths)?;
    let contents: Vec<String> = files
        .iter()
        .map(fs::read_to_string)
        .collect::<io::Result<_>>()?;

    let mut report = Report {
        files: files.len(),
        ..Report::default()
    };
    let tokens = collect_tokens(&contents);
    for (name, count) in &tokens.conflicts {
        report.findings.push(Finding {
            file: "(tokens)".into(),
            line: 0,
            rule: rule::TOKEN,
            severity: Severity::Warn,
            message: format!("--{name} is declared with {count} different values across the files; any contrast check that uses it is skipped"),
        });
    }

    for (path, text) in files.iter().zip(&contents) {
        let file = path.display().to_string();
        lint_image_hosts(&mut report, &file, text);
        if is_html(path) {
            lint_figures(&mut report, &file, text);
        }
        lint_fonts(&mut report, &file, text);
        lint_patterns(&mut report, &file, text);
        lint_tells(&mut report, &file, text, is_html(path));
        lint_contrast(&mut report, &file, text, &tokens);
    }
    lint_viewports(&mut report, options.evidence_dir.as_deref());

    report.findings.sort_by(|a, b| {
        a.file
            .cmp(&b.file)
            .then(a.line.cmp(&b.line))
            .then(a.rule.cmp(b.rule))
    });
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lint_text(name: &str, body: &str) -> Report {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(name);
        fs::write(&path, body).unwrap();
        lint(&[path], &Options::default()).unwrap()
    }

    #[test]
    fn low_contrast_rule_fails_and_high_contrast_passes() {
        let bad = lint_text(
            "a.css",
            ".x { color: #777; background-color: #fff; font-size: 16px; }\n",
        );
        assert_eq!(bad.count(rule::CONTRAST, Severity::Fail), 1);
        let good = lint_text(
            "b.css",
            ".x { color: #111; background-color: #fff; font-size: 16px; }\n",
        );
        assert_eq!(good.count(rule::CONTRAST, Severity::Fail), 0);
        assert_eq!(good.contrast_checked, 1);
    }

    #[test]
    fn unresolvable_background_is_counted_as_skipped() {
        let r = lint_text("c.css", ".x { color: #111; background: url(a.png); }\n");
        assert_eq!(r.contrast_checked, 0);
        assert_eq!(r.contrast_skipped, 1);
    }

    #[test]
    fn placeholder_host_is_a_failure() {
        let r = lint_text("d.html", r#"<img src="https://picsum.photos/200">"#);
        assert_eq!(r.count(rule::IMAGE_HOST, Severity::Fail), 1);
    }

    #[test]
    fn emoji_is_a_warning() {
        let r = lint_text("e.html", "<p>Ship it \u{1F680}</p>");
        assert_eq!(r.count(rule::EMOJI, Severity::Warn), 1);
    }

    #[test]
    fn missing_evidence_is_info_and_missing_widths_fail() {
        let r = lint(&[], &Options::default()).unwrap();
        assert_eq!(r.count(rule::VIEWPORTS, Severity::Info), 1);
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("1768.png"), "").unwrap();
        let r = lint(
            &[],
            &Options {
                evidence_dir: Some(dir.path().to_path_buf()),
            },
        )
        .unwrap();
        assert_eq!(
            r.count(rule::VIEWPORTS, Severity::Fail),
            3,
            "1768 must not count as 768"
        );
    }

    #[test]
    fn clamp_uses_the_smallest_bound() {
        assert_eq!(
            parse_size("clamp(14px, 2vw, 24px)").map(|(p, _)| p),
            Some(14.0)
        );
    }

    #[test]
    fn weights_are_hundreds_or_keywords() {
        assert_eq!(parse_weight("bold"), Some(700));
        assert_eq!(parse_weight("600"), Some(600));
        assert_eq!(parse_weight("650"), None);
    }
}
