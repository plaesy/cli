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
    run_with_store(first, open, record, &MemoryStore::default(), script)
}

/// In-memory `PendingStore`, so the chat loop is tested without a vault.
#[derive(Default)]
struct MemoryStore {
    items: Mutex<Vec<(String, String, Vec<String>)>>,
    fail_answer: bool,
}

impl chat_pending::PendingStore for MemoryStore {
    fn update(&self, id: &str, text: &str) -> Result<String, String> {
        let mut items = self.items.lock().expect("items");
        let item = items
            .iter_mut()
            .find(|(item_id, _, _)| item_id == id)
            .ok_or_else(|| format!("no decision {id}"))?;
        item.1 = text.to_owned();
        Ok(format!("{id} updated"))
    }

    fn list_decisions(&self) -> Result<String, String> {
        let items = self.items.lock().expect("items");
        if items.is_empty() {
            return Ok("no decisions".to_owned());
        }
        Ok(items
            .iter()
            .map(|(id, question, _)| format!("{id} [proposed] {question}"))
            .collect::<Vec<_>>()
            .join(
                "
",
            ))
    }

    fn archive(&self, id: &str) -> Result<String, String> {
        let mut items = self.items.lock().expect("items");
        let before = items.len();
        items.retain(|(item_id, _, _)| item_id != id);
        if items.len() == before {
            return Err(format!("no decision {id}"));
        }
        Ok(format!("{id} archived"))
    }

    fn ask(&self, question: &str, options: &[String]) -> Result<String, String> {
        let mut items = self.items.lock().expect("items");
        let id = format!("decision-{}", items.len() + 1);
        items.push((id.clone(), question.to_owned(), options.to_vec()));
        Ok(id)
    }

    fn list(&self) -> Result<String, String> {
        let items = self.items.lock().expect("items");
        if items.is_empty() {
            return Ok("no pending questions".to_owned());
        }
        let mut out = String::new();
        for (id, question, options) in items.iter() {
            out.push_str(&format!("{id} {question}\n"));
            for (index, option) in options.iter().enumerate() {
                out.push_str(&format!("   {}. {option}\n", index + 1));
            }
        }
        Ok(out.trim_end().to_owned())
    }

    fn answer(&self, id: &str, choice: usize) -> Result<String, String> {
        if self.fail_answer {
            return Err("vault tidak ada".to_owned());
        }
        let mut items = self.items.lock().expect("items");
        let index = items
            .iter()
            .position(|(item_id, _, _)| item_id == id)
            .ok_or_else(|| format!("no pending question {id}"))?;
        let chosen = choice
            .checked_sub(1)
            .and_then(|i| items[index].2.get(i))
            .cloned()
            .ok_or_else(|| format!("option {choice} does not exist for {id}"))?;
        items.remove(index);
        Ok(format!("{id} answered: {chosen}"))
    }
}

fn run_with_store(
    first: Box<dyn Provider>,
    open: &ProviderOpener<'_>,
    record: &DecisionRecorder<'_>,
    store: &dyn chat_pending::PendingStore,
    script: &str,
) -> String {
    let mut out = Vec::new();
    converse(
        first,
        open,
        record,
        store,
        ChatSettings {
            timeout: Duration::from_secs(5),
            max_tokens: 0,
            lang: Lang::En,
            repo: PathBuf::from("."),
            interactive: false,
            hooks: Vec::new(),
            approved: Vec::new(),
            mcp_servers: Vec::new(),
            roles: Vec::new(),
            role: "default".to_owned(),
            skills_dir: PathBuf::from("."),
            plugins: Vec::new(),
            agent_name: "chat".to_owned(),
            peers: Vec::new(),
            advisor: None,
            council: Vec::new(),
            lsp_servers: Vec::new(),
        },
        Cursor::new(script.to_owned()),
        &mut out,
    )
    .expect("converse");
    String::from_utf8(out).expect("utf8 output")
}

#[test]
fn parse_input_recognises_queue_commands() {
    assert_eq!(parse_input("/pending"), Input::Pending);
    assert_eq!(
        parse_input("/ask Q? | a | b"),
        Input::Ask("Q? | a | b".to_owned())
    );
    assert_eq!(parse_input("/pick 1 2"), Input::Pick("1 2".to_owned()));
    assert_eq!(
        parse_input("/pending now"),
        Input::Message("/pending now".to_owned())
    );
}

