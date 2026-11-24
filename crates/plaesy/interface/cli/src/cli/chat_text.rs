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

#[cfg(test)]
#[path = "chat_text_test.rs"]
mod chat_text_test;
