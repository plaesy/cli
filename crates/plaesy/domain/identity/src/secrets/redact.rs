//! Redaksi pola rahasia pada teks keluaran (token API, kunci, kata sandi).
//!
//! [`super::SecretStore::redact`] hanya mengganti nilai yang sudah tersimpan. Modul ini
//! melengkapinya dengan pola: nilai `password=`, `token:`, dan `api_key` (termasuk di query
//! URL, header, dan JSON), kata sandi di userinfo URL, header `Authorization`, token `Bearer`,
//! serta format token penyedia yang dikenal.
//!
//! Pencocokan dilakukan tanpa regex agar domain tetap bebas dependensi tambahan. Setiap pola
//! adalah implementasi [`RedactionRule`]; pola baru ditambahkan lewat [`Redactor::push`].
//! Penanda pengganti berbentuk `[REDACTED:<jenis>]`, sama dengan [`super::SecretStore`].

use std::ops::Range;

/// Rentang byte pada satu baris teks yang harus disamarkan, beserta jenisnya.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RedactionSpan {
    /// Rentang byte di dalam baris. Harus berada di batas karakter UTF-8.
    pub range: Range<usize>,
    /// Jenis rahasia; muncul di penanda `[REDACTED:<jenis>]`.
    pub kind: String,
}

/// Pola redaksi. Implementasi mengembalikan rentang yang harus disamarkan pada satu baris.
pub trait RedactionRule: Send + Sync {
    /// Menemukan rentang rahasia pada `line`. Baris berisi satu segmen (boleh diakhiri `\n`).
    fn find(&self, line: &str) -> Vec<RedactionSpan>;
}

/// Pola token penyedia: awalan tetap diikuti deret karakter token minimal `min_tail`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TokenPattern {
    kind: String,
    prefix: String,
    min_tail: usize,
}

impl TokenPattern {
    /// Membuat pola token dengan jenis, awalan, dan panjang ekor minimal.
    #[must_use]
    pub fn new(kind: &str, prefix: &str, min_tail: usize) -> Self {
        Self {
            kind: kind.to_owned(),
            prefix: prefix.to_owned(),
            min_tail,
        }
    }
}

/// Kunci yang nilainya disamarkan saat muncul sebagai `kunci=nilai` atau `kunci: nilai`.
pub const DEFAULT_SECRET_KEYS: &[&str] = &[
    "password",
    "passwd",
    "secret",
    "client_secret",
    "client-secret",
    "api_key",
    "api-key",
    "apikey",
    "access_token",
    "access-token",
    "auth_token",
    "refresh_token",
    "token",
];

/// Kumpulan pola token penyedia bawaan. Urutan penting: pola yang lebih spesifik di depan.
#[must_use]
pub fn default_token_patterns() -> Vec<TokenPattern> {
    [
        ("anthropic_api_key", "sk-ant-", 20),
        ("openai_api_key", "sk-", 32),
        ("github_token", "ghp_", 20),
        ("github_token", "gho_", 20),
        ("github_token", "ghu_", 20),
        ("github_token", "ghs_", 20),
        ("github_token", "ghr_", 20),
        ("github_token", "github_pat_", 20),
        ("slack_token", "xoxb-", 20),
        ("slack_token", "xoxp-", 20),
        ("slack_token", "xoxa-", 20),
        ("slack_token", "xoxr-", 20),
        ("slack_token", "xoxs-", 20),
        ("stripe_secret_key", "sk_live_", 16),
        ("stripe_secret_key", "sk_test_", 16),
        ("stripe_secret_key", "rk_live_", 16),
        ("stripe_secret_key", "rk_test_", 16),
        ("google_api_key", "AIza", 35),
        ("aws_access_key_id", "AKIA", 16),
    ]
    .into_iter()
    .map(|(kind, prefix, min_tail)| TokenPattern::new(kind, prefix, min_tail))
    .collect()
}

/// Kumpulan pola yang dijalankan berurutan atas setiap baris teks.
#[derive(Default)]
pub struct Redactor {
    rules: Vec<Box<dyn RedactionRule>>,
}

