//! `images` helpers split out of `designlint.rs`.

use super::*;

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

pub(super) fn lint_image_hosts(report: &mut Report, file: &str, text: &str) {
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
pub(super) fn lint_figures(report: &mut Report, file: &str, text: &str) {
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
