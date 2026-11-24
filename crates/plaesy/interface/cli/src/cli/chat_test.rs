use std::io::Cursor;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use super::*;
use crate::delegate::{DelegateError, ProviderReply};

/// Test provider: returns a fixed reply and records every prompt it receives.
struct Fake {
    name: String,
    reply: Result<String, String>,
    seen: Arc<Mutex<Vec<String>>>,
}

impl Provider for Fake {
    fn name(&self) -> &str {
        &self.name
    }

    fn complete(&self, req: &Request) -> Result<ProviderReply, DelegateError> {
        self.seen
            .lock()
            .expect("seen lock")
            .push(req.prompt.clone());
        match &self.reply {
            Ok(text) => Ok(ProviderReply {
                text: text.clone(),
                input_tokens: None,
                output_tokens: None,
            }),
            Err(msg) => Err(DelegateError::terminal(msg.clone())),
        }
    }
}

fn fake(name: &str, reply: &str) -> (Box<dyn Provider>, Arc<Mutex<Vec<String>>>) {
    let seen = Arc::new(Mutex::new(Vec::new()));
    let provider = Fake {
        name: name.to_owned(),
        reply: Ok(reply.to_owned()),
        seen: Arc::clone(&seen),
    };
    (Box::new(provider), seen)
}

fn run_session(first: Box<dyn Provider>, open: &ProviderOpener<'_>, script: &str) -> String {
    let mut out = Vec::new();
    converse(
        first,
        open,
        Duration::from_secs(5),
        0,
        Cursor::new(script.to_owned()),
        &mut out,
    )
    .expect("converse");
    String::from_utf8(out).expect("utf8 output")
}

fn no_open(name: &str) -> Result<Box<dyn Provider>, String> {
    Err(format!("unexpected open of {name}"))
}

#[test]
fn parse_input_recognises_commands_and_messages() {
    assert_eq!(parse_input("   "), Input::Empty);
    assert_eq!(parse_input("/exit"), Input::Exit);
    assert_eq!(parse_input(" /quit "), Input::Exit);
    assert_eq!(parse_input("/clear"), Input::Clear);
    assert_eq!(
        parse_input("/provider ollama"),
        Input::Provider("ollama".to_owned())
    );
    assert_eq!(parse_input("/provider"), Input::ProviderMissing);
    assert_eq!(parse_input("/provider  "), Input::ProviderMissing);
    assert_eq!(
        parse_input("/providerx"),
        Input::Message("/providerx".to_owned())
    );
    assert_eq!(parse_input("halo"), Input::Message("halo".to_owned()));
}

#[test]
fn transcript_renders_history_then_open_label() {
    let mut t = Transcript::default();
    assert!(t.is_empty());
    assert_eq!(t.render("hai"), "User: hai\n\nAssistant:");

    t.push(Role::User, "satu");
    t.push(Role::Assistant, "dua");
    assert_eq!(t.len(), 2);
    assert_eq!(
        t.render("tiga"),
        "User: satu\n\nAssistant: dua\n\nUser: tiga\n\nAssistant:"
    );

    t.clear();
    assert!(t.is_empty());
}

#[test]
fn second_prompt_carries_the_first_exchange() {
    let (provider, seen) = fake("a", "jawaban");
    let out = run_session(
        provider,
        &no_open,
        "pertanyaan satu\npertanyaan dua\n/exit\n",
    );

    assert!(out.contains("jawaban"));
    let prompts = seen.lock().expect("seen").clone();
    assert_eq!(prompts.len(), 2);
    assert_eq!(prompts[0], "User: pertanyaan satu\n\nAssistant:");
    assert_eq!(
        prompts[1],
        "User: pertanyaan satu\n\nAssistant: jawaban\n\nUser: pertanyaan dua\n\nAssistant:"
    );
}

#[test]
fn clear_resets_history_and_exit_stops_reading() {
    let (provider, seen) = fake("a", "ok");
    let out = run_session(
        provider,
        &no_open,
        "satu\n/clear\nsatu lagi\n/exit\ntidak dibaca\n",
    );

    assert!(out.contains("history cleared"));
    let prompts = seen.lock().expect("seen").clone();
    assert_eq!(prompts.len(), 2, "text after /exit must not be sent");
    assert_eq!(prompts[1], "User: satu lagi\n\nAssistant:");
}

#[test]
fn failed_call_is_reported_and_not_recorded() {
    let seen = Arc::new(Mutex::new(Vec::new()));
    let failing = Fake {
        name: "a".to_owned(),
        reply: Err("boom".to_owned()),
        seen: Arc::clone(&seen),
    };
    let out = run_session(Box::new(failing), &no_open, "gagal\n/exit\n");

    assert!(out.contains("error: boom"), "output was: {out}");
    assert_eq!(seen.lock().expect("seen").len(), 1);
}

#[test]
fn provider_switch_keeps_transcript_and_uses_new_provider() {
    let (first, first_seen) = fake("a", "dari a");
    let (second, second_seen) = fake("b", "dari b");
    let second = Mutex::new(Some(second));
    let open = move |name: &str| -> Result<Box<dyn Provider>, String> {
        if name == "b" {
            second
                .lock()
                .expect("open lock")
                .take()
                .ok_or_else(|| "already opened".to_owned())
        } else {
            Err(format!("unknown provider {name:?}"))
        }
    };

    let out = run_session(first, &open, "pertama\n/provider b\nkedua\n/exit\n");

    assert!(out.contains("provider: b"));
    assert_eq!(first_seen.lock().expect("first").len(), 1);
    let second_prompts = second_seen.lock().expect("second").clone();
    assert_eq!(second_prompts.len(), 1);
    assert!(
        second_prompts[0].contains("User: pertama\n\nAssistant: dari a"),
        "new provider must receive the earlier exchange: {second_prompts:?}"
    );
}

#[test]
fn unknown_provider_keeps_current_provider() {
    let (first, first_seen) = fake("a", "tetap a");
    let open = |name: &str| -> Result<Box<dyn Provider>, String> {
        Err(format!("unknown provider {name:?}"))
    };
    let out = run_session(first, &open, "/provider zzz\nhai\n/exit\n");

    assert!(out.contains("error: unknown provider \"zzz\""));
    assert!(out.contains("tetap a"));
    assert_eq!(first_seen.lock().expect("first").len(), 1);
}

#[test]
fn provider_without_name_prints_usage() {
    let (first, _) = fake("a", "x");
    let out = run_session(first, &no_open, "/provider\n/exit\n");
    assert!(out.contains("usage: /provider <name>"));
}
