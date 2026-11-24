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
    run_with_recorder(first, open, &|_| Ok("saved".to_owned()), script)
}

fn run_with_recorder(
    first: Box<dyn Provider>,
    open: &ProviderOpener<'_>,
    record: &DecisionRecorder<'_>,
    script: &str,
) -> String {
    let mut out = Vec::new();
    converse(
        first,
        open,
        record,
        Duration::from_secs(5),
        0,
        Cursor::new(script.to_owned()),
        &mut out,
    )
    .expect("converse");
    String::from_utf8(out).expect("utf8 output")
}

#[test]
fn parse_input_recognises_decision_with_raw_rest() {
    assert_eq!(
        parse_input("/decision a | b | c"),
        Input::Decision("a | b | c".to_owned())
    );
    assert_eq!(parse_input("/decision"), Input::Decision(String::new()));
    assert_eq!(
        parse_input("/decisions"),
        Input::Message("/decisions".to_owned())
    );
}

#[test]
fn parse_decision_requires_three_non_empty_parts() {
    assert_eq!(
        parse_decision("judul | pilih A | karena cepat"),
        Ok(Decision {
            title: "judul".to_owned(),
            chosen: "pilih A".to_owned(),
            why: "karena cepat".to_owned(),
        })
    );
    assert_eq!(
        parse_decision("judul | pilih A"),
        Err(DECISION_USAGE.to_owned())
    );
    assert_eq!(
        parse_decision("judul |  | alasan"),
        Err(DECISION_USAGE.to_owned())
    );
    assert_eq!(parse_decision(""), Err(DECISION_USAGE.to_owned()));
}

#[test]
fn decision_args_target_the_decisions_folder_with_optional_vault() {
    let decision = Decision {
        title: "Pakai Ollama".to_owned(),
        chosen: "Ollama lokal".to_owned(),
        why: "gratis".to_owned(),
    };
    let plain = decision_args(&decision, None, "decision-abc");
    assert_eq!(plain[0], "memory");
    assert!(!plain.contains(&"--vault".to_owned()));
    assert!(plain.windows(2).any(|w| w == ["--folder", "decisions"]));
    assert!(plain.windows(2).any(|w| w == ["--type", "decision"]));
    assert!(plain.windows(2).any(|w| w == ["--id", "decision-abc"]));
    assert!(plain.iter().any(|a| a.contains("Pilihan: Ollama lokal")));

    let with_vault = decision_args(&decision, Some(Path::new("C:/vault")), "decision-abc");
    assert_eq!(&with_vault[..3], ["memory", "--vault", "C:/vault"]);
}

#[test]
fn decision_id_is_stable_for_the_same_input_and_differs_by_time() {
    let first = decision_id("Pakai Ollama", 1);
    assert_eq!(first, decision_id("Pakai Ollama", 1));
    assert_ne!(first, decision_id("Pakai Ollama", 2));
    assert_ne!(first, decision_id("Lainnya", 1));
    assert!(first.starts_with("decision-") && first.len() == "decision-".len() + 16);
}

#[test]
fn decision_command_saves_through_the_recorder_and_reports_it() {
    let (provider, _) = fake("a", "x");
    let saved = Mutex::new(Vec::new());
    let record = |d: &Decision| -> Result<String, String> {
        saved.lock().expect("saved").push(d.clone());
        Ok("note saved".to_owned())
    };
    let out = run_with_recorder(
        provider,
        &no_open,
        &record,
        "/decision judul | pilih A | alasan\n/exit\n",
    );

    assert!(out.contains("decision note saved"), "output was: {out}");
    let saved = saved.lock().expect("saved").clone();
    assert_eq!(saved.len(), 1);
    assert_eq!(saved[0].chosen, "pilih A");
}

#[test]
fn bad_decision_prints_usage_and_saves_nothing() {
    let (provider, _) = fake("a", "x");
    let calls = Mutex::new(0usize);
    let record = |_: &Decision| -> Result<String, String> {
        *calls.lock().expect("calls") += 1;
        Ok(String::new())
    };
    let out = run_with_recorder(provider, &no_open, &record, "/decision rusak\n/exit\n");

    assert!(out.contains(DECISION_USAGE));
    assert_eq!(*calls.lock().expect("calls"), 0);
}

#[test]
fn recorder_failure_is_reported_not_fatal() {
    let (provider, seen) = fake("a", "tetap");
    let record = |_: &Decision| -> Result<String, String> { Err("vault tidak ada".to_owned()) };
    let out = run_with_recorder(
        provider,
        &no_open,
        &record,
        "/decision a | b | c\nhai\n/exit\n",
    );

    assert!(out.contains("error: vault tidak ada"));
    assert!(
        out.contains("tetap"),
        "the session continues after a failed save"
    );
    assert_eq!(seen.lock().expect("seen").len(), 1);
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
