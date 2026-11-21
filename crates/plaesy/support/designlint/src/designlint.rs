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

mod colour;
mod fonts;
mod images;
mod tokens;
use colour::*;
use fonts::*;
use images::*;
use tokens::*;

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
#[path = "designlint_test.rs"]
mod designlint_test;
