//! Parser perintah suara dari teks hasil transkripsi.
//!
//! Modul ini murni: menerima kalimat yang sudah ditranskripsi dan mengembalikan perintah
//! bertipe. Tidak ada mikrofon, model suara, atau I/O. Tiga perintah tugas dikenali, dengan
//! kata kunci Indonesia dan Inggris:
//!
//! - buat tugas: `buat tugas <judul>`, `tambahkan tugas <judul>`, `add task <title>`
//! - tandai selesai: `selesaikan tugas <judul>`, `tandai selesai <judul>`,
//!   `tandai tugas <judul> sebagai selesai`, `mark task <title> as done`
//! - daftar: `daftar tugas`, `tampilkan semua tugas`, `show all tasks`
//!
//! Kata pengisi di depan (`tolong`, `please`, `plaesy`) diabaikan. Judul mempertahankan huruf
//! besar dan tanda baca di dalam kalimat, tetapi tanda baca di ujung judul dibuang.

use serde::Serialize;
use thiserror::Error;

/// Perintah tugas yang dikenali dari sebuah kalimat. Parser hanya menghasilkan nilai ini;
/// eksekusinya ada di pemanggil.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum VoiceCommand {
    /// Membuat tugas baru dengan judul ini.
    CreateTask { title: String },
    /// Menandai tugas dengan judul (atau potongan judul) ini sebagai selesai.
    CompleteTask { title: String },
    /// Menampilkan daftar tugas.
    ListTasks,
}

/// Alasan sebuah teks tidak menghasilkan perintah.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum VoiceError {
    /// Teks kosong, atau hanya berisi kata pengisi.
    #[error("teks perintah kosong")]
    Empty,
    /// Kalimat tidak cocok dengan pola perintah mana pun. Teks yang dibersihkan disertakan.
    #[error("perintah tidak dikenali: \"{0}\"")]
    Unrecognized(String),
    /// Kata kerja dikenali, tetapi judul tugasnya kosong.
    #[error("perintah {action} membutuhkan judul tugas")]
    MissingTitle { action: &'static str },
}

const FILLER: &[&str] = &["tolong", "please", "plaesy"];
const CREATE_VERBS: &[&str] = &[
    "buat",
    "bikin",
    "tambah",
    "tambahkan",
    "create",
    "add",
    "new",
    "make",
];
const CREATE_SKIP: &[&str] = &["baru", "new", "untuk", "to"];
const LIST_VERBS: &[&str] = &["daftar", "lihat", "tampilkan", "tunjukkan", "list", "show"];
const LIST_SCOPE: &[&str] = &["semua", "all", "saya", "my"];
const COMPLETE_VERBS: &[&str] = &["selesaikan", "complete", "finish"];
const MARK_VERBS: &[&str] = &["tandai", "mark"];
const TASK_NOUNS: &[&str] = &["tugas", "task", "tasks"];
const DONE_WORDS: &[&str] = &[
    "selesai",
    "done",
    "complete",
    "completed",
    "sebagai",
    "as",
    "sudah",
];

/// Satu kata hasil pemecahan. `key` untuk pencocokan (huruf kecil, tanpa tanda baca di ujung);
/// `raw` dipakai untuk judul agar huruf dan tanda baca di dalam kalimat tetap utuh.
#[derive(Debug, Clone)]
struct Token {
    raw: String,
    key: String,
}

/// Mengubah kalimat transkripsi menjadi perintah tugas.
///
/// # Errors
///
/// Returns [`VoiceError::Empty`] when the text has no words after filler removal,
/// [`VoiceError::MissingTitle`] when a verb is recognized without a task title, and
/// [`VoiceError::Unrecognized`] when no command pattern matches.
pub fn parse(input: &str) -> Result<VoiceCommand, VoiceError> {
    let tokens = tokenize(input);
    let tokens = skip_words(&tokens, FILLER);
    if tokens.is_empty() {
        return Err(VoiceError::Empty);
    }
    match_create(tokens)
        .or_else(|| match_list(tokens))
        .or_else(|| match_complete(tokens))
        .unwrap_or_else(|| Err(VoiceError::Unrecognized(input.trim().to_string())))
}

