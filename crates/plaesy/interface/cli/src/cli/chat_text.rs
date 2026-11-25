//! User-facing text for `plaesy chat`, in English or Indonesian.
//!
//! The language comes from `--lang`, then `"language"` in `~/.plaesy/settings.json`, then English.
//! Only what the user reads is translated. Keys stored inside decision notes (`statement:`,
//! `chosen:`, `why:`, `options:`) stay English, so updates do not depend on the language.

use std::path::Path;

use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Lang {
    #[default]
    En,
    Id,
}

impl Lang {
    pub fn parse(raw: &str) -> Result<Lang, String> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "en" | "english" => Ok(Lang::En),
            "id" | "indonesia" => Ok(Lang::Id),
            other => Err(format!("unknown language {other:?}; use en or id")),
        }
    }
}

/// `--lang` wins, then `"language"` in the settings file, then English. A missing settings file
/// is not an error; an unknown language value is.
pub fn resolve(flag: Option<&str>, settings: &Path) -> Result<Lang, String> {
    if let Some(flag) = flag {
        return Lang::parse(flag);
    }
    if let Ok(raw) = std::fs::read_to_string(settings) {
        if let Ok(value) = serde_json::from_str::<Value>(&raw) {
            if let Some(lang) = value.get("language").and_then(Value::as_str) {
                return Lang::parse(lang);
            }
        }
    }
    Ok(Lang::En)
}

fn pick(lang: Lang, en: &'static str, id: &'static str) -> &'static str {
    match lang {
        Lang::En => en,
        Lang::Id => id,
    }
}

/// The usage lines a command can print when its arguments are wrong.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Usage {
    Ask,
    Pick,
    Decision,
    DecisionsUpdate,
    DecisionsDelete,
    Mcp,
    Plugin,
    Agent,
    Provider,
}

pub fn usage(lang: Lang, kind: Usage) -> &'static str {
    match kind {
        Usage::Ask => pick(
            lang,
            "usage: /ask <question> | <option 1> | <option 2> [| ...]",
            "usage: /ask <pertanyaan> | <opsi 1> | <opsi 2> [| ...]",
        ),
        Usage::Pick => pick(
            lang,
            "usage: /pick <id> <option number>",
            "usage: /pick <id> <nomor opsi>",
        ),
        Usage::Decision => pick(
            lang,
            "usage: /decision <title> | <choice> | <reason>",
            "usage: /decision <judul> | <pilihan> | <alasan>",
        ),
        Usage::DecisionsUpdate => pick(
            lang,
            "usage: /decisions update <id> <new text>",
            "usage: /decisions update <id> <teks baru>",
        ),
        Usage::DecisionsDelete => "usage: /decisions delete <id>",
        Usage::Mcp => pick(
            lang,
            "usage: /mcp | /mcp call <server> <tool> <json>",
            "usage: /mcp | /mcp call <server> <tool> <json>",
        ),
        Usage::Plugin => "usage: /plugins | /plugin run <plugin> <command> [args...]",
        Usage::Agent => "usage: /agent inbox | /agent send <to> <text> | /agent reply <id> <text> | /agent ack <id>",
        Usage::Provider => "usage: /provider <name>",
    }
}

pub fn banner(lang: Lang, provider: &str) -> String {
    match lang {
        Lang::En => format!(
            "chat with {provider}. /provider <name> switches, /decision <title> | <choice> | <reason> saves a decision, /decisions lists them, /ask asks a question, /pending lists open questions, /clear resets, /exit quits."
        ),
        Lang::Id => format!(
            "chat dengan {provider}. /provider <nama> berganti provider, /decision <judul> | <pilihan> | <alasan> menyimpan keputusan, /decisions menampilkan daftar, /ask mengajukan pertanyaan, /pending menampilkan pertanyaan terbuka, /clear mengulang, /exit keluar."
        ),
    }
}

pub fn history_cleared(lang: Lang) -> &'static str {
    pick(lang, "history cleared", "riwayat dihapus")
}

pub fn provider_switched(lang: Lang, name: &str) -> String {
    format!("{}: {name}", pick(lang, "provider", "provider"))
}

pub fn error(lang: Lang, detail: &str) -> String {
    format!("{}: {detail}", pick(lang, "error", "galat"))
}

pub fn warning(lang: Lang, detail: &str) -> String {
    format!("{}: {detail}", pick(lang, "warning", "peringatan"))
}

pub fn decision_saved(lang: Lang, saved: &str) -> String {
    format!("{} {saved}", pick(lang, "decision", "keputusan"))
}

pub fn question_proposed(lang: Lang, id: &str, question: &str) -> String {
    format!("{} {id}: {question}", pick(lang, "proposed", "diusulkan"))
}

