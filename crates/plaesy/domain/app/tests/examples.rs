//! Uji konformansi: contoh resmi lolos, fixture negatif gagal. Berkas dari `docs/adr/`.

use std::fs;
use std::path::PathBuf;

use plaesy_domain_app::validate;
use serde_json::Value;

fn load(rel: &str) -> Value {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../../docs/adr")
        .join(rel);
    let text = fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("gagal membaca {}: {e}", path.display()));
    serde_json::from_str(&text).unwrap_or_else(|e| panic!("JSON rusak {}: {e}", path.display()))
}

#[test]
fn official_examples_pass() {
    for rel in "examples/app-ir-1/order-desk.json examples/app-ir-1/login-and-stock.json"
        .split_whitespace()
    {
        let errs = validate(&load(rel));
        assert!(errs.is_empty(), "{rel} gagal: {errs:?}");
    }
}

#[test]
fn negative_fixture_fails() {
    let errs = validate(&load(
        "examples-negative-semantic/app-ir-1/missing-start-screen.json",
    ));
    assert!(
        !errs.is_empty(),
        "examples-negative-semantic/app-ir-1/missing-start-screen.json seharusnya gagal"
    );
}