impl Redactor {
    /// Membuat redactor kosong tanpa pola.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Membuat redactor dengan semua pola bawaan.
    #[must_use]
    pub fn with_defaults() -> Self {
        let mut redactor = Self::new();
        redactor.push(Box::new(AuthorizationHeaderRule));
        redactor.push(Box::new(BearerTokenRule));
        redactor.push(Box::new(UrlPasswordRule));
        redactor.push(Box::new(KeyValueRule::new(DEFAULT_SECRET_KEYS)));
        redactor.push(Box::new(ProviderTokenRule::new(default_token_patterns())));
        redactor
    }

    /// Menambahkan pola di akhir daftar.
    pub fn push(&mut self, rule: Box<dyn RedactionRule>) {
        self.rules.push(rule);
    }

    /// Menyamarkan semua rahasia yang dikenali pola pada `text`. Teks tanpa rahasia tidak berubah.
    /// Baris dipisah di `\n`, dan pemisah baris dipertahankan.
    #[must_use]
    pub fn redact(&self, text: &str) -> String {
        text.split_inclusive('\n')
            .map(|segment| self.redact_segment(segment))
            .collect()
    }

    fn redact_segment(&self, segment: &str) -> String {
        let mut spans: Vec<RedactionSpan> = self
            .rules
            .iter()
            .flat_map(|rule| rule.find(segment))
            .filter(|span| is_valid_span(segment, &span.range))
            .collect();
        if spans.is_empty() {
            return segment.to_owned();
        }
        // Urutkan berdasarkan awal, lalu rentang terpanjang. Rentang yang tumpang tindih
        // digabung agar tidak ada sisa rahasia yang terbuka.
        spans.sort_by_key(|span| (span.range.start, std::cmp::Reverse(span.range.end)));
        let mut out = String::with_capacity(segment.len());
        let mut cursor = 0usize;
        for span in spans {
            if span.range.start < cursor {
                if span.range.end > cursor {
                    cursor = span.range.end;
                }
                continue;
            }
            out.push_str(&segment[cursor..span.range.start]);
            out.push_str("[REDACTED:");
            out.push_str(&span.kind);
            out.push(']');
            cursor = span.range.end;
        }
        out.push_str(&segment[cursor..]);
        out
    }
}

/// Redaksi dengan semua pola bawaan. Untuk pemakaian berulang, simpan [`Redactor`] dan panggil
/// [`Redactor::redact`] agar pola tidak dibangun ulang.
#[must_use]
pub fn redact_text(text: &str) -> String {
    Redactor::with_defaults().redact(text)
}

fn is_valid_span(segment: &str, range: &Range<usize>) -> bool {
    range.start < range.end
        && range.end <= segment.len()
        && segment.is_char_boundary(range.start)
        && segment.is_char_boundary(range.end)
}

/// Batas kata di depan `index`: awal baris atau karakter sebelumnya bukan huruf/angka ASCII.
fn starts_token_at(line: &str, index: usize) -> bool {
    index == 0 || !line.as_bytes()[index - 1].is_ascii_alphanumeric()
}

/// Nilai placeholder atau nilai kosong tidak dianggap rahasia.
fn is_placeholder(value: &str) -> bool {
    let value = value.trim();
    if value.is_empty() {
        return true;
    }
    if value.starts_with(['<', '$', '{', '[']) {
        return true;
    }
    if value.chars().all(|c| c == '*' || c == 'x' || c == 'X') {
        return true;
    }
    matches!(
        value.to_ascii_lowercase().as_str(),
        "redacted"
            | "placeholder"
            | "example"
            | "none"
            | "null"
            | "changeme"
            | "change_me"
            | "todo"
            | "dummy"
    )
}