pub fn answered(lang: Lang, id: &str, chosen: &str) -> String {
    format!("{id} {}: {chosen}", pick(lang, "answered", "dijawab"))
}

pub fn archived(lang: Lang, id: &str) -> String {
    format!("{id} {}", pick(lang, "archived", "diarsipkan"))
}

pub fn updated(lang: Lang, id: &str) -> String {
    format!("{id} {}", pick(lang, "updated", "diperbarui"))
}

pub fn no_decisions(lang: Lang) -> &'static str {
    pick(lang, "no decisions", "belum ada keputusan")
}

pub fn goal_none(lang: Lang) -> &'static str {
    pick(lang, "no goal set", "belum ada goal")
}

pub fn goal_set(lang: Lang, goal: &str) -> String {
    format!("{}: {goal}", pick(lang, "goal", "goal"))
}

pub fn goal_cleared(lang: Lang) -> &'static str {
    pick(lang, "goal cleared", "goal dihapus")
}

pub fn nothing_to_compact(lang: Lang) -> &'static str {
    pick(lang, "nothing to compact", "tidak ada yang diringkas")
}

pub fn compacted(lang: Lang, turns: usize) -> String {
    match lang {
        Lang::En => format!("compacted {turns} turns into one summary"),
        Lang::Id => format!("{turns} giliran diringkas menjadi satu catatan"),
    }
}

pub fn no_pending(lang: Lang) -> &'static str {
    pick(lang, "no pending questions", "tidak ada pertanyaan terbuka")
}

pub fn cancelled(lang: Lang) -> &'static str {
    pick(lang, "cancelled", "dibatalkan")
}

pub fn decisions_count(lang: Lang, count: usize) -> String {
    match lang {
        Lang::En => format!("{count} decisions"),
        Lang::Id => format!("{count} keputusan"),
    }
}

pub fn open_questions_count(lang: Lang, count: usize) -> String {
    match lang {
        Lang::En => format!("{count} open questions"),
        Lang::Id => format!("{count} pertanyaan terbuka"),
    }
}

pub fn action_edit(lang: Lang) -> &'static str {
    pick(lang, "Edit text", "Ubah teks")
}

pub fn action_archive(lang: Lang) -> &'static str {
    pick(lang, "Archive", "Arsipkan")
}

pub fn action_cancel(lang: Lang) -> &'static str {
    pick(lang, "Cancel", "Batal")
}

pub fn not_found(lang: Lang, id: &str) -> String {
    format!("{} {id}", pick(lang, "no such item", "tidak ditemukan"))
}

pub fn no_fingerprint(lang: Lang, path: &str) -> String {
    format!(
        "{} {path}",
        pick(lang, "no fingerprint for", "tidak ada fingerprint untuk")
    )
}

pub fn not_proposed(lang: Lang, id: &str) -> String {
    format!(
        "{id} {}",
        pick(
            lang,
            "is not a proposed decision",
            "bukan keputusan proposed"
        )
    )
}

pub fn option_missing(lang: Lang, choice: usize, id: &str, count: usize) -> String {
    match lang {
        Lang::En => format!("option {choice} does not exist for {id} (1..={count})"),
        Lang::Id => format!("opsi {choice} tidak ada untuk {id} (1..={count})"),
    }
}

pub fn pass_vault(lang: Lang) -> &'static str {
    pick(
        lang,
        "pass --vault: option text is read from the vault",
        "beri --vault: teks opsi dibaca dari vault",
    )
}

pub fn no_tasks(lang: Lang) -> &'static str {
    pick(lang, "no tasks", "belum ada tugas")
}

pub fn tasks_count(lang: Lang, count: usize) -> String {
    match lang {
        Lang::En => format!("{count} tasks"),
        Lang::Id => format!("{count} tugas"),
    }
}

pub fn action_next(lang: Lang) -> &'static str {
    pick(lang, "Move to todo", "Pindah ke todo")
}

pub fn action_start(lang: Lang) -> &'static str {
    pick(lang, "Start", "Mulai")
}

pub fn action_block(lang: Lang) -> &'static str {
    pick(lang, "Block", "Blokir")
}

pub fn action_unblock(lang: Lang) -> &'static str {
    pick(lang, "Unblock", "Buka blokir")
}

pub fn action_complete(lang: Lang) -> &'static str {
    pick(lang, "Complete", "Selesaikan")
}

pub fn action_show(lang: Lang) -> &'static str {
    pick(lang, "Show", "Lihat")
}

pub fn block_reason_prompt(lang: Lang) -> &'static str {
    pick(lang, "Reason for blocking", "Alasan diblokir")
}

