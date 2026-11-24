//! Unit tests for `export.rs`: CSV escaping, column order, and the Markdown table layout.

use super::*;

fn sample_task() -> TaskRow {
    TaskRow {
        id: "add-login".to_owned(),
        status: "doing".to_owned(),
        kind: "task".to_owned(),
        parent: Some("auth-epic".to_owned()),
        depends_on: vec!["setup-db".to_owned(), "setup-env".to_owned()],
        labels: vec!["backend".to_owned()],
        assignee: None,
        milestone: Some("v1".to_owned()),
    }
}

fn sample_run() -> RunRow {
    RunRow {
        provider: "openai".to_owned(),
        calls: 3,
        input_tokens: 1200,
        output_tokens: 300,
        calls_without_token_counts: 1,
        wall_ms: 4500,
    }
}

#[test]
fn csv_escape_leaves_plain_text_unchanged() {
    assert_eq!(csv_escape("abc"), "abc");
    assert_eq!(csv_escape(""), "");
}

#[test]
fn csv_escape_quotes_fields_with_comma_or_line_break() {
    assert_eq!(csv_escape("a,b"), "\"a,b\"");
    assert_eq!(csv_escape("a\nb"), "\"a\nb\"");
    assert_eq!(csv_escape("a\r\nb"), "\"a\r\nb\"");
}

#[test]
fn csv_escape_doubles_embedded_quotes() {
    assert_eq!(csv_escape("say \"hi\""), "\"say \"\"hi\"\"\"");
    assert_eq!(csv_escape("\""), "\"\"\"\"");
}

#[test]
fn csv_escape_guards_leading_formula_characters() {
    assert_eq!(csv_escape("=SUM(A1)"), "'=SUM(A1)");
    assert_eq!(csv_escape("+1"), "'+1");
    assert_eq!(csv_escape("-cmd"), "'-cmd");
    assert_eq!(csv_escape("@user"), "'@user");
    assert_eq!(csv_escape("\tx"), "'\tx");
    // A guarded field that also needs quoting is both prefixed and quoted.
    assert_eq!(csv_escape("=a,b"), "\"'=a,b\"");
}

#[test]
fn csv_escape_does_not_guard_inner_formula_characters() {
    assert_eq!(csv_escape("a=b"), "a=b");
    assert_eq!(csv_escape("x-y"), "x-y");
}

#[test]
fn task_csv_header_follows_column_order() {
    let csv = tasks_csv(&[]);
    assert_eq!(
        csv,
        "id,status,type,parent,depends_on,labels,assignee,milestone\r\n"
    );
}

#[test]
fn task_csv_row_follows_column_order_and_joins_lists() {
    let csv = tasks_csv(&[sample_task()]);
    let mut lines = csv.split("\r\n");
    assert_eq!(
        lines.next(),
        Some("id,status,type,parent,depends_on,labels,assignee,milestone")
    );
    assert_eq!(
        lines.next(),
        Some("add-login,doing,task,auth-epic,setup-db;setup-env,backend,,v1")
    );
    assert_eq!(lines.next(), Some(""));
    assert_eq!(lines.next(), None);
}

#[test]
fn task_csv_escapes_label_with_comma() {
    let mut task = sample_task();
    task.labels = vec!["a,b".to_owned()];
    let csv = tasks_csv(&[task]);
    assert!(csv.contains("\"a,b\""), "{csv}");
}

#[test]
fn task_markdown_header_and_separator_follow_column_order() {
    let md = tasks_markdown(&[]);
    let mut lines = md.lines();
    assert_eq!(
        lines.next(),
        Some("| id | status | type | parent | depends_on | labels | assignee | milestone |")
    );
    assert_eq!(
        lines.next(),
        Some("| --- | --- | --- | --- | --- | --- | --- | --- |")
    );
    assert_eq!(lines.next(), None);
}

#[test]
fn task_markdown_row_follows_column_order() {
    let md = tasks_markdown(&[sample_task()]);
    let row = md.lines().nth(2);
    assert_eq!(
        row,
        Some("| add-login | doing | task | auth-epic | setup-db;setup-env | backend |  | v1 |")
    );
}

#[test]
fn task_markdown_escapes_pipes_and_flattens_line_breaks() {
    let mut task = sample_task();
    task.labels = vec!["a|b".to_owned()];
    task.milestone = Some("line1\nline2".to_owned());
    let md = tasks_markdown(&[task]);
    let row = md.lines().nth(2).unwrap_or_default();
    assert!(row.contains("a\\|b"), "{row}");
    assert!(row.ends_with("| line1 line2 |"), "{row}");
    assert_eq!(md.lines().count(), 3);
}

#[test]
fn run_csv_header_and_row_follow_column_order() {
    let csv = runs_csv(&[sample_run()]);
    let mut lines = csv.split("\r\n");
    assert_eq!(
        lines.next(),
        Some("provider,calls,input_tokens,output_tokens,calls_without_token_counts,wall_ms")
    );
    assert_eq!(lines.next(), Some("openai,3,1200,300,1,4500"));
}

#[test]
fn run_markdown_header_and_row_follow_column_order() {
    let md = runs_markdown(&[sample_run()]);
    let mut lines = md.lines();
    assert_eq!(
        lines.next(),
        Some("| provider | calls | input_tokens | output_tokens | calls_without_token_counts | wall_ms |")
    );
    assert_eq!(lines.next(), Some("| --- | --- | --- | --- | --- | --- |"));
    assert_eq!(lines.next(), Some("| openai | 3 | 1200 | 300 | 1 | 4500 |"));
    assert_eq!(lines.next(), None);
}

#[test]
fn column_constants_match_rendered_headers() {
    let md_tasks = tasks_markdown(&[]);
    let header = md_tasks.lines().next().unwrap_or_default();
    assert_eq!(header, format!("| {} |", TASK_COLUMNS.join(" | ")));

    let md_runs = runs_markdown(&[]);
    let header = md_runs.lines().next().unwrap_or_default();
    assert_eq!(header, format!("| {} |", RUN_COLUMNS.join(" | ")));
}

#[test]
fn format_parses_known_names_and_rejects_others() {
    assert_eq!("md".parse::<Format>(), Ok(Format::Markdown));
    assert_eq!("markdown".parse::<Format>(), Ok(Format::Markdown));
    assert_eq!("csv".parse::<Format>(), Ok(Format::Csv));
    assert_eq!(
        "pdf".parse::<Format>(),
        Err(Error::UnknownFormat("pdf".to_owned()))
    );
}

#[test]
fn render_dispatches_on_format() {
    let rows = [sample_task()];
    assert_eq!(render_tasks(&rows, Format::Csv), tasks_csv(&rows));
    assert_eq!(render_tasks(&rows, Format::Markdown), tasks_markdown(&rows));
    let runs = [sample_run()];
    assert_eq!(render_runs(&runs, Format::Csv), runs_csv(&runs));
    assert_eq!(render_runs(&runs, Format::Markdown), runs_markdown(&runs));
}
