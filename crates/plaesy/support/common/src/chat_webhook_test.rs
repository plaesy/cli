//! Unit tests for `chat_webhook.rs`, included via `#[path]`.

use std::cell::RefCell;

use super::*;

/// Pemanggil HTTP palsu yang mencatat setiap permintaan.
struct FakePoster {
    status: Result<u16, &'static str>,
    calls: RefCell<Vec<(String, Vec<(String, String)>, Vec<u8>)>>,
}

impl FakePoster {
    fn ok(status: u16) -> Self {
        Self {
            status: Ok(status),
            calls: RefCell::new(Vec::new()),
        }
    }
}

impl HttpPoster for FakePoster {
    fn post_json(
        &self,
        url: &str,
        headers: &[(String, String)],
        body: &[u8],
    ) -> Result<u16, TransportError> {
        self.calls
            .borrow_mut()
            .push((url.to_string(), headers.to_vec(), body.to_vec()));
        self.status.map_err(|e| TransportError(e.to_string()))
    }
}

fn no_env(_: &str) -> Option<String> {
    None
}

fn target(name: &str, url: &str, enabled: bool, auth: Option<(&str, &str)>) -> ChatWebhookTarget {
    ChatWebhookTarget {
        name: name.to_string(),
        url: url.to_string(),
        enabled,
        auth: auth.map(|(header, env)| AuthHeader {
            header: header.to_string(),
            env: env.to_string(),
        }),
    }
}

fn message() -> ChatMessage {
    ChatMessage::new("task.done", "Tugas selesai", "T-12 lolos semua cek").expect("valid message")
}

#[test]
fn https_is_accepted_for_any_host() {
    assert!(validate_url("https://hooks.example.com/services/abc?x=1").is_ok());
    assert!(validate_url("https://example.com:8443/path").is_ok());
}

#[test]
fn http_is_accepted_only_for_loopback() {
    assert!(validate_url("http://localhost:8080/hook").is_ok());
    assert!(validate_url("http://127.0.0.1/hook").is_ok());
    assert!(validate_url("http://127.255.0.9:9000").is_ok());
    assert!(validate_url("http://[::1]:9000/x").is_ok());
    assert!(validate_url("http://example.com/hook").is_err());
    assert!(validate_url("http://192.168.1.10/hook").is_err());
    assert!(validate_url("http://127.0.0.1.evil.example/hook").is_err());
}

#[test]
fn unsafe_urls_are_rejected() {
    for url in [
        "",
        "ftp://example.com/x",
        "example.com/hook",
        "https://",
        "https:///path",
        "https://user:pass@example.com/hook",
        "https://example.com:abc/hook",
        "https://example.com:/hook",
        "https://a:b:c/hook",
        "https://[::1/hook",
        "https://exa mple.com/hook",
        "https://example.com/hook\nX-Injected: 1",
        "https://ex_ample.com/hook",
    ] {
        assert!(validate_url(url).is_err(), "seharusnya ditolak: {url:?}");
    }
    let long = format!("https://example.com/{}", "a".repeat(MAX_URL_BYTES));
    assert!(validate_url(&long).is_err());
}

#[test]
fn config_parses_targets_and_defaults_to_disabled() {
    let json = r#"{"targets":[
        {"name":"tim-a","url":"https://hooks.example.com/a",
         "auth":{"header":"Authorization","env":"PLAESY_CHAT_TOKEN_A"}},
        {"name":"lokal","url":"http://localhost:9000/x","enabled":true}
    ]}"#;
    let config = ChatWebhookConfig::from_json(json).expect("valid config");
    assert_eq!(config.targets.len(), 2);
    assert!(!config.targets[0].enabled);
    assert!(config.targets[1].enabled);
    assert_eq!(
        config.targets[0].auth,
        Some(AuthHeader {
            header: "Authorization".to_string(),
            env: "PLAESY_CHAT_TOKEN_A".to_string(),
        })
    );
}