#[test]
fn ask_then_pending_lists_the_question_with_numbered_options() {
    let (provider, _) = fake("a", "x");
    let store = MemoryStore::default();
    let out = run_with_store(
        provider,
        &no_open,
        &|_| Ok("saved".to_owned()),
        &store,
        "/ask Pakai SQLite? | ya | tidak\n/pending\n/exit\n",
    );
    assert!(
        out.contains("proposed decision-1: Pakai SQLite?"),
        "output was: {out}"
    );
    assert!(out.contains("decision-1 Pakai SQLite?"));
    assert!(out.contains("   2. tidak"));
}

#[test]
fn pick_answers_the_question_and_removes_it_from_pending() {
    let (provider, _) = fake("a", "x");
    let store = MemoryStore::default();
    let out = run_with_store(
        provider,
        &no_open,
        &|_| Ok("saved".to_owned()),
        &store,
        "/ask Q? | a | b\n/pick decision-1 2\n/pending\n/exit\n",
    );
    assert!(out.contains("decision-1 answered: b"), "output was: {out}");
    assert!(out.contains("no pending questions"), "output was: {out}");
}

#[test]
fn failed_answer_keeps_the_question_pending() {
    let (provider, _) = fake("a", "x");
    let store = MemoryStore {
        fail_answer: true,
        ..MemoryStore::default()
    };
    let out = run_with_store(
        provider,
        &no_open,
        &|_| Ok("saved".to_owned()),
        &store,
        "/ask Q? | a | b\n/pick decision-1 1\n/pending\n/exit\n",
    );
    assert!(out.contains("error: vault tidak ada"), "output was: {out}");
    assert!(
        out.contains("decision-1 Q?"),
        "the question must stay pending: {out}"
    );
}

#[test]
fn pick_with_bad_arguments_prints_usage() {
    let (provider, _) = fake("a", "x");
    let store = MemoryStore::default();
    let out = run_with_store(
        provider,
        &no_open,
        &|_| Ok(String::new()),
        &store,
        "/pick x\n/exit\n",
    );
    assert!(
        out.contains(chat_text::usage(Lang::En, Usage::Pick)),
        "output was: {out}"
    );
}