/// Melewati pemisah `=` atau `:` setelah kunci (dan tanda kutip penutup kunci JSON).
/// Mengembalikan indeks awal nilai, atau `None` jika bukan penugasan.
fn assignment_value_start(line: &str, after_key: usize) -> Option<usize> {
    let bytes = line.as_bytes();
    let mut i = after_key;
    while i < bytes.len() && matches!(bytes[i], b'"' | b'\'' | b'`') {
        i += 1;
    }
    while i < bytes.len() && matches!(bytes[i], b' ' | b'\t') {
        i += 1;
    }
    if i < bytes.len() && matches!(bytes[i], b'=' | b':') {
        i += 1;
    } else {
        return None;
    }
    while i < bytes.len() && matches!(bytes[i], b' ' | b'\t') {
        i += 1;
    }
    Some(i)
}

/// Mengambil rentang nilai mulai dari `start`. Nilai berkutip berakhir di kutip penutup;
/// nilai tanpa kutip berakhir di spasi atau pembatas umum. Placeholder dilewati.
fn scan_value(line: &str, start: usize) -> Option<Range<usize>> {
    let quote = line
        .as_bytes()
        .get(start)
        .copied()
        .filter(|b| matches!(*b, b'"' | b'\'' | b'`'));
    let value_start = if quote.is_some() { start + 1 } else { start };
    let rest = line.get(value_start..)?;
    let len = match quote {
        Some(q) => rest.find(q as char).unwrap_or(rest.len()),
        None => rest
            .find(|c: char| {
                c.is_whitespace()
                    || matches!(
                        c,
                        '&' | ';' | ',' | '"' | '\'' | '`' | ')' | '}' | ']' | '<' | '>'
                    )
            })
            .unwrap_or(rest.len()),
    };
    let end = value_start + len;
    if is_placeholder(line.get(value_start..end)?) {
        return None;
    }
    Some(value_start..end)
}

/// Kunci dalam query URL, header, JSON, atau format `kunci=nilai`. Nama kunci tidak peka huruf.
#[derive(Clone, Debug)]
pub struct KeyValueRule {
    keys: Vec<String>,
}

impl KeyValueRule {
    /// Membuat aturan untuk daftar kunci. Kunci kosong diabaikan.
    #[must_use]
    pub fn new(keys: &[&str]) -> Self {
        Self {
            keys: keys
                .iter()
                .filter(|key| !key.is_empty())
                .map(|key| key.to_ascii_lowercase())
                .collect(),
        }
    }
}

impl RedactionRule for KeyValueRule {
    fn find(&self, line: &str) -> Vec<RedactionSpan> {
        let lower = line.to_ascii_lowercase();
        let mut spans = Vec::new();
        for key in &self.keys {
            for (start, _) in lower.match_indices(key.as_str()) {
                if !starts_token_at(line, start) {
                    continue;
                }
                let Some(value_start) = assignment_value_start(line, start + key.len()) else {
                    continue;
                };
                if let Some(range) = scan_value(line, value_start) {
                    spans.push(RedactionSpan {
                        range,
                        kind: key.clone(),
                    });
                }
            }
        }
        spans
    }
}

/// Nilai header `Authorization`, dengan atau tanpa skema `Bearer`, `Basic`, `Token`, `Digest`.
#[derive(Clone, Copy, Debug)]
pub struct AuthorizationHeaderRule;

impl RedactionRule for AuthorizationHeaderRule {
    fn find(&self, line: &str) -> Vec<RedactionSpan> {
        const NAME: &str = "authorization";
        const SCHEMES: &[&str] = &["bearer", "basic", "token", "digest"];
        let lower = line.to_ascii_lowercase();
        let mut spans = Vec::new();
        for (start, _) in lower.match_indices(NAME) {
            if !starts_token_at(line, start) {
                continue;
            }
            let Some(mut index) = assignment_value_start(line, start + NAME.len()) else {
                continue;
            };
            for scheme in SCHEMES {
                let Some(rest) = lower.get(index..) else {
                    break;
                };
                let followed_by_space = rest
                    .get(scheme.len()..)
                    .is_some_and(|after| after.starts_with([' ', '\t']));
                if rest.starts_with(*scheme) && followed_by_space {
                    index += scheme.len();
                    while line
                        .as_bytes()
                        .get(index)
                        .copied()
                        .is_some_and(|b| matches!(b, b' ' | b'\t'))
                    {
                        index += 1;
                    }
                    break;
                }
            }
            if let Some(range) = scan_value(line, index) {
                spans.push(RedactionSpan {
                    range,
                    kind: "authorization".to_owned(),
                });
            }
        }
        spans
    }
}