pub fn task_done(lang: Lang, id: &str) -> String {
    format!("{id} {}", pick(lang, "updated", "diperbarui"))
}

#[cfg(test)]
#[path = "chat_text_test.rs"]
mod chat_text_test;

pub fn advisor_usage(lang: Lang) -> &'static str {
    pick(
        lang,
        "usage: /advisor <question>",
        "pemakaian: /advisor <pertanyaan>",
    )
}

pub fn advisor_not_set(lang: Lang) -> &'static str {
    pick(
        lang,
        "no advisor set; add chat.advisor to settings.json",
        "belum ada advisor; tambahkan chat.advisor di settings.json",
    )
}

pub fn council_usage(lang: Lang) -> &'static str {
    pick(
        lang,
        "usage: /council <question>",
        "pemakaian: /council <pertanyaan>",
    )
}

/// Shown when the provider answers without any text, so the person knows the turn ended empty.
pub fn empty_reply(lang: Lang) -> &'static str {
    pick(
        lang,
        "the provider returned no text; try again or check /debug",
        "provider tidak mengembalikan teks; coba lagi atau cek /debug",
    )
}

pub fn council_empty(lang: Lang) -> &'static str {
    pick(
        lang,
        "no enabled providers for the council",
        "tidak ada provider aktif untuk council",
    )
}

pub fn queued(lang: Lang, waiting: usize) -> String {
    match lang {
        Lang::En => format!("queued: {waiting} waiting"),
        Lang::Id => format!("antre: {waiting} menunggu"),
    }
}

/// What `/debug` reports about the running session. Nothing secret: no keys, no transcript text.
pub struct DebugFacts<'a> {
    pub version: &'a str,
    pub provider: &'a str,
    pub role: &'a str,
    pub repo: &'a str,
    pub agent: &'a str,
    pub turns: usize,
    pub goal_set: bool,
    pub hooks: usize,
    pub mcp_servers: usize,
    pub plugins: usize,
    pub roles: usize,
    pub peers: usize,
    pub advisor: Option<&'a str>,
    pub council: usize,
    pub timeout: &'a str,
    pub max_tokens: u32,
}

pub fn debug_report(lang: Lang, facts: &DebugFacts<'_>) -> String {
    let rows: [(&str, &str, String); 16] = [
        ("version", "versi", facts.version.to_owned()),
        ("provider", "provider", facts.provider.to_owned()),
        ("role", "role", facts.role.to_owned()),
        ("repo", "repo", facts.repo.to_owned()),
        ("agent", "agen", facts.agent.to_owned()),
        (
            "language",
            "bahasa",
            match lang {
                Lang::En => "en",
                Lang::Id => "id",
            }
            .to_owned(),
        ),
        ("turns", "giliran", facts.turns.to_string()),
        (
            "goal",
            "goal",
            if facts.goal_set { "set" } else { "none" }.to_owned(),
        ),
        ("hooks", "hook", facts.hooks.to_string()),
        ("mcp servers", "server mcp", facts.mcp_servers.to_string()),
        ("plugins", "plugin", facts.plugins.to_string()),
        ("roles", "role", facts.roles.to_string()),
        ("peers", "peer", facts.peers.to_string()),
        (
            "advisor",
            "advisor",
            facts.advisor.unwrap_or("none").to_owned(),
        ),
        (
            "council members",
            "anggota council",
            facts.council.to_string(),
        ),
        ("timeout", "timeout", facts.timeout.to_owned()),
    ];
    let mut lines = vec![pick(lang, "debug", "debug").to_owned()];
    for (en, id, value) in rows {
        lines.push(format!("  {}: {value}", pick(lang, en, id)));
    }
    lines.push(format!(
        "  {}: {}",
        pick(lang, "max tokens", "token maksimum"),
        if facts.max_tokens == 0 {
            pick(lang, "provider default", "default provider").to_owned()
        } else {
            facts.max_tokens.to_string()
        }
    ));
    lines.join("\n")
}

pub fn no_providers(lang: Lang) -> &'static str {
    pick(
        lang,
        "no providers configured",
        "belum ada provider terdaftar",
    )
}

/// One line of `/providers`: whether it may run, and whether it is the one in use.
pub fn provider_line(
    lang: Lang,
    name: &str,
    enabled: bool,
    billable: bool,
    active: bool,
) -> String {
    let status = match (enabled, billable) {
        (false, _) => pick(lang, "disabled", "nonaktif"),
        (true, true) => pick(lang, "enabled, billable", "aktif, berbayar"),
        (true, false) => pick(lang, "enabled, free", "aktif, gratis"),
    };
    let marker = if active { "* " } else { "  " };
    format!("{marker}{name}: {status}")
}