#[test]
fn config_rejects_literal_secret_values_and_unknown_fields() {
    let json = r#"{"targets":[{"name":"a","url":"https://x.example/h",
        "auth":{"header":"Authorization","value":"Bearer rahasia"}}]}"#;
    let err = ChatWebhookConfig::from_json(json).expect_err("value field is unknown");
    assert!(matches!(err, ChatWebhookError::Config(_)));
    let json = r#"{"targets":[{"name":"a","url":"https://x.example/h","secret":"x"}]}"#;
    assert!(ChatWebhookConfig::from_json(json).is_err());
}

#[test]
fn config_rejects_duplicate_names_bad_urls_and_bad_auth_fields() {
    let dup = ChatWebhookConfig {
        targets: vec![
            target("a", "https://x.example/1", false, None),
            target("a", "https://x.example/2", false, None),
        ],
    };
    assert!(matches!(dup.validate(), Err(ChatWebhookError::Config(_))));

    let bad_url = ChatWebhookConfig {
        targets: vec![target("a", "http://x.example/1", true, None)],
    };
    assert!(matches!(
        bad_url.validate(),
        Err(ChatWebhookError::InvalidUrl { .. })
    ));

    for (header, env) in [
        ("Content-Type", "TOKEN"),
        ("Host", "TOKEN"),
        ("X Bad", "TOKEN"),
        ("", "TOKEN"),
        ("Authorization", "1BAD"),
        ("Authorization", "BAD-NAME"),
    ] {
        let config = ChatWebhookConfig {
            targets: vec![target("a", "https://x.example/1", true, Some((header, env)))],
        };
        assert!(config.validate().is_err(), "seharusnya ditolak: {header} {env}");
    }
}

#[test]
fn message_validation_rejects_bad_event_and_title() {
    assert!(ChatMessage::new("Task Done", "x", "").is_err());
    assert!(ChatMessage::new("", "x", "").is_err());
    assert!(ChatMessage::new("ok.event-1", "   ", "").is_err());
    assert!(ChatMessage::new("ok", "baris\nbaru", "").is_err());
    assert!(ChatMessage::new("ok", "judul", "").is_ok());
}

#[test]
fn payload_has_generic_json_shape() {
    let bytes = build_payload("tim-a", &message()).expect("payload");
    let value: serde_json::Value = serde_json::from_slice(&bytes).expect("json");
    assert_eq!(value["schema"], SCHEMA);
    assert_eq!(value["event"], "task.done");
    assert_eq!(value["target"], "tim-a");
    assert_eq!(value["text"], "Tugas selesai\n\nT-12 lolos semua cek");
}

#[test]
fn payload_without_body_uses_title_only() {
    let msg = ChatMessage::new("ev", "Hanya judul", "").expect("message");
    let value: serde_json::Value =
        serde_json::from_slice(&build_payload("t", &msg).expect("payload")).expect("json");
    assert_eq!(value["text"], "Hanya judul");
}

#[test]
fn oversized_body_is_truncated_within_payload_limit() {
    let msg = ChatMessage::new("ev", "judul", "a".repeat(200_000)).expect("message");
    let bytes = build_payload("t", &msg).expect("payload");
    assert!(bytes.len() <= MAX_PAYLOAD_BYTES);
    let value: serde_json::Value = serde_json::from_slice(&bytes).expect("json");
    assert!(value["text"].as_str().expect("text").ends_with(ELLIPSIS));
}

#[test]
fn control_characters_that_expand_when_escaped_still_fit() {
    let msg = ChatMessage::new("ev", "judul", "\u{1}".repeat(50_000)).expect("message");
    let bytes = build_payload("t", &msg).expect("payload");
    assert!(bytes.len() <= MAX_PAYLOAD_BYTES);
    serde_json::from_slice::<serde_json::Value>(&bytes).expect("json remains valid");
}