/// Token `Bearer` berdiri sendiri dalam teks. Panjang minimal mencegah kata biasa ikut tersamar.
#[derive(Clone, Copy, Debug)]
pub struct BearerTokenRule;

impl RedactionRule for BearerTokenRule {
    fn find(&self, line: &str) -> Vec<RedactionSpan> {
        const MIN_LEN: usize = 20;
        let lower = line.to_ascii_lowercase();
        let mut spans = Vec::new();
        for (start, _) in lower.match_indices("bearer ") {
            if !starts_token_at(line, start) {
                continue;
            }
            let value_start = start + "bearer ".len();
            let len = line
                .as_bytes()
                .get(value_start..)
                .unwrap_or(&[])
                .iter()
                .take_while(|&&b| {
                    b.is_ascii_alphanumeric()
                        || matches!(b, b'-' | b'.' | b'_' | b'~' | b'+' | b'/' | b'=')
                })
                .count();
            if len >= MIN_LEN {
                spans.push(RedactionSpan {
                    range: value_start..value_start + len,
                    kind: "bearer_token".to_owned(),
                });
            }
        }
        spans
    }
}

/// Kata sandi di userinfo URL, misalnya `postgres://user:kata-sandi@host/db`.
#[derive(Clone, Copy, Debug)]
pub struct UrlPasswordRule;

impl RedactionRule for UrlPasswordRule {
    fn find(&self, line: &str) -> Vec<RedactionSpan> {
        let mut spans = Vec::new();
        for (index, _) in line.match_indices("://") {
            let authority_start = index + "://".len();
            let authority_end = line
                .get(authority_start..)
                .and_then(|rest| {
                    rest.find(|c: char| {
                        c.is_whitespace()
                            || matches!(c, '/' | '?' | '#' | '"' | '\'' | '`' | '<' | '>')
                    })
                })
                .map_or(line.len(), |offset| authority_start + offset);
            let Some(authority) = line.get(authority_start..authority_end) else {
                continue;
            };
            let Some(at) = authority.rfind('@') else {
                continue;
            };
            let userinfo = &authority[..at];
            let Some(colon) = userinfo.find(':') else {
                continue;
            };
            let password_start = authority_start + colon + 1;
            let password_end = authority_start + at;
            if password_end > password_start && !is_placeholder(&line[password_start..password_end])
            {
                spans.push(RedactionSpan {
                    range: password_start..password_end,
                    kind: "url_password".to_owned(),
                });
            }
        }
        spans
    }
}

/// Token penyedia berdasarkan awalan tetap, misalnya `ghp_` atau `sk-ant-`.
#[derive(Clone, Debug)]
pub struct ProviderTokenRule {
    patterns: Vec<TokenPattern>,
}

impl ProviderTokenRule {
    /// Membuat aturan dari daftar pola. Urutan menentukan jenis bila dua pola cocok.
    #[must_use]
    pub fn new(patterns: Vec<TokenPattern>) -> Self {
        Self { patterns }
    }
}

impl RedactionRule for ProviderTokenRule {
    fn find(&self, line: &str) -> Vec<RedactionSpan> {
        let mut spans = Vec::new();
        for pattern in &self.patterns {
            for (start, _) in line.match_indices(pattern.prefix.as_str()) {
                if !starts_token_at(line, start) {
                    continue;
                }
                let tail_start = start + pattern.prefix.len();
                let tail = line
                    .as_bytes()
                    .get(tail_start..)
                    .unwrap_or(&[])
                    .iter()
                    .take_while(|&&b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-'))
                    .count();
                if tail >= pattern.min_tail {
                    spans.push(RedactionSpan {
                        range: start..tail_start + tail,
                        kind: pattern.kind.clone(),
                    });
                }
            }
        }
        spans
    }
}

#[cfg(test)]
#[path = "redact_test.rs"]
mod redact_test;
