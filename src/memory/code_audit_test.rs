//! Unit tests for `tests` split out of `code_audit.rs`.
//! Kept as a separate file so they are easy to find; included via `#[path]`.

use super::*;
use std::fs;
use tempfile::tempdir;
#[test]
fn extracts_duplicates() {
    let d = tempdir().unwrap();
    fs::write(
        d.path().join("a.dart"),
        "class A { String greet(String name) { return name.trim(); } }",
    )
    .unwrap();
    fs::write(
        d.path().join("b.dart"),
        "class B { String greet( String name ) { return name.trim(); } }",
    )
    .unwrap();
    let r = audit(d.path(), Language::Dart).unwrap();
    assert_eq!(r.declarations.len(), 2);
    assert_eq!(r.exact_duplicates.len(), 1);
    assert_eq!(
        r.exact_duplicates[0].classification,
        "cross_class_collision"
    );
    // Exact duplicates are not duplicated into the semantic report.
    assert_eq!(r.semantic_duplicates.len(), 0);
}
#[test]
fn find_returns_exact_reuse_and_cross_class_candidates() {
    let d = tempdir().unwrap();
    fs::write(
        d.path().join("a.dart"),
        "class A { String greet(String name) { return name.trim(); } }",
    )
    .unwrap();
    fs::write(
        d.path().join("b.dart"),
        "class B { String greet(String name) { return name.trim(); } }",
    )
    .unwrap();
    let r = audit(d.path(), Language::Dart).unwrap();
    let found = find(&r.declarations, "greet", None, Vec::new());
    assert_eq!(found.decision, "reuse_exact");
    assert_eq!(found.candidates.len(), 2);
    assert!(found
        .candidates
        .iter()
        .all(|c| c.match_kind == "exact_name"));
}

#[test]
fn find_semantic_candidate_and_create_new() {
    let d = tempdir().unwrap();
    fs::write(
        d.path().join("a.dart"),
        "class A { String authenticate(String token) { return token; } }",
    )
    .unwrap();
    let r = audit(d.path(), Language::Dart).unwrap();
    assert_eq!(
        find(
            &r.declarations,
            "login",
            Some("authenticate a user"),
            Vec::new()
        )
        .decision,
        "reuse_candidate"
    );
    assert_eq!(
        find(&r.declarations, "missing", Some("encrypt data"), Vec::new()).decision,
        "create_new"
    );
}

#[test]
fn generated_files_are_excluded() {
    let d = tempdir().unwrap();
    fs::write(
        d.path().join("generated.g.dart"),
        "class A { void greet() {} }",
    )
    .unwrap();
    let r = audit(d.path(), Language::Dart).unwrap();
    assert!(r.declarations.is_empty());
}

#[test]
fn documentation_is_used_for_reuse_search() {
    let d = tempdir().unwrap();
    fs::write(
            d.path().join("auth.dart"),
            "/// Authenticates a user with email and password.\nFuture<User> login(String email, String password) async { return client.login(); }\n",
        )
        .unwrap();
    let report = audit(d.path(), Language::Dart).unwrap();
    assert_eq!(
        report.declarations[0].documentation.as_deref(),
        Some("Authenticates a user with email and password.")
    );
    let found = find(
        &report.declarations,
        "signIn",
        Some("authenticate user with email and password"),
        vec![],
    );
    assert_eq!(found.decision, "reuse_candidate");
    assert_eq!(found.candidates[0].name, "login");
}

#[test]
fn extracts_async_generic_methods_and_ignores_locals() {
    let d = tempdir().unwrap();
    fs::write(
        d.path().join("a.dart"),
        r#"// void fake(int x) {}
class A {
  Future<void> login(String email, String password) async {
    void local(int x) {}
    print("void fake(int x) {}");
  }
}"#,
    )
    .unwrap();
    let r = audit(d.path(), Language::Dart).unwrap();
    assert_eq!(r.declarations.len(), 1);
    assert_eq!(r.declarations[0].name, "login");
    assert_eq!(r.declarations[0].signature, "login(idid,idid)");
}
