//! Unit tests for `tests` split out of `providers.rs`.
//! Kept as a separate file so they are easy to find; included via `#[path]`.

use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};

/// A provider that returns a fixed result and counts its calls.
struct Stub {
    result: Result<ProviderReply, DelegateError>,
    calls: Arc<AtomicUsize>,
}

impl Provider for Stub {
    fn name(&self) -> &str {
        "stub"
    }
    fn complete(&self, _req: &Request) -> Result<ProviderReply, DelegateError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.result.clone()
    }
}

fn stub(
    result: Result<ProviderReply, DelegateError>,
    calls: &Arc<AtomicUsize>,
) -> Box<dyn Provider> {
    Box::new(Stub {
        result,
        calls: Arc::clone(calls),
    })
}

fn answer(text: &str) -> ProviderReply {
    ProviderReply {
        text: text.into(),
        input_tokens: None,
        output_tokens: None,
    }
}

fn model_fallback(models: Vec<Box<dyn Provider>>) -> ModelFallback {
    ModelFallback {
        name: "p".into(),
        models,
    }
}

#[test]
fn model_fallback_moves_past_a_retryable_error() {
    let calls = Arc::new(AtomicUsize::new(0));
    let chain = model_fallback(vec![
        stub(Err(DelegateError::transient("HTTP 503")), &calls),
        stub(Ok(answer("second")), &calls),
    ]);
    let reply = chain.complete(&Request::default()).unwrap();
    assert_eq!(reply.text, "second");
    assert_eq!(calls.load(Ordering::SeqCst), 2);
}

#[test]
fn model_fallback_moves_past_a_rate_limit() {
    let calls = Arc::new(AtomicUsize::new(0));
    let limited = DelegateError {
        class: Class::RateLimited,
        retry_after: None,
        message: "HTTP 429".into(),
    };
    let chain = model_fallback(vec![
        stub(Err(limited), &calls),
        stub(Ok(answer("second")), &calls),
    ]);
    assert_eq!(chain.complete(&Request::default()).unwrap().text, "second");
}

#[test]
fn model_fallback_stops_on_a_terminal_error() {
    let calls = Arc::new(AtomicUsize::new(0));
    let chain = model_fallback(vec![
        stub(Err(DelegateError::terminal("HTTP 401")), &calls),
        stub(Ok(answer("unreached")), &calls),
    ]);
    let err = chain.complete(&Request::default()).unwrap_err();
    assert_eq!(err.class, Class::Terminal);
    assert_eq!(
        calls.load(Ordering::SeqCst),
        1,
        "the second model must not be called"
    );
}

#[test]
fn model_fallback_reports_every_retryable_failure_when_none_answers() {
    let calls = Arc::new(AtomicUsize::new(0));
    let limited = DelegateError {
        class: Class::RateLimited,
        retry_after: None,
        message: "HTTP 429".into(),
    };
    let chain = model_fallback(vec![
        stub(Err(limited), &calls),
        stub(Err(DelegateError::transient("HTTP 503")), &calls),
    ]);
    let err = chain.complete(&Request::default()).unwrap_err();
    assert_eq!(err.class, Class::Transient);
    assert!(err.message.contains("HTTP 429"), "{}", err.message);
    assert!(err.message.contains("HTTP 503"), "{}", err.message);
    assert_eq!(calls.load(Ordering::SeqCst), 2);
}

#[test]
fn fallback_models_build_one_provider_named_after_the_entry() {
    let entry = DelegateProviderConfig {
        name: "local".into(),
        kind: "ollama".into(),
        model: "a".into(),
        fallback_models: vec!["b".into()],
        endpoint: "http://127.0.0.1:9".into(),
        ..Default::default()
    };
    let provider = build(&entry, std::sync::Arc::new(|_: &str| None)).unwrap();
    assert_eq!(provider.name(), "local");
}

#[test]
fn fallback_models_are_refused_for_a_cli_entry() {
    let entry = DelegateProviderConfig {
        name: "c".into(),
        kind: "cli".into(),
        command: "x".into(),
        fallback_models: vec!["b".into()],
        ..Default::default()
    };
    let err = build(&entry, std::sync::Arc::new(|_: &str| None))
        .err()
        .expect("refused");
    assert_eq!(err.class, Class::Terminal);
}

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