#[test]
fn parse_input_recognises_decision_with_raw_rest() {
    assert_eq!(
        parse_input("/decision a | b | c"),
        Input::Decision("a | b | c".to_owned())
    );
    assert_eq!(parse_input("/decision"), Input::Decision(String::new()));
    assert_eq!(parse_input("/decisions"), Input::Decisions(String::new()));
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
    assert_eq!(parse_decision("judul | pilih A"), Err(Usage::Decision));
    assert_eq!(parse_decision("judul |  | alasan"), Err(Usage::Decision));
    assert_eq!(parse_decision(""), Err(Usage::Decision));
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
    assert!(plain.iter().any(|a| a.contains("chosen: Ollama lokal")));

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

    assert!(out.contains(chat_text::usage(Lang::En, Usage::Decision)));
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

#[test]
fn decisions_lists_proposed_items_and_archive_removes_them() {
    let (provider, _) = fake("a", "x");
    let store = MemoryStore::default();
    let out = run_with_store(
        provider,
        &no_open,
        &|_| Ok("saved".to_owned()),
        &store,
        "/ask Q? | a | b\n/decisions\n/decisions delete decision-1\n/decisions\n/exit\n",
    );
    assert!(
        out.contains("decision-1 [proposed] Q?"),
        "output was: {out}"
    );
    assert!(out.contains("decision-1 archived"), "output was: {out}");
    assert!(
        out.contains("no decisions"),
        "archived item leaves the list: {out}"
    );
}

#[test]
fn decisions_delete_of_unknown_id_is_reported() {
    let (provider, _) = fake("a", "x");
    let store = MemoryStore::default();
    let out = run_with_store(
        provider,
        &no_open,
        &|_| Ok("saved".to_owned()),
        &store,
        "/decisions delete nope\n/exit\n",
    );
    assert!(out.contains("error: no decision nope"), "output was: {out}");
}

#[test]
fn decisions_with_text_still_saves_a_decision() {
    let (provider, _) = fake("a", "x");
    let store = MemoryStore::default();
    let out = run_with_store(
        provider,
        &no_open,
        &|_| Ok("decision note saved".to_owned()),
        &store,
        "/decisions jangan full test workspace\n/exit\n",
    );
    assert!(out.contains("decision note saved"), "output was: {out}");
}

#[test]
fn decisions_update_rewrites_the_text_in_place() {
    let (provider, _) = fake("a", "x");
    let store = MemoryStore::default();
    let out = run_with_store(
        provider,
        &no_open,
        &|_| Ok("saved".to_owned()),
        &store,
        "/ask Lama? | a | b\n/decisions update decision-1 Baru\n/decisions\n/exit\n",
    );
    assert!(out.contains("decision-1 updated"), "output was: {out}");
    assert!(
        out.contains("decision-1 [proposed] Baru"),
        "output was: {out}"
    );
}

#[test]
fn decisions_update_without_text_prints_usage() {
    let (provider, _) = fake("a", "x");
    let store = MemoryStore::default();
    let out = run_with_store(
        provider,
        &no_open,
        &|_| Ok("saved".to_owned()),
        &store,
        "/decisions update decision-1\n/exit\n",
    );
    assert!(
        out.contains("usage: /decisions update"),
        "output was: {out}"
    );
}

#[test]
fn decisions_delete_without_id_prints_usage_and_saves_nothing() {
    let (provider, _) = fake("a", "x");
    let store = MemoryStore::default();
    let out = run_with_store(
        provider,
        &no_open,
        &|_| Ok("saved".to_owned()),
        &store,
        "/decisions delete
/exit
",
    );
    assert!(
        out.contains("usage: /decisions delete <id>"),
        "output was: {out}"
    );
    assert!(
        !out.contains("decision note saved"),
        "must not save as a decision: {out}"
    );
}

#[test]
fn single_statement_decision_body_says_keputusan_not_an_empty_alasan() {
    let statement = Decision {
        title: "Uji".to_owned(),
        chosen: "Uji".to_owned(),
        why: String::new(),
    };
    let args = decision_args(&statement, None, "decision-x");
    let body = args.iter().find(|a| a.starts_with("statement:")).cloned();
    assert_eq!(body.as_deref(), Some("statement: Uji\n"), "args: {args:?}");
    assert!(!args.iter().any(|a| a.contains("why:")));
}

#[test]
fn role_system_prompt_is_sent_before_the_conversation() {
    let (provider, seen) = fake("a", "ok");
    let roles = vec![chat_roles::Role {
        name: "reviewer".to_owned(),
        system: Some("Tinjau dengan teliti.".to_owned()),
        ..chat_roles::Role::default()
    }];
    let settings = ChatSettings {
        timeout: Duration::from_secs(5),
        max_tokens: 0,
        lang: Lang::En,
        repo: PathBuf::from("."),
        interactive: false,
        hooks: Vec::new(),
        approved: Vec::new(),
        mcp_servers: Vec::new(),
        roles,
        role: "reviewer".to_owned(),
        skills_dir: PathBuf::from("."),
        plugins: Vec::new(),
        agent_name: "chat".to_owned(),
        peers: Vec::new(),
        advisor: None,
        council: Vec::new(),
        lsp_servers: Vec::new(),
    };
    let store = MemoryStore::default();
    let mut out = Vec::new();
    converse(
        provider,
        &no_open,
        &|_| Ok("saved".to_owned()),
        &store,
        settings,
        Cursor::new("halo\n/exit\n".to_owned()),
        &mut out,
    )
    .expect("converse");
    let prompts = seen.lock().expect("seen").clone();
    assert!(
        prompts[0].starts_with("System: Tinjau dengan teliti.\n\nUser: halo"),
        "prompt was: {}",
        prompts[0]
    );
}

#[test]
fn role_command_switches_and_refuses_unknown_names() {
    let (provider, seen) = fake("a", "ok");
    let roles = vec![chat_roles::Role {
        name: "penulis".to_owned(),
        system: Some("Tulis singkat.".to_owned()),
        ..chat_roles::Role::default()
    }];
    let settings = ChatSettings {
        timeout: Duration::from_secs(5),
        max_tokens: 0,
        lang: Lang::En,
        repo: PathBuf::from("."),
        interactive: false,
        hooks: Vec::new(),
        approved: Vec::new(),
        mcp_servers: Vec::new(),
        roles,
        role: "default".to_owned(),
        skills_dir: PathBuf::from("."),
        plugins: Vec::new(),
        agent_name: "chat".to_owned(),
        peers: Vec::new(),
        advisor: None,
        council: Vec::new(),
        lsp_servers: Vec::new(),
    };
    let store = MemoryStore::default();
    let mut out = Vec::new();
    converse(
        provider,
        &no_open,
        &|_| Ok("saved".to_owned()),
        &store,
        settings,
        Cursor::new("/role penulis\n/role tidak-ada\nhalo\n/exit\n".to_owned()),
        &mut out,
    )
    .expect("converse");
    let text = String::from_utf8(out).expect("utf8");
    assert!(text.contains("role: penulis"), "output was: {text}");
    assert!(
        text.contains("unknown role tidak-ada"),
        "output was: {text}"
    );
    assert!(seen.lock().expect("seen")[0].starts_with("System: Tulis singkat."));
}

#[test]
fn at_mentions_address_another_agent_and_bare_at_is_text() {
    assert_eq!(
        parse_input("@nara halo teman"),
        Input::Mention("nara".to_owned(), "halo teman".to_owned())
    );
    assert_eq!(
        parse_input("@creative"),
        Input::Mention("creative".to_owned(), String::new())
    );
    assert_eq!(parse_input("@ halo"), Input::Message("@ halo".to_owned()));
    assert_eq!(
        parse_input("email a@b.com"),
        Input::Message("email a@b.com".to_owned())
    );
}

#[test]
fn bare_at_opens_the_peer_menu() {
    assert_eq!(parse_input("@"), Input::MentionMenu);
    assert_eq!(parse_input("@ "), Input::MentionMenu);
}

#[test]
fn goal_and_compact_parse_as_their_own_commands() {
    assert_eq!(parse_input("/goal"), Input::Goal(String::new()));
    assert_eq!(
        parse_input("/goal tes unit per fitur"),
        Input::Goal("tes unit per fitur".to_owned())
    );
    assert_eq!(parse_input("/goal clear"), Input::Goal("clear".to_owned()));
    assert_eq!(parse_input("/compact"), Input::Compact);
}

#[test]
fn goal_is_shown_in_every_prompt_until_cleared() {
    let (provider, seen) = fake("a", "ok");
    let out = run_session(
        provider,
        &no_open,
        "/goal\n/goal tes unit\nsatu\n/goal clear\ndua\n/exit\n",
    );

    assert!(out.contains("no goal set"));
    assert!(out.contains("goal: tes unit"));
    assert!(out.contains("goal cleared"));
    let prompts = seen.lock().expect("seen").clone();
    assert_eq!(prompts.len(), 2);
    assert!(prompts[0].contains("Goal sesi ini: tes unit"));
    assert!(!prompts[1].contains("Goal sesi ini"));
}

#[test]
fn compact_replaces_history_with_the_summary() {
    let (provider, seen) = fake("a", "ringkasan singkat");
    let out = run_session(provider, &no_open, "pertama\n/compact\nkedua\n/exit\n");

    assert!(out.contains("compacted 2 turns into one summary"));
    let prompts = seen.lock().expect("seen").clone();
    assert_eq!(prompts.len(), 3, "message, compact, then the next message");
    assert!(prompts[1].contains("pertama"), "compact sends the history");
    assert!(
        prompts[2].contains("ringkasan singkat") && !prompts[2].contains("pertama"),
        "after compact only the summary remains: {}",
        prompts[2]
    );
}

#[test]
fn compact_with_no_history_says_so_and_calls_nothing() {
    let (provider, seen) = fake("a", "tidak dipanggil");
    let out = run_session(provider, &no_open, "/compact\n/exit\n");

    assert!(out.contains("nothing to compact"));
    assert!(seen.lock().expect("seen").is_empty());
}

fn run_with_members(
    first: Box<dyn Provider>,
    open: &ProviderOpener<'_>,
    advisor: Option<&str>,
    council: &[&str],
    script: &str,
) -> String {
    let mut out = Vec::new();
    converse(
        first,
        open,
        &|_| Ok(String::new()),
        &MemoryStore::default(),
        ChatSettings {
            timeout: Duration::from_secs(5),
            max_tokens: 0,
            lang: Lang::En,
            repo: PathBuf::from("."),
            interactive: false,
            hooks: Vec::new(),
            approved: Vec::new(),
            mcp_servers: Vec::new(),
            roles: Vec::new(),
            role: "default".to_owned(),
            skills_dir: PathBuf::from("."),
            plugins: Vec::new(),
            agent_name: "chat".to_owned(),
            peers: Vec::new(),
            advisor: advisor.map(str::to_owned),
            council: council.iter().map(|name| (*name).to_owned()).collect(),
            lsp_servers: Vec::new(),
        },
        Cursor::new(script.to_owned()),
        &mut out,
    )
    .expect("converse");
    String::from_utf8(out).expect("utf8 output")
}

#[test]
fn advisor_and_council_parse_their_questions() {
    assert_eq!(parse_input("/advisor"), Input::Advisor(String::new()));
    assert_eq!(
        parse_input("/advisor apa risikonya?"),
        Input::Advisor("apa risikonya?".to_owned())
    );
    assert_eq!(parse_input("/council"), Input::Council(String::new()));
    assert_eq!(
        parse_input("/council pilih a atau b"),
        Input::Council("pilih a atau b".to_owned())
    );
}

#[test]
fn advisor_without_a_setting_says_so_and_calls_nothing() {
    let (provider, seen) = fake("a", "tidak dipanggil");
    let out = run_with_members(provider, &no_open, None, &[], "/advisor apa?\n/exit\n");

    assert!(out.contains("no advisor set"));
    assert!(seen.lock().expect("seen").is_empty());
}

#[test]
fn advisor_reviews_through_its_own_provider() {
    let (provider, _) = fake("a", "jawab");
    let open =
        |name: &str| -> Result<Box<dyn Provider>, String> { Ok(fake(name, "saran: uji dulu").0) };
    let out = run_with_members(
        provider,
        &open,
        Some("reviewer"),
        &[],
        "pertama\n/advisor apa risikonya?\n/exit\n",
    );

    assert!(out.contains("[advisor reviewer]"));
    assert!(out.contains("saran: uji dulu"));
}

#[test]
fn council_asks_every_member_separately() {
    let (provider, _) = fake("a", "tidak dipakai");
    let open = |name: &str| -> Result<Box<dyn Provider>, String> {
        Ok(fake(name, &format!("jawab {name}")).0)
    };
    let out = run_with_members(
        provider,
        &open,
        None,
        &["satu", "dua"],
        "/council pilih?\n/exit\n",
    );

    assert!(out.contains("[satu]") && out.contains("jawab satu"));
    assert!(out.contains("[dua]") && out.contains("jawab dua"));
    assert!(
        !out.contains("tidak dipakai"),
        "council does not use the current provider"
    );
}

#[test]
fn council_with_no_members_says_so() {
    let (provider, _) = fake("a", "x");
    let out = run_with_members(provider, &no_open, None, &[], "/council pilih?\n/exit\n");

    assert!(out.contains("no enabled providers for the council"));
}

#[test]
fn steered_line_runs_before_older_queued_lines() {
    let mut queue: VecDeque<String> = ["satu", "!dua", "tiga"]
        .iter()
        .map(|line| (*line).to_owned())
        .collect();
    assert_eq!(
        take_next(&mut queue),
        "dua",
        "the steered line goes first, without its !"
    );
    assert_eq!(take_next(&mut queue), "satu");
    assert_eq!(take_next(&mut queue), "tiga");
    assert!(queue.is_empty());
}

#[test]
fn queued_lines_run_in_order_when_none_is_steered() {
    let (provider, seen) = fake("a", "ok");
    let out = run_session(provider, &no_open, "pertama\nkedua\n/exit\n");

    assert!(out.contains("ok"), "both lines get a reply: {out}");
    let prompts = seen.lock().expect("seen").clone();
    assert_eq!(prompts.len(), 2);
    assert!(prompts[1].contains("pertama") && prompts[1].contains("kedua"));
}

#[test]
fn debug_reports_the_session_without_transcript_text() {
    assert_eq!(parse_input("/debug"), Input::Debug);
    let (provider, _) = fake("alpha", "rahasia-jawaban");
    let out = run_with_members(
        provider,
        &no_open,
        Some("reviewer"),
        &["satu"],
        "rahasia-pertanyaan\n/debug\n/exit\n",
    );

    assert!(out.contains("provider: alpha"), "{out}");
    assert!(out.contains("advisor: reviewer"), "{out}");
    assert!(out.contains("council members: 1"), "{out}");
    assert!(out.contains("turns: 2"), "{out}");
}
