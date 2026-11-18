//! Unit tests for `tests` split out of `designlint.rs`.
//! Kept as a separate file so they are easy to find; included via `#[path]`.

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
