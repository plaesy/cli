//! Unit tests for `flow_view.rs`: ARIA attributes, text status, and escaping.

use super::*;

fn sample() -> FlowView {
    FlowView {
        modules: vec![Entry {
            name: "auth".to_string(),
            status: Status::Ok,
        }],
        flows: vec![Entry {
            name: "login".to_string(),
            status: Status::Failed,
        }],
    }
}

#[test]
fn lists_carry_role_and_aria_label() {
    let html = render(&sample());
    assert!(html.contains("role=\"list\" aria-label=\"Daftar modul\""));
    assert!(html.contains("role=\"list\" aria-label=\"Daftar alur\""));
    assert!(html.contains("role=\"listitem\""));
}

#[test]
fn status_is_shown_as_text_not_only_color() {
    let html = render(&sample());
    assert!(html.contains("Status: Berhasil"));
    assert!(html.contains("Status: Gagal"));
    assert!(html.contains("class=\"flow-item status-failed\""));
}

#[test]
fn empty_list_says_so_in_text() {
    let html = render(&FlowView::default());
    assert!(html.contains("Tidak ada modul."));
    assert!(html.contains("Tidak ada alur."));
    assert!(!html.contains("role=\"list\""));
}

#[test]
fn names_are_escaped() {
    let view = FlowView {
        modules: vec![Entry {
            name: "<script>alert(1)</script>".to_string(),
            status: Status::Draft,
        }],
        flows: Vec::new(),
    };
    let html = render(&view);
    assert!(!html.contains("<script>"));
    assert!(html.contains("&lt;script&gt;alert(1)&lt;/script&gt;"));
    assert!(html.contains("Status: Draf"));
}

#[test]
fn escape_html_covers_all_special_characters() {
    assert_eq!(escape_html("&<>\"'"), "&amp;&lt;&gt;&quot;&#39;");
    assert_eq!(escape_html("plain"), "plain");
}
