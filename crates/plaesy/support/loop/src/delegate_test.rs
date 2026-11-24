//! Unit tests for `tests` split out of `delegate.rs`.
//! Kept as a separate file so they are easy to find; included via `#[path]`.

use super::*;

fn cfg() -> DelegateConfig {
    DelegateConfig {
        providers: vec![
            DelegateProviderConfig {
                name: "off".into(),
                kind: "cli".into(),
                enabled: false,
                command: "x".into(),
                ..Default::default()
            },
            DelegateProviderConfig {
                name: "paid".into(),
                kind: "cli".into(),
                enabled: true,
                billable: true,
                command: "x".into(),
                ..Default::default()
            },
        ],
        ..Default::default()
    }
}

#[test]
fn disabled_and_unconfirmed_billable_providers_are_refused() {
    let c = cfg();
    assert!(select(&c, "off", true).unwrap_err().contains("disabled"));
    assert!(select(&c, "paid", false).unwrap_err().contains("--confirm"));
    assert!(select(&c, "paid", true).is_ok());
    assert!(select(&c, "nope", true)
        .unwrap_err()
        .contains("unknown provider"));
}

struct Scripted(Result<ProviderReply, DelegateError>);

impl Provider for Scripted {
    fn name(&self) -> &str {
        "scripted"
    }
    fn complete(&self, _req: &Request) -> Result<ProviderReply, DelegateError> {
        self.0.clone()
    }
}

fn chain_cfg() -> DelegateConfig {
    DelegateConfig {
        providers: ["first", "second"]
            .into_iter()
            .map(|name| DelegateProviderConfig {
                name: name.into(),
                kind: "cli".into(),
                enabled: true,
                command: "x".into(),
                ..Default::default()
            })
            .collect(),
        ..Default::default()
    }
}

#[test]
fn transient_failure_falls_through_to_the_next_provider() {
    let build = |entry: &DelegateProviderConfig| -> Result<Box<dyn Provider>, DelegateError> {
        let reply = if entry.name == "first" {
            Err(DelegateError::transient("timeout"))
        } else {
            Ok(ProviderReply {
                text: "answer".into(),
                input_tokens: None,
                output_tokens: None,
            })
        };
        Ok(Box::new(Scripted(reply)))
    };
    let sleep = |_: Duration| Ok(());
    let opts = ChainOptions {
        chain: "c".into(),
        confirm: true,
        max_retry_after: DEFAULT_MAX_RETRY_AFTER,
        build: &build,
        check: None,
        sleep: &sleep,
    };
    let names = vec!["first".to_owned(), "second".to_owned()];
    let (resp, attempts) = run_chain(&chain_cfg(), &names, &Request::default(), &opts).unwrap();
    assert_eq!(resp.text, "answer");
    assert_eq!(attempts[0].outcome, "failed");
    assert_eq!(attempts[1].outcome, "answered");
}

#[test]
fn terminal_failure_stops_the_chain_and_names_every_attempt() {
    let build = |_: &DelegateProviderConfig| -> Result<Box<dyn Provider>, DelegateError> {
        Ok(Box::new(Scripted(Err(DelegateError::terminal("bad key")))))
    };
    let sleep = |_: Duration| Ok(());
    let opts = ChainOptions {
        chain: "c".into(),
        confirm: true,
        max_retry_after: DEFAULT_MAX_RETRY_AFTER,
        build: &build,
        check: None,
        sleep: &sleep,
    };
    let names = vec!["first".to_owned(), "second".to_owned()];
    let err = run_chain(&chain_cfg(), &names, &Request::default(), &opts).unwrap_err();
    assert!(err.stopped);
    assert_eq!(
        err.attempts.len(),
        1,
        "the second provider must not be tried"
    );
    assert!(err.to_string().contains("bad key"));
}

#[test]
fn check_refusal_sends_no_request() {
    let build = |_: &DelegateProviderConfig| -> Result<Box<dyn Provider>, DelegateError> {
        Ok(Box::new(Scripted(Err(DelegateError::transient(
            "unreached",
        )))))
    };
    let check = |_: &DelegateProviderConfig, _: &Request| -> Result<(), DelegateError> {
        Err(DelegateError::terminal("budget"))
    };
    let sleep = |_: Duration| Ok(());
    let opts = ChainOptions {
        chain: String::new(),
        confirm: true,
        max_retry_after: DEFAULT_MAX_RETRY_AFTER,
        build: &build,
        check: Some(&check),
        sleep: &sleep,
    };
    let names = vec!["first".to_owned()];
    let err = run_chain(&chain_cfg(), &names, &Request::default(), &opts).unwrap_err();
    assert!(err.stopped);
    assert_eq!(
        err.attempts[0].class,
        Some("terminal"),
        "the refusal is terminal and nothing is sent"
    );
}

#[test]
fn flatten_matches_completion_style_and_leaves_label_open_after_user() {
    assert_eq!(flatten(&[Message::user("hai")]), "User: hai\n\nAssistant:");
    assert_eq!(
        flatten(&[Message::user("a"), Message::assistant("b")]),
        "User: a\n\nAssistant: b\n\n"
    );
    assert_eq!(flatten(&[]), "Assistant:");
}

/// Provider that only implements `complete`, to exercise the flattening default.
struct PromptOnly {
    seen: std::sync::Mutex<Vec<String>>,
}

impl Provider for PromptOnly {
    fn name(&self) -> &str {
        "prompt-only"
    }

    fn complete(&self, req: &Request) -> Result<ProviderReply, DelegateError> {
        self.seen.lock().expect("lock").push(req.prompt.clone());
        Ok(ProviderReply {
            text: "ok".into(),
            input_tokens: None,
            output_tokens: None,
        })
    }
}

#[test]
fn default_complete_messages_flattens_for_single_prompt_providers() {
    let provider = PromptOnly {
        seen: std::sync::Mutex::new(Vec::new()),
    };
    let messages = vec![
        Message::user("satu"),
        Message::assistant("dua"),
        Message::user("tiga"),
    ];
    let req = Request::default();
    let out = run_messages(&provider, &messages, &req).expect("run");
    assert_eq!(out.text, "ok");
    assert_eq!(
        provider.seen.lock().expect("lock")[0],
        "User: satu\n\nAssistant: dua\n\nUser: tiga\n\nAssistant:"
    );
}
