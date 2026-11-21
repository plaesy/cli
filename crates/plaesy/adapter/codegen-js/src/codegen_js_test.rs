use super::*;
use std::path::PathBuf;

fn example(name: &str) -> Value {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../../docs/adr/examples/flow-ir-1")
        .join(name);
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    serde_json::from_str(&text).unwrap()
}

#[test]
fn js_action_with_retry_and_timeout_is_translated() {
    let generated = generate(&example("kirim-faktur.json")).unwrap();
    let source = &generated.source;
    assert!(source.contains("export async function kirimFaktur(input, ctx)"));
    assert!(source.contains("scope.m1 = await ctx.withRetry("));
    assert!(source.contains(
        r#"ctx.action("billing.get_invoice", { id: input.invoice_id }, { timeoutSecs: 30 })"#
    ));
    assert!(source.contains(r#""maxAttempts":3"#));
}

#[test]
fn js_pass_module_is_translated_with_string_literal() {
    let generated = generate(&example("kirim-faktur.json")).unwrap();
    assert!(generated
        .source
        .contains("scope.m7 = { status: \"skipped\" };"));
    assert!(generated
        .report
        .mapped
        .iter()
        .any(|m| m.module == "m7" && m.kind == "pass"));
}

#[test]
fn js_unsupported_modules_are_reported_and_throw_at_runtime() {
    let generated = generate(&example("kirim-faktur.json")).unwrap();
    let lost: Vec<&str> = generated
        .report
        .lost
        .iter()
        .map(|l| l.module.as_str())
        .collect();
    assert!(lost.contains(&"m2"), "branch_one harus tercatat: {lost:?}");
    assert!(lost.contains(&"m3"), "approval harus tercatat: {lost:?}");
    // Tidak ada modul yang dilewati diam-diam: setiap yang hilang punya throw di kodenya.
    assert!(generated.source.contains("belum didukung codegen-js"));
    assert!(generated
        .source
        .contains("throw new Error(\"modul m2 (branch_one) belum didukung"));
}

#[test]
fn js_every_module_is_either_mapped_or_lost() {
    let doc = example("kirim-faktur.json");
    let generated = generate(&doc).unwrap();
    let total = doc["modules"].as_array().unwrap().len();
    assert_eq!(
        generated.report.mapped.len() + generated.report.lost.len(),
        total
    );
}

#[test]
fn js_invalid_flow_is_refused() {
    let mut doc = example("kirim-faktur.json");
    doc["modules"] = json!([]);
    assert!(generate(&doc).is_err());
}

#[test]
fn js_unsupported_expression_makes_the_module_a_loss() {
    let mut doc = example("kirim-faktur.json");
    // Ekspresi dengan operator di luar cakupan: modul ini harus menjadi kehilangan.
    doc["modules"][6]["inputs"]["status"] = json!("m1.invoice.status == 'paid'");
    let generated = generate(&doc).unwrap();
    assert!(generated
        .report
        .lost
        .iter()
        .any(|l| l.module == "m7" && l.reason.contains("di luar cakupan")));
}
