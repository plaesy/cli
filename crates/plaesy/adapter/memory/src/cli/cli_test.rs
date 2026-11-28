//! Unit tests for `remember_cli_tests` split out of `cli.rs`.
//! Kept as a separate file so they are easy to find; included via `#[path]`.

use super::*;

#[test]
fn serve_stdio_never_reads_implicit_http_credentials() {
    for (mcp, transport) in [
        (false, serve::Transport::Stdio),
        (true, serve::Transport::Stdio),
        (false, serve::Transport::Http),
    ] {
        let token = resolve_serve_auth_token(mcp, transport, None, "PLAESY_MCP_TOKEN", |_| {
            panic!("non-HTTP-MCP services must not read the environment or secrets file")
        })
        .expect("implicit HTTP credentials do not affect this service");
        assert!(token.is_none());
    }
}

#[test]
fn serve_http_uses_configured_credential_lookup_and_propagates_errors() {
    let token =
        resolve_serve_auth_token(true, serve::Transport::Http, None, "CUSTOM_TOKEN", |key| {
            assert_eq!(key, "CUSTOM_TOKEN");
            Ok(Some("test-http-token".into()))
        })
        .expect("HTTP reads the configured credential source");
    assert_eq!(token.as_deref(), Some("test-http-token"));

    let failed =
        resolve_serve_auth_token(true, serve::Transport::Http, None, "CUSTOM_TOKEN", |_| {
            Err(MemoryError::ConfigError(
                "unreadable credential source".into(),
            ))
        });
    assert!(
        failed.is_err(),
        "HTTP credential lookup must remain fail-closed"
    );
}

#[test]
fn serve_explicit_token_reaches_transport_validation_without_lookup() {
    for mcp in [false, true] {
        for transport in [serve::Transport::Stdio, serve::Transport::Http] {
            let token = resolve_serve_auth_token(
                mcp,
                transport,
                Some("explicit-test-token".into()),
                "CUSTOM_TOKEN",
                |_| panic!("explicit credentials override implicit credentials"),
            )
            .expect("explicit token is preserved for serve validation");
            assert_eq!(token.as_deref(), Some("explicit-test-token"));
        }
    }
}

#[test]
fn serve_http_empty_auth_token_env_disables_lookup() {
    let token = resolve_serve_auth_token(true, serve::Transport::Http, None, "", |_| {
        panic!("an empty auth_token_env disables credential lookup")
    })
    .expect("disabled implicit credentials");
    assert!(token.is_none());
}

#[tokio::test]
async fn serve_explicit_token_still_requires_mcp_http() {
    for (mcp, transport) in [
        (false, serve::Transport::Stdio),
        (true, serve::Transport::Stdio),
        (false, serve::Transport::Http),
    ] {
        let error = serve::run(
            MemoryConfig::default(),
            mcp,
            transport,
            9123,
            Some("explicit-test-token".into()),
            true,
        )
        .await
        .expect_err("explicit HTTP credentials must be refused for other services");
        assert!(error
            .to_string()
            .contains("--auth-token requires --mcp --transport http"));
    }
}

fn report(labels: &[&str]) -> HygieneReport {
    HygieneReport {
        checked: 0,
        findings: labels
            .iter()
            .map(|label| crate::memory::hygiene::Finding {
                kind: "sample".into(),
                path: "note.md".into(),
                line: None,
                detail: "sample finding".into(),
                label: (*label).into(),
            })
            .collect(),
        summary: Default::default(),
    }
}

#[test]
fn hygiene_strict_facts_ignores_inferred_findings_but_strict_does_not() {
    let inferred = report(&["INFERRED"]);
    assert!(!hygiene_strict_failure(&inferred, false, true));
    assert!(hygiene_strict_failure(&inferred, true, false));

    let confirmed = report(&["FACT"]);
    assert!(hygiene_strict_failure(&confirmed, false, true));
    assert!(hygiene_strict_failure(&confirmed, true, false));
}

