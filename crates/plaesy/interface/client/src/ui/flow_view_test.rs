use super::*;

fn module(id: &str, kind: &str, inputs: &[(&str, &str)]) -> ModuleView {
    ModuleView {
        id: id.to_owned(),
        kind: kind.to_owned(),
        inputs: inputs
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect(),
    }
}

#[test]
fn escape_neutralizes_markup() {
    assert_eq!(
        escape("<script>\"x\" & 'y'</script>"),
        "&lt;script&gt;&quot;x&quot; &amp; &#39;y&#39;&lt;/script&gt;"
    );
}

#[test]
fn render_lists_modules_edges_and_is_escaped() {
    let modules = vec![
        module("m1", "action", &[]),
        module("m2", "pass", &[("total", "m1.amount")]),
    ];
    let html = render_flow("<alur>", &modules);
    assert!(html.contains("&lt;alur&gt;"));
    assert!(html.contains("m1 &rarr; m2"));
    assert!(html.contains("Tidak ada masalah lokal."));
    assert!(!html.contains("<alur>"));
}

#[test]
fn render_reports_local_issues() {
    let modules = vec![module("m1", "action", &[]), module("m1", "pass", &[])];
    let html = render_flow("flow", &modules);
    assert!(html.contains("id modul ganda: m1"));
}
