//! Uji konformansi: contoh resmi harus lolos, fixture negatif harus gagal.
//! Berkas diambil dari `docs/adr/` di root repo (lihat ADR 0002 dan prototipe Node).

use std::fs;
use std::path::PathBuf;

use plaesy_domain_flow::{ir, validate};
use serde_json::Value;

fn adr_dir() -> PathBuf {
    // crates/plaesy/domain/flow -> root repo
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../../docs/adr")
}

fn load(rel: &str) -> Value {
    let path = adr_dir().join(rel);
    let text = fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("gagal membaca {}: {e}", path.display()));
    serde_json::from_str(&text).unwrap_or_else(|e| panic!("JSON rusak {}: {e}", path.display()))
}

#[test]
fn official_examples_pass() {
    for name in ["kirim-faktur.json", "retry-parallel-branches.json"] {
        let doc = load(&format!("examples/flow-ir-1/{name}"));
        assert!(ir::is_flow(&doc), "{name} harus flow-ir/1");
        let errs = validate(&doc);
        assert!(errs.is_empty(), "{name} gagal: {errs:?}");
    }
}

#[test]
fn semantic_negative_fixtures_fail() {
    for name in ["missing-on-error-target.json", "undeclared-flow-input.json"] {
        let doc = load(&format!("examples-negative-semantic/flow-ir-1/{name}"));
        assert!(!validate(&doc).is_empty(), "{name} seharusnya gagal");
    }
}
