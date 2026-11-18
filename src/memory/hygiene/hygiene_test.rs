//! Unit tests for `tests` split out of `mod.rs`.
//! Kept as a separate file so they are easy to find; included via `#[path]`.

use super::*;

#[test]
fn scoped_markdown_link_validation_accepts_targets_elsewhere_in_vault() {
    let vault = tempfile::tempdir().unwrap();
    let scope = vault.path().join("python");
    std::fs::create_dir_all(&scope).unwrap();
    std::fs::write(scope.join("source.md"), "[target](../shared/target.md)\n").unwrap();
    std::fs::create_dir_all(vault.path().join("shared")).unwrap();
    std::fs::write(vault.path().join("shared/target.md"), "# Target\n").unwrap();

    let findings = validate_markdown_links_in_scope(vault.path(), &scope).unwrap();

    assert!(!findings.iter().any(|finding| {
        finding.kind == "broken_markdown_link" || finding.kind == "broken_link_anchor"
    }));
}

#[cfg(unix)]
#[test]
fn scoped_markdown_link_validation_rejects_symlink_escape() {
    use std::os::unix::fs::symlink;

    let vault = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    let scope = vault.path().join("python");
    std::fs::create_dir_all(&scope).unwrap();
    std::fs::write(outside.path().join("target.md"), "# Target\n").unwrap();
    symlink(outside.path(), vault.path().join("external")).unwrap();
    std::fs::write(scope.join("source.md"), "[target](../external/target.md)\n").unwrap();

    let findings = validate_markdown_links_in_scope(vault.path(), &scope).unwrap();

    assert!(
        findings
            .iter()
            .any(|finding| finding.kind == "broken_markdown_link"),
        "expected symlink escape finding; actual findings: {findings:?}"
    );
}

#[test]
fn scoped_markdown_link_validation_rejects_scope_outside_vault() {
    let vault = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();

    let error = validate_markdown_links_in_scope(vault.path(), outside.path()).unwrap_err();

    assert!(error.to_string().contains("outside vault root"));
}

#[test]
fn path_references_filters_noise() {
    let body = "See `src/lib.rs` and `Cargo.toml` and `a b/c.md`.\n\
                    ```\n`inside/fence.rs`\n```\n\
                    Also `https://x.dev/a.md`, `*.rs`, `docs/guide.md`, `dir/`, `tests/[module]/x.py`.\n";
    let refs: Vec<String> = path_references(body).into_iter().map(|(t, _)| t).collect();
    assert_eq!(refs, vec!["src/lib.rs", "docs/guide.md"]);
    let lines: Vec<usize> = path_references(body).into_iter().map(|(_, l)| l).collect();
    assert_eq!(lines, vec![1, 5]);
}

#[test]
fn secret_references_report_kind_and_line_without_value() {
    let body = "safe\nOPENAI_API_KEY=sk-test-value\npassword = \"do-not-return\"\n-----BEGIN PRIVATE KEY-----\n";
    let findings = secret_references(body);
    assert_eq!(findings.len(), 3);
    assert_eq!(findings[0], ("api_key".to_string(), 2));
    assert_eq!(findings[1], ("password".to_string(), 3));
    assert_eq!(findings[2], ("private_key".to_string(), 4));
    assert!(!format!("{findings:?}").contains("do-not-return"));
}

#[test]
fn secret_references_detect_provider_formats_without_returning_values() {
    let body = [
        "AKIAIOSFODNN7EXAMPLE",
        "ghp_012345678901234567890123456789012345",
        "xoxb-123456789012345678901234",
        "sk_live_12345678901234567890",
        "AIza01234567890123456789012345678901234",
        "glpat-01234567890123456789",
        "sk-ant-api03-0123456789abcdefghij",
        "sk-proj-012345678901234567890123456789012345",
    ]
    .join("\n");
    let findings = secret_references(&body);
    assert_eq!(
        findings
            .iter()
            .map(|(kind, _)| kind.as_str())
            .collect::<Vec<_>>(),
        [
            "aws_access_key_id",
            "github_token",
            "slack_token",
            "stripe_secret_key",
            "google_api_key",
            "gitlab_token",
            "anthropic_api_key",
            "openai_api_key",
        ]
    );
    assert!(!format!("{findings:?}").contains("0123456789"));
}

#[test]
fn secret_scanner_synthetic_corpus_precision_and_recall() {
    // These are deliberately fake strings. The corpus measures our current rules; it is not
    // evidence of provider-side validity or an exhaustive secret scan.
    let positives = [
        ("OPENAI_API_KEY=sk-example-secret-value", true),
        ("password: correct-horse-battery", true),
        ("Authorization: Bearer abcdEFGH0123456789xyz", true),
        ("AKIA1234567890ABCDEF", true),
        ("ghp_012345678901234567890123456789012345", true),
        ("xoxb-123456789012345678901234", true),
        ("sk_live_12345678901234567890", true),
        ("sk_test_0123456789012345", true),
        ("AIza01234567890123456789012345678901234", true),
        ("glpat-01234567890123456789", true),
        ("sk-ant-api03-0123456789abcdefghij", true),
        ("sk-proj-012345678901234567890123456789012345", true),
        ("-----BEGIN PRIVATE KEY-----", true),
    ];
    let negatives = [
        "API_KEY=<your-key>",
        "password: changeme",
        "The word token appears in this documentation.",
        "AIza0123456789012345678901234567890123",
        "```text\nOPENAI_API_KEY=sk-example-secret-value\n```",
        "client_secret is the field name, not a value",
        "Bearer <access-token>",
        "glpat-12345",
        "sk-ant-api03-placeholder",
    ];

    let tp = positives
        .iter()
        .filter(|(line, expected)| *expected && !secret_references(line).is_empty())
        .count();
    let fn_ = positives.len() - tp;
    let fp = negatives
        .iter()
        .filter(|line| !secret_references(line).is_empty())
        .count();
    let tn = negatives.len() - fp;
    let recall = tp as f64 / (tp + fn_) as f64;
    let precision = tp as f64 / (tp + fp) as f64;

    eprintln!(
            "synthetic secret scan: TP={tp} FN={fn_} FP={fp} TN={tn}; precision={precision:.3}, recall={recall:.3}"
        );
    assert_eq!((tp, fn_, fp, tn), (13, 0, 0, 9));
    for (line, _) in positives {
        let findings = secret_references(line);
        assert_eq!(findings.len(), 1);
        assert!(!format!("{findings:?}").contains(line));
    }
}
