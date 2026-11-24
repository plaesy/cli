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

#[test]
fn anthropic_body_sends_the_whole_conversation() {
    let messages = vec![
        Message::user("satu"),
        Message::assistant("dua"),
        Message::user("tiga"),
    ];
    let body = anthropic_body("claude-x", 256, &messages);
    assert_eq!(body["model"], "claude-x");
    assert_eq!(body["max_tokens"], 256);
    assert_eq!(
        body["messages"],
        json!([
            { "role": "user", "content": "satu" },
            { "role": "assistant", "content": "dua" },
            { "role": "user", "content": "tiga" },
        ])
    );
}

#[test]
fn anthropic_reply_joins_text_blocks_and_reads_usage() {
    let reply = json!({
        "content": [
            { "type": "text", "text": "halo " },
            { "type": "tool_use", "name": "x" },
            { "type": "text", "text": "dunia" },
        ],
        "usage": { "input_tokens": 7, "output_tokens": 3 },
    });
    let got = anthropic_reply(&reply);
    assert_eq!(got.text, "halo dunia");
    assert_eq!(got.input_tokens, Some(7));
    assert_eq!(got.output_tokens, Some(3));
}

#[test]
fn ollama_body_uses_chat_messages_without_streaming() {
    let messages = vec![
        Message::user("a"),
        Message::assistant("b"),
        Message::user("c"),
    ];
    let body = ollama_body("llama", 64, &messages);
    assert_eq!(body["stream"], false);
    assert_eq!(body["options"]["num_predict"], 64);
    assert_eq!(
        body["messages"][1],
        json!({ "role": "assistant", "content": "b" })
    );
    assert!(
        body.get("prompt").is_none(),
        "chat API takes messages, not prompt"
    );
}

#[test]
fn ollama_reply_reads_message_content_and_counts() {
    let reply = json!({
        "message": { "role": "assistant", "content": "  jawab  " },
        "prompt_eval_count": 11,
        "eval_count": 4,
    });
    let got = ollama_reply(&reply);
    assert_eq!(got.text, "jawab");
    assert_eq!(got.input_tokens, Some(11));
    assert_eq!(got.output_tokens, Some(4));
}

#[test]
fn openai_body_and_reply_round_trip_a_conversation() {
    let messages = vec![
        Message::user("q"),
        Message::assistant("a"),
        Message::user("r"),
    ];
    let body = openai_body("gpt-x", 0, &messages);
    assert_eq!(
        body["messages"][2],
        json!({ "role": "user", "content": "r" })
    );
    assert_eq!(body["max_tokens"], 0);

    let reply = json!({
        "choices": [{ "message": { "role": "assistant", "content": " ok " } }],
        "usage": { "prompt_tokens": 5, "completion_tokens": 2 },
    });
    let got = openai_reply("p", &reply).expect("reply has a choice");
    assert_eq!(got.text, "ok");
    assert_eq!(got.input_tokens, Some(5));
    assert_eq!(got.output_tokens, Some(2));
}

#[test]
fn openai_reply_without_choices_is_terminal() {
    let err = openai_reply("p", &json!({ "choices": [] })).expect_err("no choice");
    assert_eq!(err.class, Class::Terminal);
}
