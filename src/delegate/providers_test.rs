//! Unit tests for `tests` split out of `providers.rs`.
//! Kept as a separate file so they are easy to find; included via `#[path]`.

use super::*;

#[test]
fn status_classes_match_the_chain_rules() {
    assert_eq!(
        http_error("p", 429, Some("7"), "").class,
        Class::RateLimited
    );
    assert_eq!(
        http_error("p", 429, Some("7"), "").retry_after,
        Some(Duration::from_secs(7))
    );
    assert_eq!(http_error("p", 503, None, "").class, Class::Transient);
    assert_eq!(http_error("p", 401, None, "").class, Class::Terminal);
}

#[test]
fn retry_after_accepts_seconds_and_rejects_nonsense() {
    assert_eq!(parse_retry_after("12"), Some(Duration::from_secs(12)));
    assert_eq!(parse_retry_after("0"), None);
    assert_eq!(parse_retry_after("soon"), None);
}

#[test]
fn error_bodies_are_capped() {
    let long = "x".repeat(500);
    assert!(http_error("p", 400, None, &long).message.len() < 400);
}

#[test]
fn missing_model_is_a_configuration_error() {
    let entry = DelegateProviderConfig {
        name: "a".into(),
        kind: "anthropic".into(),
        ..Default::default()
    };
    let err = build(&entry, std::sync::Arc::new(|_: &str| None))
        .err()
        .expect("refused");
    assert_eq!(err.class, Class::Terminal);
}

#[test]
fn cli_provider_runs_a_local_command_with_the_prompt_on_stdin() {
    let entry = DelegateProviderConfig {
        name: "echo".into(),
        kind: "cli".into(),
        command: if cfg!(windows) {
            "cmd".into()
        } else {
            "cat".into()
        },
        args: if cfg!(windows) {
            vec!["/C".into(), "more".into()]
        } else {
            vec![]
        },
        ..Default::default()
    };
    let provider = build(&entry, std::sync::Arc::new(|_: &str| None)).unwrap();
    let reply = provider
        .complete(&Request {
            prompt: "hello there".into(),
            ..Request::default()
        })
        .unwrap();
    assert_eq!(reply.text, "hello there");
}
