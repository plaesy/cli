//! Unit tests for `harness.rs`, included via `#[path]`. The rendering and status logic is checked
//! here without touching `PATH`.

use plaesy_common::harness::{Harness, Readiness, HARNESSES};

use super::*;

fn sample(binary: Option<&'static str>, min: Option<&'static str>) -> Harness {
    Harness {
        id: "sample",
        binary,
        tui: true,
        chat: false,
        reviewer: true,
        min_version: min,
    }
}

#[test]
fn list_has_a_header_and_one_row_per_harness() {
    let text = render_list(HARNESSES);
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines.len(), HARNESSES.len() + 1);
    assert!(lines[0].starts_with("ID"));
    assert!(lines
        .iter()
        .any(|l| l.starts_with("claude-code") && l.contains("claude")));
}

#[test]
fn list_marks_unverified_binaries_and_minimum_versions() {
    let text = render_list(HARNESSES);
    assert!(text.lines().any(|l| {
        l.starts_with("docker-agent") && l.contains("(unverified)") && l.ends_with("1.150.0")
    }));
    assert!(text
        .lines()
        .any(|l| l.starts_with("mimo-code") && l.ends_with("0.1.14")));
}

#[test]
fn check_line_states_each_outcome() {
    let h = sample(Some("sample-agent"), None);
    let found = Readiness::Found {
        path: "/bin/sample-agent".into(),
        version: Some("1.2.3".to_owned()),
    };
    assert!(render_check_line(&h, &found).ends_with("installed v1.2.3 at /bin/sample-agent"));

    let unknown = Readiness::Found {
        path: "/bin/sample-agent".into(),
        version: None,
    };
    assert!(render_check_line(&h, &unknown).contains("version unknown"));
    assert!(render_check_line(&h, &Readiness::Missing).ends_with("missing: not on PATH"));
    assert!(render_check_line(&h, &Readiness::Unverified).contains("unverified"));

    let old = Readiness::Outdated {
        path: "/bin/sample-agent".into(),
        version: "0.1.13".to_owned(),
        min: "0.1.14",
    };
    assert!(render_check_line(&h, &old).contains("needs >= 0.1.14"));
}

#[test]
fn only_a_found_harness_counts_as_ready() {
    assert!(is_ready(&Readiness::Found {
        path: "/bin/x".into(),
        version: None,
    }));
    assert!(!is_ready(&Readiness::Missing));
    assert!(!is_ready(&Readiness::Unverified));
    assert!(!is_ready(&Readiness::Outdated {
        path: "/bin/x".into(),
        version: "0.0.1".to_owned(),
        min: "0.1.0",
    }));
}
