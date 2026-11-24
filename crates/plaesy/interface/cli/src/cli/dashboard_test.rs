//! Unit tests for `dashboard.rs`: escaping, task counts, usage totals, and page rendering.

use super::*;

fn task(status: Status, name: &str) -> TaskRef {
    TaskRef {
        spec: "spec".to_owned(),
        status,
        name: name.to_owned(),
        title: String::new(),
        phase: String::new(),
    }
}

fn record(provider: &str, input: Option<i64>, output: Option<i64>, wall_ms: i64) -> Record {
    Record {
        at: "2026-11-23T10:00:00Z".to_owned(),
        provider: provider.to_owned(),
        chain: String::new(),
        billable: false,
        input_tokens: input,
        output_tokens: output,
        wall_ms,
        task: String::new(),
        cost_micro_usd: None,
    }
}

fn snapshot(project: &str, usage: Option<UsageSummary>) -> Snapshot {
    Snapshot {
        project: project.to_owned(),
        generated_at: "2026-11-23T10:00:00Z".to_owned(),
        task_counts: count_tasks(&[task(Status::Todo, "a")]),
        usage,
    }
}

#[test]
fn escape_html_replaces_all_special_characters() {
    assert_eq!(
        escape_html(r#"<script>alert("x') & 'y'</script>"#),
        "&lt;script&gt;alert(&quot;x&#x27;) &amp; &#x27;y&#x27;&lt;/script&gt;"
    );
}

#[test]
fn escape_html_keeps_plain_text_and_unicode() {
    assert_eq!(escape_html("tugas 1 — selesai"), "tugas 1 — selesai");
    assert_eq!(escape_html(""), "");
}

#[test]
fn escape_html_does_not_double_escape_ampersands() {
    assert_eq!(escape_html("&lt;"), "&amp;lt;");
}

#[test]
fn count_tasks_reports_every_status_in_lifecycle_order() {
    let refs = [
        task(Status::Backlog, "one"),
        task(Status::Backlog, "two"),
        task(Status::Done, "three"),
        task(Status::Blocked, "four"),
    ];
    let counts = count_tasks(&refs);
    assert_eq!(
        counts,
        vec![
            (Status::Backlog, 2),
            (Status::Todo, 0),
            (Status::Doing, 0),
            (Status::Review, 0),
            (Status::Done, 1),
            (Status::Blocked, 1),
        ]
    );
}

#[test]
fn count_tasks_on_empty_project_is_all_zero() {
    let counts = count_tasks(&[]);
    assert_eq!(counts.len(), STATUSES.len());
    assert!(counts.iter().all(|(_, n)| *n == 0));
}

#[test]
fn summarize_usage_without_records_is_none() {
    assert!(summarize_usage(&[]).is_none());
}

#[test]
fn summarize_usage_sums_totals_and_counts_unknown_tokens() {
    let records = [
        record("alpha", Some(10), Some(5), 100),
        record("alpha", None, None, 40),
        record("beta", Some(1), Some(1), 7),
    ];
    let summary = summarize_usage(&records).expect("records are present, so a summary exists");

    assert_eq!(summary.total.calls, 3);
    assert_eq!(summary.total.input_tokens, 11);
    assert_eq!(summary.total.output_tokens, 6);
    assert_eq!(summary.total.unknown_tokens, 1);
    assert_eq!(summary.total.wall_ms, 147);

    let alpha = summary.by_provider.get("alpha").expect("alpha is recorded");
    assert_eq!(alpha.calls, 2);
    assert_eq!(alpha.input_tokens, 10);
    assert_eq!(alpha.unknown_tokens, 1);
    let beta = summary.by_provider.get("beta").expect("beta is recorded");
    assert_eq!(beta.calls, 1);
}

#[test]
fn render_shows_task_counts_and_total() {
    let html = render_html(&snapshot("proyek", None));
    assert!(html.contains("<td>todo</td><td class=\"num\">1</td>"));
    assert!(html.contains("<td>Total</td><td class=\"num\">1</td>"));
    assert!(html.contains("No delegate usage recorded."));
}

#[test]
fn render_shows_usage_totals_when_present() {
    let records = [record("alpha", Some(10), Some(5), 100)];
    let html = render_html(&snapshot("proyek", summarize_usage(&records)));
    assert!(html.contains("<td>alpha</td><td class=\"num\">1</td>"));
    assert!(html.contains("<td>Total</td><td class=\"num\">1</td><td class=\"num\">10</td>"));
    assert!(!html.contains("No delegate usage recorded."));
}

#[test]
fn render_escapes_project_and_provider_names() {
    let records = [record("<img src=x onerror=alert(1)>", Some(1), Some(1), 1)];
    let html = render_html(&snapshot(
        "<script>alert('p')</script>",
        summarize_usage(&records),
    ));
    assert!(!html.contains("<script>alert"));
    assert!(!html.contains("<img src=x"));
    assert!(html.contains("&lt;script&gt;alert(&#x27;p&#x27;)&lt;/script&gt;"));
    assert!(html.contains("&lt;img src=x onerror=alert(1)&gt;"));
}

#[test]
fn render_has_no_scripts_or_external_references() {
    let records = [record("alpha", Some(1), Some(1), 1)];
    let html = render_html(&snapshot("proyek", summarize_usage(&records)));
    let lower = html.to_ascii_lowercase();
    assert!(!lower.contains("<script"));
    assert!(!lower.contains("<link"));
    assert!(!lower.contains("src="));
    assert!(!lower.contains("http://"));
    assert!(!lower.contains("https://"));
    assert!(lower.starts_with("<!doctype html>"));
}

#[test]
fn write_dashboard_creates_parent_folders() {
    let temp = tempfile::tempdir().unwrap();
    let out = temp.path().join("nested").join("dashboard.html");
    write_dashboard(&out, "<!doctype html>\n").unwrap();
    assert_eq!(std::fs::read_to_string(&out).unwrap(), "<!doctype html>\n");
}