#[test]
fn truncation_never_splits_a_multibyte_character() {
    let msg = ChatMessage::new("ev", "judul", "é".repeat(10_000)).expect("message");
    let bytes = build_payload("t", &msg).expect("payload");
    // Potongan yang terpotong di tengah karakter akan membuat JSON tidak valid di sini.
    let value: serde_json::Value = serde_json::from_slice(&bytes).expect("json");
    assert!(value["text"].as_str().expect("text").ends_with(ELLIPSIS));
    assert!(bytes.len() <= MAX_PAYLOAD_BYTES);
}

#[test]
fn headers_read_env_and_never_echo_the_value() {
    let t = target("a", "https://x.example/1", true, Some(("Authorization", "TOKEN_A")));
    let env = |k: &str| (k == "TOKEN_A").then(|| "Bearer s3cret".to_string());
    let headers = resolve_headers(&t, &env).expect("headers");
    assert_eq!(headers[0].0, "Content-Type");
    assert_eq!(headers[1], ("Authorization".to_string(), "Bearer s3cret".to_string()));
}

#[test]
fn missing_or_invalid_env_value_is_a_typed_error_without_the_secret() {
    let t = target("a", "https://x.example/1", true, Some(("Authorization", "TOKEN_A")));
    assert_eq!(
        resolve_headers(&t, &no_env),
        Err(ChatWebhookError::MissingEnv {
            target: "a".to_string(),
            var: "TOKEN_A".to_string(),
        })
    );
    let empty = |_: &str| Some(String::new());
    assert!(resolve_headers(&t, &empty).is_err());
    let injected = |_: &str| Some("x\r\nHost: evil".to_string());
    let err = resolve_headers(&t, &injected).expect_err("control char");
    assert!(!err.to_string().contains("evil"));
}

#[test]
fn deliver_posts_json_with_auth_header_and_accepts_2xx() {
    let t = target("a", "https://x.example/hook", true, Some(("Authorization", "TOKEN_A")));
    let env = |k: &str| (k == "TOKEN_A").then(|| "Bearer s3cret".to_string());
    let poster = FakePoster::ok(204);
    deliver(&poster, &t, &message(), &env).expect("delivered");
    let calls = poster.calls.borrow();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].0, "https://x.example/hook");
    assert!(calls[0].1.contains(&("Authorization".to_string(), "Bearer s3cret".to_string())));
    let body: serde_json::Value = serde_json::from_slice(&calls[0].2).expect("json");
    assert_eq!(body["schema"], SCHEMA);
}

#[test]
fn deliver_reports_non_2xx_status_and_transport_failure() {
    let t = target("a", "https://x.example/hook", true, None);
    assert_eq!(
        deliver(&FakePoster::ok(500), &t, &message(), &no_env),
        Err(ChatWebhookError::Status {
            target: "a".to_string(),
            status: 500,
        })
    );
    let failing = FakePoster {
        status: Err("koneksi ditolak"),
        calls: RefCell::new(Vec::new()),
    };
    assert_eq!(
        deliver(&failing, &t, &message(), &no_env),
        Err(ChatWebhookError::Transport {
            target: "a".to_string(),
            reason: "koneksi ditolak".to_string(),
        })
    );
}

#[test]
fn deliver_rejects_unsafe_url_before_any_post() {
    let t = target("a", "http://203.0.113.5/hook", true, None);
    let poster = FakePoster::ok(200);
    assert!(deliver(&poster, &t, &message(), &no_env).is_err());
    assert!(poster.calls.borrow().is_empty());
}

#[test]
fn dispatch_sends_only_to_opted_in_targets() {
    let config = ChatWebhookConfig {
        targets: vec![
            target("on", "https://x.example/on", true, None),
            target("off", "https://x.example/off", false, None),
        ],
    };
    let poster = FakePoster::ok(200);
    let reports = dispatch(&config, &poster, &message(), &no_env);
    assert_eq!(
        reports,
        vec![DeliveryReport {
            target: "on".to_string(),
            result: Ok(()),
        }]
    );
    let calls = poster.calls.borrow();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].0, "https://x.example/on");
}