#[test]
fn hygiene_strict_modes_cannot_be_combined() {
    assert!(Cli::try_parse_from([
        "plaesy memory",
        "hygiene",
        "vault",
        "--strict",
        "--strict-facts",
    ])
    .is_err());
}

#[test]
fn template_remember_options_parse() {
    let cli = Cli::try_parse_from([
        "plaesy memory",
        "remember",
        "A title",
        "--template",
        "knowledge",
        "--folder",
        "knowledges/testing",
        "--set",
        "ONE_SENTENCE_SUMMARY=A core claim",
    ])
    .expect("template remember arguments should parse");
    let Command::Remember {
        title,
        template,
        values,
        body,
        ..
    } = cli.command
    else {
        panic!("expected remember subcommand");
    };
    assert_eq!(title, "A title");
    assert_eq!(template.as_deref(), Some("knowledge"));
    assert_eq!(values, ["ONE_SENTENCE_SUMMARY=A core claim"]);
    assert!(body.is_none());
}

#[test]
fn forget_requires_confirmation_and_a_structured_reason() {
    assert!(Cli::try_parse_from([
        "plaesy memory",
        "forget",
        "notes/a.md",
        "--reason",
        "owner-request",
    ])
    .is_err());

    let with_confirmation = Cli::try_parse_from([
        "plaesy memory",
        "forget",
        "notes/a.md",
        "--reason",
        "privacy",
        "--yes",
    ])
    .expect("structured forget command parses");
    assert!(matches!(
        with_confirmation.command,
        Command::Forget { yes: true, .. }
    ));

    assert!(Cli::try_parse_from([
        "plaesy memory",
        "forget",
        "notes/a.md",
        "--reason",
        "free-form secret reason",
        "--yes",
    ])
    .is_err());
}

#[test]
fn update_requires_expected_hash_reason_and_confirmation() {
    let parsed = Cli::try_parse_from([
        "plaesy memory",
        "update",
        "knowledges/a.md",
        "--if-match",
        &"a".repeat(64),
        "--reason",
        "new-evidence",
        "--yes",
    ])
    .expect("well-formed update parses");
    assert!(matches!(parsed.command, Command::Update { yes: true, .. }));

    assert!(Cli::try_parse_from([
        "plaesy memory",
        "update",
        "knowledges/a.md",
        "--if-match",
        &"a".repeat(64),
        "--reason",
        "free-form",
        "--yes",
    ])
    .is_err());
    assert!(Cli::try_parse_from([
        "plaesy memory",
        "update",
        "knowledges/a.md",
        "--if-match",
        &"a".repeat(64),
        "--reason",
        "new-evidence",
    ])
    .is_err());
}

#[test]
fn fingerprint_path_parses() {
    assert!(matches!(
        Cli::try_parse_from(["plaesy memory", "fingerprint", "knowledges/a.md"])
            .expect("fingerprint parses")
            .command,
        Command::Fingerprint { .. }
    ));
}

#[test]
fn archive_requires_expected_hash_reason_and_confirmation() {
    let parsed = Cli::try_parse_from([
        "plaesy memory",
        "archive",
        "knowledges/a.md",
        "--if-match",
        &"b".repeat(64),
        "--reason",
        "completed",
        "--yes",
    ])
    .expect("well-formed archive parses");
    assert!(matches!(parsed.command, Command::Archive { yes: true, .. }));

    assert!(Cli::try_parse_from([
        "plaesy memory",
        "archive",
        "knowledges/a.md",
        "--if-match",
        &"b".repeat(64),
        "--reason",
        "free-form",
        "--yes",
    ])
    .is_err());
    assert!(Cli::try_parse_from([
        "plaesy memory",
        "archive",
        "knowledges/a.md",
        "--if-match",
        &"b".repeat(64),
        "--reason",
        "completed",
    ])
    .is_err());
}
