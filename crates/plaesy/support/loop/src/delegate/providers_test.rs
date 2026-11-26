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

#[test]
fn anthropic_body_moves_system_messages_to_the_system_field() {
    let messages = vec![Message::system("Anda reviewer."), Message::user("halo")];
    let body = anthropic_body("claude-x", 64, &messages);
    assert_eq!(body["system"], "Anda reviewer.");
    assert_eq!(
        body["messages"],
        json!([{ "role": "user", "content": "halo" }])
    );
}

#[test]
fn anthropic_body_without_system_has_no_system_field() {
    let body = anthropic_body("claude-x", 64, &[Message::user("halo")]);
    assert!(body.get("system").is_none());
}

fn empty_stream_reply() -> ProviderReply {
    ProviderReply {
        text: String::new(),
        input_tokens: None,
        output_tokens: None,
    }
}

#[test]
fn anthropic_stream_passes_text_deltas_and_collects_tokens() {
    let mut reply = empty_stream_reply();
    let mut seen = String::new();
    let events = [
        r#"{"type":"message_start","message":{"usage":{"input_tokens":12}}}"#,
        r#"{"type":"content_block_delta","delta":{"type":"text_delta","text":"Halo"}}"#,
        r#"{"type":"content_block_delta","delta":{"type":"text_delta","text":", dunia"}}"#,
        r#"{"type":"message_delta","usage":{"output_tokens":4}}"#,
        r#"{"type":"message_stop"}"#,
    ];
    for data in events {
        anthropic_stream_event("p", data, &mut reply, &mut |t| seen.push_str(t)).expect("event");
    }
    assert_eq!(seen, "Halo, dunia");
    assert_eq!(reply.text, "Halo, dunia");
    assert_eq!(
        (reply.input_tokens, reply.output_tokens),
        (Some(12), Some(4))
    );
}

#[test]
fn anthropic_stream_error_event_is_classified() {
    let mut reply = empty_stream_reply();
    let overloaded = r#"{"type":"error","error":{"type":"overloaded_error","message":"sibuk"}}"#;
    let err = anthropic_stream_event("p", overloaded, &mut reply, &mut |_| {}).expect_err("error");
    assert_eq!(err.class, Class::Transient);

    let invalid = r#"{"type":"error","error":{"type":"invalid_request_error","message":"salah"}}"#;
    let err = anthropic_stream_event("p", invalid, &mut reply, &mut |_| {}).expect_err("error");
    assert_eq!(err.class, Class::Terminal);
}

#[test]
fn ollama_stream_reads_one_object_per_line_and_the_final_counts() {
    let mut reply = empty_stream_reply();
    let mut seen = String::new();
    let lines = [
        r#"{"message":{"content":"Ha"},"done":false}"#,
        "",
        r#"{"message":{"content":"lo"},"done":false}"#,
        r#"{"message":{"content":""},"done":true,"prompt_eval_count":7,"eval_count":2}"#,
    ];
    for line in lines {
        ollama_stream_line("p", line, &mut reply, &mut |t| seen.push_str(t)).expect("line");
    }
    assert_eq!(seen, "Halo");
    assert_eq!(
        (reply.input_tokens, reply.output_tokens),
        (Some(7), Some(2))
    );
}

#[test]
fn openai_stream_skips_done_and_reads_content_deltas() {
    let mut reply = empty_stream_reply();
    let mut seen = String::new();
    let lines = [
        "",
        r#"data: {"choices":[{"delta":{"role":"assistant"}}]}"#,
        r#"data: {"choices":[{"delta":{"content":"Ha"}}]}"#,
        r#"data: {"choices":[{"delta":{"content":"lo"}}]}"#,
        "data: [DONE]",
    ];
    for line in lines {
        openai_stream_line("p", line, &mut reply, &mut |t| seen.push_str(t)).expect("line");
    }
    assert_eq!(seen, "Halo");
    assert_eq!(reply.text, "Halo");
}

#[test]
fn post_lines_hands_over_each_line_of_a_2xx_reply() {
    use std::io::{Read as _, Write as _};
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
    let addr = listener.local_addr().expect("address");
    let server = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept");
        // Read the whole request first: closing with unread bytes resets the connection.
        let mut request = Vec::new();
        let mut chunk = [0u8; 1024];
        let body_start = loop {
            if let Some(at) = request.windows(4).position(|w| w == b"\r\n\r\n") {
                break at + 4;
            }
            let n = stream.read(&mut chunk).expect("read headers");
            assert!(n > 0, "client closed before the headers ended");
            request.extend_from_slice(&chunk[..n]);
        };
        let head = String::from_utf8_lossy(&request[..body_start]).to_ascii_lowercase();
        let length: usize = head
            .lines()
            .find_map(|line| line.strip_prefix("content-length:"))
            .and_then(|value| value.trim().parse().ok())
            .unwrap_or(0);
        while request.len() < body_start + length {
            let n = stream.read(&mut chunk).expect("read body");
            assert!(n > 0, "client closed before the body ended");
            request.extend_from_slice(&chunk[..n]);
        }
        let body = "{\"a\":1}\n{\"a\":2}\n";
        let response = format!(
            "HTTP/1.1 200 OK\r\ncontent-type: application/x-ndjson\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
            body.len()
        );
        stream.write_all(response.as_bytes()).expect("write");
    });
    let mut lines = Vec::new();
    post_lines(
        "p",
        &format!("http://{addr}/api/chat"),
        &[("content-type", "application/json".to_owned())],
        &json!({ "x": 1 }),
        Duration::from_secs(5),
        &mut |line| {
            lines.push(line.to_owned());
            Ok(())
        },
    )
    .expect("post");
    server.join().expect("server thread");
    assert_eq!(lines, vec!["{\"a\":1}", "{\"a\":2}"]);
}

