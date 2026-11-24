use super::*;
use serde_json::json;
use std::path::PathBuf;

fn example(name: &str) -> Value {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../../docs/adr/examples/flow-ir-1")
        .join(name);
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    serde_json::from_str(&text).unwrap()
}

#[test]
fn rust_action_with_retry_and_timeout_is_translated() {
    let generated = generate(&example("kirim-faktur.json")).unwrap();
    let source = &generated.source;
    assert!(source.contains("pub fn kirim_faktur(input: &Value, runner: &dyn ActionRunner)"));
    assert!(source.contains("with_retry(Some(&RetryPolicy { max_attempts: 3, base_secs: 1, multiplier: 2, cap_secs: 60 })"));
    assert!(source.contains(
        r#"runner.action("billing.get_invoice", &json!({ "id": dig(input, &["invoice_id"]) }), 30)"#
    ));
    assert!(source.contains(r#"scope.insert("m1".to_owned(), value);"#));
}

#[test]
fn rust_pass_module_is_translated_with_string_literal() {
    let generated = generate(&example("kirim-faktur.json")).unwrap();
    assert!(generated
        .source
        .contains(r#"scope.insert("m7".to_owned(), json!({ "status": json!("skipped") }));"#));
    assert!(generated
        .report
        .mapped
        .iter()
        .any(|m| m.module == "m7" && m.kind == "pass"));
}

#[test]
fn rust_branch_becomes_if_with_path_comparison() {
    let generated = generate(&example("kirim-faktur.json")).unwrap();
    assert!(generated.source.contains(
        r#"if dig(scope.get("m1").unwrap_or(&Value::Null), &["invoice", "status"]) == json!("draft") {"#
    ));
    assert!(generated
        .report
        .mapped
        .iter()
        .any(|m| m.module == "m2" && m.kind == "branch_one"));
}

#[test]
fn rust_unsupported_modules_are_reported_and_return_an_error() {
    let generated = generate(&example("kirim-faktur.json")).unwrap();
    assert!(generated
        .report
        .lost
        .iter()
        .any(|l| l.module == "m3" && l.kind == "approval"));
    assert!(generated
        .source
        .contains(r#"return Err(FlowError { module: "m3".to_owned()"#));
}

#[test]
fn rust_every_module_is_recorded_exactly_once() {
    let doc = example("kirim-faktur.json");
    let generated = generate(&doc).unwrap();
    let total = doc["modules"].as_array().unwrap().len();
    let mut ids: Vec<&str> = generated
        .report
        .mapped
        .iter()
        .map(|m| m.module.as_str())
        .chain(generated.report.lost.iter().map(|l| l.module.as_str()))
        .collect();
    assert_eq!(ids.len(), total, "setiap modul harus tercatat");
    ids.sort_unstable();
    ids.dedup();
    assert_eq!(ids.len(), total, "tidak boleh ada modul tercatat dua kali");
}

#[test]
fn rust_for_each_loops_over_the_iterator_with_item_in_scope() {
    let mut doc = example("kirim-faktur.json");
    // Ubah m5 agar didukung: on_error "fail", dan input item.value berjalan di dalam loop.
    doc["modules"][4]["on_error"] = json!("fail");
    let generated = generate(&doc).unwrap();
    assert!(generated.source.contains(
        r#"for item in dig(scope.get("m1").unwrap_or(&Value::Null), &["invoice", "lines"]).as_array().cloned().unwrap_or_default() {"#
    ));
    assert!(generated.source.contains(r#"dig(&item, &["value"])"#));
    assert!(generated
        .report
        .mapped
        .iter()
        .any(|m| m.module == "m5" && m.kind == "action"));
}

#[test]
fn rust_action_with_continue_on_error_is_a_loss() {
    let generated = generate(&example("kirim-faktur.json")).unwrap();
    assert!(generated
        .report
        .lost
        .iter()
        .any(|l| l.module == "m5" && l.reason.contains("on_error")));
}

#[test]
fn rust_unsupported_expression_makes_the_module_a_loss() {
    let mut doc = example("kirim-faktur.json");
    doc["modules"][6]["inputs"]["status"] = json!("m1.invoice.status == 'paid'");
    let generated = generate(&doc).unwrap();
    assert!(generated
        .report
        .lost
        .iter()
        .any(|l| l.module == "m7" && l.reason.contains("di luar cakupan")));
}

#[test]
fn rust_invalid_flow_is_refused() {
    let mut doc = example("kirim-faktur.json");
    doc["modules"] = json!([]);
    assert!(generate(&doc).is_err());
}
