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
fn go_action_with_retry_and_timeout_is_translated() {
    let generated = generate(&example("kirim-faktur.json")).unwrap();
    let source = &generated.source;
    assert!(source.contains("package flows"));
    assert!(source.contains("func KirimFaktur(input map[string]any, ctx Context)"));
    assert!(source.contains(r#"ctx.WithRetry(RetryPolicy{Kind: "exponential", MaxAttempts: 3"#));
    assert!(source.contains(r#"ctx.Action("billing.get_invoice", map[string]any{"id": lookup(input, "invoice_id")}, ActionOptions{TimeoutSecs: 30})"#));
}

#[test]
fn go_pass_module_is_translated_with_string_literal() {
    let generated = generate(&example("kirim-faktur.json")).unwrap();
    assert!(generated
        .source
        .contains(r#"scope["m7"] = map[string]any{"status": "skipped"}"#));
    assert!(generated
        .report
        .mapped
        .iter()
        .any(|m| m.module == "m7" && m.kind == "pass"));
}

#[test]
fn go_branch_one_becomes_if_and_unsupported_member_returns_inside_it() {
    let generated = generate(&example("kirim-faktur.json")).unwrap();
    assert!(generated
        .report
        .mapped
        .iter()
        .any(|m| m.module == "m2" && m.kind == "branch_one"));
    assert!(generated
        .source
        .contains("if lookup(scope, \"m1\", \"invoice\", \"status\") == \"draft\" {"));
    assert!(generated.source.contains("} else {"));
    assert!(generated
        .report
        .lost
        .iter()
        .any(|l| l.module == "m3" && l.kind == "approval"));
    assert!(generated
        .source
        .contains("return nil, fmt.Errorf(\"modul m3 (approval) belum didukung"));
    assert!(generated
        .source
        .contains("scope[\"m7\"] = map[string]any{\"status\": \"skipped\"}"));
}

#[test]
fn go_branch_not_equal_keeps_the_operator() {
    let mut doc = example("kirim-faktur.json");
    doc["modules"][1]["branches"][0]["when"] = json!("m1.invoice.status != 'paid'");
    let generated = generate(&doc).unwrap();
    assert!(generated
        .source
        .contains("if lookup(scope, \"m1\", \"invoice\", \"status\") != \"paid\" {"));
}

#[test]
fn go_numeric_condition_compares_as_float64_like_json_decoding() {
    let mut doc = example("kirim-faktur.json");
    doc["modules"][1]["branches"][0]["when"] = json!("m1.invoice.count == 2");
    let generated = generate(&doc).unwrap();
    assert!(generated
        .source
        .contains("if lookup(scope, \"m1\", \"invoice\", \"count\") == float64(2) {"));
}

#[test]
fn go_branch_condition_outside_scope_loses_branch_and_its_members() {
    let mut doc = example("kirim-faktur.json");
    doc["modules"][1]["branches"][0]["when"] = json!("m1.invoice.total > 3");
    let generated = generate(&doc).unwrap();
    assert!(generated
        .report
        .lost
        .iter()
        .any(|l| l.module == "m2" && l.reason.contains("di luar cakupan")));
    assert!(generated
        .report
        .lost
        .iter()
        .any(|l| l.module == "m3" && l.reason.contains("m2")));
    assert!(generated
        .source
        .contains("return nil, fmt.Errorf(\"modul m2 (branch_one) belum didukung"));
}

#[test]
fn go_every_module_is_either_mapped_or_lost() {
    let doc = example("kirim-faktur.json");
    let generated = generate(&doc).unwrap();
    let total = doc["modules"].as_array().unwrap().len();
    assert_eq!(
        generated.report.mapped.len() + generated.report.lost.len(),
        total
    );
}

#[test]
fn go_invalid_flow_is_refused() {
    let mut doc = example("kirim-faktur.json");
    doc["modules"] = json!([]);
    assert!(generate(&doc).is_err());
}

#[test]
fn go_unsupported_expression_makes_the_module_a_loss() {
    let mut doc = example("kirim-faktur.json");
    doc["modules"][6]["inputs"]["status"] = json!("m1.invoice.status == 'paid'");
    let generated = generate(&doc).unwrap();
    assert!(generated
        .report
        .lost
        .iter()
        .any(|l| l.module == "m7" && l.reason.contains("di luar cakupan")));
}

/// Memeriksa sintaks dengan `gofmt -e` bila Go tersedia. Tanpa Go, tes ini dilewati dengan pesan.
#[test]
fn go_generated_package_has_valid_syntax_when_go_is_available() {
    if std::process::Command::new("gofmt")
        .arg("-h")
        .output()
        .is_err()
    {
        eprintln!("LEWAT: gofmt tidak terpasang, pemeriksaan sintaks Go dilewati");
        return;
    }
    let generated = generate(&example("kirim-faktur.json")).unwrap();
    let path = std::env::temp_dir().join("plaesy-codegen-go-syntax.go");
    std::fs::write(&path, &generated.source).unwrap();
    let output = std::process::Command::new("gofmt")
        .arg("-e")
        .arg(&path)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "gofmt -e gagal: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn go_for_each_translates_when_its_body_is_supported() {
    let mut doc = example("kirim-faktur.json");
    doc["modules"][4]["on_error"] = json!("fail");
    let generated = generate(&doc).unwrap();
    assert!(generated
        .report
        .mapped
        .iter()
        .any(|m| m.module == "m4" && m.kind == "for_each"));
    assert!(generated
        .report
        .mapped
        .iter()
        .any(|m| m.module == "m5" && m.kind == "action"));
    assert!(generated
        .source
        .contains(r#"for _, item := range lookupList(scope, "m1", "invoice", "lines") {"#));
    assert!(generated.source.contains(
        r#"scope["m5"], err = ctx.Action("billing.check_line", map[string]any{"line": item}, ActionOptions{TimeoutSecs: 0})"#
    ));
    assert!(generated.source.contains("parallelism 10 dicatat"));
}

#[test]
fn go_for_each_stays_lost_when_body_uses_on_error_continue() {
    let generated = generate(&example("kirim-faktur.json")).unwrap();
    let m4 = generated
        .report
        .lost
        .iter()
        .find(|l| l.module == "m4")
        .expect("m4 harus kehilangan");
    assert!(m4.reason.contains("on_error"), "{}", m4.reason);
    assert!(generated
        .report
        .lost
        .iter()
        .any(|l| l.module == "m5" && l.reason.contains("anggota m4")));
}

#[test]
fn go_for_each_with_skip_failures_is_a_loss() {
    let mut doc = example("kirim-faktur.json");
    doc["modules"][4]["on_error"] = json!("fail");
    doc["modules"][3]["skip_failures"] = json!(true);
    let generated = generate(&doc).unwrap();
    let m4 = generated
        .report
        .lost
        .iter()
        .find(|l| l.module == "m4")
        .unwrap();
    assert!(m4.reason.contains("skip_failures"));
}

#[test]
fn go_item_value_outside_a_loop_is_refused() {
    let mut doc = example("kirim-faktur.json");
    doc["modules"][6]["inputs"]["status"] = json!("item.value");
    let generated = generate(&doc).unwrap();
    assert!(generated
        .report
        .lost
        .iter()
        .any(|l| l.module == "m7" && l.reason.contains("di luar cakupan")));
}