#[test]
fn claude_stream_passes_each_text_delta_and_counts_tokens_from_the_result() {
    let lines = [
        r#"{"type":"system","subtype":"init"}"#,
        r#"{"type":"stream_event","event":{"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"Jakar"}}}"#,
        r#"{"type":"stream_event","event":{"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"ta."}}}"#,
        r#"{"type":"result","subtype":"success","result":"Jakarta.","usage":{"input_tokens":2,"cache_creation_input_tokens":10,"cache_read_input_tokens":5,"output_tokens":8}}"#,
        "not json",
    ];
    let mut reply = empty_reply();
    let mut seen = Vec::new();
    for line in lines {
        claude_stream_line(line, &mut reply, &mut |text: &str| {
            seen.push(text.to_owned())
        });
    }
    assert_eq!(
        seen,
        vec!["Jakar", "ta."],
        "the result does not repeat streamed text"
    );
    assert_eq!(reply.text, "Jakarta.");
    assert_eq!(reply.input_tokens, Some(17));
    assert_eq!(reply.output_tokens, Some(8));
}

#[test]
fn claude_stream_uses_the_result_when_no_delta_came() {
    let mut reply = empty_reply();
    let mut seen = Vec::new();
    claude_stream_line(
        r#"{"type":"result","result":"halo"}"#,
        &mut reply,
        &mut |text: &str| seen.push(text.to_owned()),
    );
    assert_eq!(reply.text, "halo");
    assert_eq!(seen, vec!["halo"]);
}

#[test]
fn a_cli_provider_streams_only_when_asked_for_stream_json() {
    let plain = CliProvider {
        name: "p".into(),
        command: "x".into(),
        args: vec!["-p".into()],
    };
    let streaming = CliProvider {
        name: "p".into(),
        command: "x".into(),
        args: vec!["-p".into(), "--output-format".into(), "stream-json".into()],
    };
    assert!(!plain.streams_json());
    assert!(streaming.streams_json());
}

/// A provider that streams its own reply, as the CLI provider does, and fails when `fail` is set.
struct StreamingStub {
    fail: bool,
    text: &'static str,
}

impl Provider for StreamingStub {
    fn name(&self) -> &str {
        "streaming"
    }
    fn complete(&self, _req: &Request) -> Result<ProviderReply, DelegateError> {
        Err(DelegateError::terminal("complete is not used by the chat stream"))
    }
    fn complete_messages_streaming(
        &self,
        _messages: &[Message],
        _req: &Request,
        on_text: &mut dyn FnMut(&str),
    ) -> Result<ProviderReply, DelegateError> {
        if self.fail {
            return Err(DelegateError::transient("HTTP 503"));
        }
        on_text(self.text);
        Ok(answer(self.text))
    }
}

#[test]
fn model_fallback_streams_through_the_models_it_wraps() {
    let chain = model_fallback(vec![
        Box::new(StreamingStub { fail: true, text: "" }),
        Box::new(StreamingStub { fail: false, text: "streamed" }),
    ]);
    let mut seen = String::new();
    let reply = chain
        .complete_messages_streaming(&[Message::user("halo")], &Request::default(), &mut |chunk| {
            seen.push_str(chunk)
        })
        .unwrap();
    assert_eq!(reply.text, "streamed");
    assert_eq!(seen, "streamed");
}

#[test]
fn claude_stream_lines_give_only_the_answer_text() {
    let lines = [
        r#"{"type":"system","subtype":"hook_started","hook_name":"SessionStart:startup"}"#,
        r#"{"type":"stream_event","event":{"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"Halo"}}}"#,
        r#"{"type":"stream_event","event":{"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"!"}}}"#,
        r#"{"type":"assistant","message":{"content":[{"type":"text","text":"Halo!"}]}}"#,
        r#"{"type":"result","result":"Halo!","usage":{"input_tokens":2,"output_tokens":3}}"#,
    ];
    let mut reply = empty_reply();
    let mut streamed = String::new();
    for line in lines {
        claude_stream_line(line, &mut reply, &mut |chunk: &str| streamed.push_str(chunk));
    }
    assert_eq!(reply.text, "Halo!");
    assert_eq!(streamed, "Halo!");
    assert_eq!(reply.output_tokens, Some(3));
}