fn tokenize(input: &str) -> Vec<Token> {
    input
        .split_whitespace()
        .filter_map(|raw| {
            let key = raw
                .trim_matches(|c: char| !c.is_alphanumeric())
                .to_lowercase();
            (!key.is_empty()).then(|| Token {
                raw: raw.to_string(),
                key,
            })
        })
        .collect()
}

/// `Some` when the first word is one of `words`, carrying the slice after it.
fn strip_one<'a>(tokens: &'a [Token], words: &[&str]) -> Option<&'a [Token]> {
    let (first, rest) = tokens.split_first()?;
    words.contains(&first.key.as_str()).then_some(rest)
}

fn skip_words<'a>(tokens: &'a [Token], words: &[&str]) -> &'a [Token] {
    let count = tokens
        .iter()
        .take_while(|t| words.contains(&t.key.as_str()))
        .count();
    &tokens[count..]
}

fn trim_trailing<'a>(tokens: &'a [Token], words: &[&str]) -> &'a [Token] {
    let count = tokens
        .iter()
        .rev()
        .take_while(|t| words.contains(&t.key.as_str()))
        .count();
    &tokens[..tokens.len() - count]
}

fn title_of(tokens: &[Token]) -> String {
    let joined = tokens
        .iter()
        .map(|t| t.raw.as_str())
        .collect::<Vec<_>>()
        .join(" ");
    joined
        .trim_matches(|c: char| !c.is_alphanumeric())
        .to_string()
}

fn non_empty_title(tokens: &[Token], action: &'static str) -> Result<String, VoiceError> {
    let title = title_of(tokens);
    if title.is_empty() {
        Err(VoiceError::MissingTitle { action })
    } else {
        Ok(title)
    }
}

/// `buat|tambahkan|add ... tugas|task <judul>`. The task noun is required, so an
/// unrelated sentence that merely starts with "add" is not treated as a task.
fn match_create(tokens: &[Token]) -> Option<Result<VoiceCommand, VoiceError>> {
    let rest = strip_one(tokens, CREATE_VERBS)?;
    let rest = strip_one(rest, TASK_NOUNS)?;
    let rest = skip_words(rest, CREATE_SKIP);
    Some(non_empty_title(rest, "buat tugas").map(|title| VoiceCommand::CreateTask { title }))
}

/// `daftar|lihat|tampilkan|show [semua] tugas|tasks`, with nothing after the noun.
fn match_list(tokens: &[Token]) -> Option<Result<VoiceCommand, VoiceError>> {
    let rest = strip_one(tokens, LIST_VERBS)?;
    let rest = skip_words(rest, LIST_SCOPE);
    let rest = strip_one(rest, TASK_NOUNS)?;
    if !rest.is_empty() {
        return None;
    }
    Some(Ok(VoiceCommand::ListTasks))
}

/// `selesaikan <judul>`, `tandai [tugas] [selesai] <judul> [sebagai selesai]`, and the English
/// forms `complete`, `finish`, and `mark ... as done`. Marker words are stripped from both ends
/// of the title, so the title is the words in between.
fn match_complete(tokens: &[Token]) -> Option<Result<VoiceCommand, VoiceError>> {
    let rest = match strip_one(tokens, COMPLETE_VERBS) {
        Some(rest) => rest,
        None => strip_one(tokens, MARK_VERBS)?,
    };
    let rest = skip_words(rest, TASK_NOUNS);
    let rest = skip_words(rest, DONE_WORDS);
    let rest = trim_trailing(rest, DONE_WORDS);
    Some(non_empty_title(rest, "tandai selesai").map(|title| VoiceCommand::CompleteTask { title }))
}

#[cfg(test)]
#[path = "voice_test.rs"]
mod voice_test;
