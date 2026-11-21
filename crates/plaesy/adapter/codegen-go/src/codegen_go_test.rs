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
fn go_unsupported_modules_are_reported_and_return_an_error() {
    let generated = generate(&example("kirim-faktur.json")).unwrap();
    let lost: Vec<&str> = generated
        .report
        .lost
        .iter()
        .map(|l| l.module.as_str())
        .collect();
    assert!(lost.contains(&"m2"), "branch_one harus tercatat: {lost:?}");
    assert!(lost.contains(&"m3"), "approval harus tercatat: {lost:?}");
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
