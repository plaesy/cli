//! Notifikasi keluar ke chat lewat webhook generik (payload JSON).
//!
//! Modul ini murni: membentuk payload, memvalidasi URL dan konfigurasi, membatasi
//! ukuran pesan, dan menyerahkan pengiriman HTTP ke pemanggil lewat trait
//! [`HttpPoster`]. Tidak ada crate HTTP yang dipilih di sini.
//!
//! Header otentikasi hanya menyimpan nama variabel lingkungan. Nilainya dibaca
//! lewat callback saat pengiriman dan tidak pernah masuk ke konfigurasi, log, atau pesan error.
//! Target bersifat opt-in: `enabled` bernilai `false` bila tidak ditulis.

use std::collections::HashSet;
use std::net::{Ipv4Addr, Ipv6Addr};

use serde::Deserialize;
use thiserror::Error;

/// Nilai field `schema` pada setiap payload.
pub const SCHEMA: &str = "plaesy.chat-notification/1";
/// Batas byte untuk field `text` sebelum dipotong.
pub const MAX_TEXT_BYTES: usize = 4096;
/// Batas byte untuk seluruh body JSON yang dikirim.
pub const MAX_PAYLOAD_BYTES: usize = 8192;
/// Batas panjang URL webhook dalam byte.
pub const MAX_URL_BYTES: usize = 2048;

const MAX_NAME_BYTES: usize = 64;
const MAX_ENV_NAME_BYTES: usize = 128;
const ELLIPSIS: &str = "...";
const FORBIDDEN_HEADERS: &[&str] = &[
    "host",
    "content-length",
    "content-type",
    "transfer-encoding",
    "connection",
];

/// Error modul webhook chat.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum ChatWebhookError {
    /// Konfigurasi tidak dapat diparse atau tidak lolos validasi.
    #[error("konfigurasi webhook tidak valid: {0}")]
    Config(String),
    /// URL target ditolak. Alasan berupa teks tetap, tanpa isi URL.
    #[error("URL webhook \"{target}\" ditolak: {reason}")]
    InvalidUrl { target: String, reason: &'static str },
    /// Variabel lingkungan untuk header otentikasi tidak ada atau kosong.
    #[error("variabel lingkungan \"{var}\" untuk target \"{target}\" kosong atau tidak ada")]
    MissingEnv { target: String, var: String },
    /// Nilai variabel lingkungan tidak boleh dipakai sebagai nilai header.
    #[error("nilai dari variabel \"{var}\" tidak valid sebagai nilai header")]
    InvalidHeaderValue { var: String },
    /// Pesan tidak lolos validasi.
    #[error("pesan tidak valid: {0}")]
    InvalidMessage(&'static str),
    /// Payload tetap melebihi batas walaupun teks sudah dipotong.
    #[error("payload melebihi batas ukuran meski sudah dipotong")]
    PayloadTooLarge,
    /// Pemanggil HTTP gagal mengirim permintaan.
    #[error("pengiriman ke \"{target}\" gagal: {reason}")]
    Transport { target: String, reason: String },
    /// Webhook membalas kode status di luar 2xx.
    #[error("webhook \"{target}\" membalas status {status}")]
    Status { target: String, status: u16 },
}

/// Kesalahan dari pemanggil HTTP. Pesannya tidak boleh memuat nilai header.
#[derive(Debug, Error, PartialEq, Eq)]
#[error("{0}")]
pub struct TransportError(pub String);

/// Pemanggil HTTP yang disuntikkan oleh pemanggil modul ini.
///
/// Implementasi wajib tidak mengikuti redirect. Jika tidak, header otentikasi
/// bisa terkirim ke host lain.
pub trait HttpPoster {
    /// Kirim `body` sebagai JSON ke `url` dengan `headers` tambahan. Mengembalikan kode status HTTP.
    fn post_json(
        &self,
        url: &str,
        headers: &[(String, String)],
        body: &[u8],
    ) -> Result<u16, TransportError>;
}

/// Konfigurasi webhook. Field yang tidak dikenal ditolak agar nilai rahasia tidak bisa
/// ditulis sebagai teks biasa.
#[derive(Debug, Clone, Default, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ChatWebhookConfig {
    #[serde(default)]
    pub targets: Vec<ChatWebhookTarget>,
}

/// Satu target webhook.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ChatWebhookTarget {
    pub name: String,
    pub url: String,
    /// Opt-in. Target tanpa `enabled: true` tidak pernah dikirimi pesan.
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub auth: Option<AuthHeader>,
}

/// Header otentikasi. `env` berisi nama variabel lingkungan, bukan nilainya.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct AuthHeader {
    pub header: String,
    pub env: String,
}

/// Pesan notifikasi yang sudah divalidasi.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatMessage {
    event: String,
    title: String,
    body: String,
}

/// Hasil pengiriman ke satu target.
#[derive(Debug, PartialEq, Eq)]
pub struct DeliveryReport {
    pub target: String,
    pub result: Result<(), ChatWebhookError>,
}

impl ChatWebhookConfig {
    /// Parse dan validasi konfigurasi dari teks JSON.
    pub fn from_json(text: &str) -> Result<Self, ChatWebhookError> {
        let config: Self =
            serde_json::from_str(text).map_err(|e| ChatWebhookError::Config(e.to_string()))?;
        config.validate()?;
        Ok(config)
    }

    /// Memvalidasi nama, URL, nama header, dan nama variabel lingkungan setiap target.
    pub fn validate(&self) -> Result<(), ChatWebhookError> {
        let mut seen = HashSet::new();
        for target in &self.targets {
            validate_name(&target.name)?;
            if !seen.insert(target.name.as_str()) {
                return Err(ChatWebhookError::Config(format!(
                    "nama target ganda: {}",
                    target.name
                )));
            }
            check_url(&target.url).map_err(|reason| ChatWebhookError::InvalidUrl {
                target: target.name.clone(),
                reason,
            })?;
            if let Some(auth) = &target.auth {
                validate_header_name(&auth.header)?;
                validate_env_name(&auth.env)?;
            }
        }
        Ok(())
    }
}

impl ChatMessage {
    /// Membuat pesan. `event` hanya boleh berisi huruf kecil, angka, `.`, `_`, dan `-`.
    /// `title` wajib berisi teks dan tidak boleh memuat baris baru.
    pub fn new(
        event: impl Into<String>,
        title: impl Into<String>,
        body: impl Into<String>,
    ) -> Result<Self, ChatWebhookError> {
        let event = event.into();
        let title = title.into();
        let body = body.into();
        let event_ok = !event.is_empty()
            && event.len() <= MAX_NAME_BYTES
            && event.bytes().all(|b| {
                b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b'.' | b'_' | b'-')
            });
        if !event_ok {
            return Err(ChatWebhookError::InvalidMessage("event harus huruf kecil, angka, . _ -"));
        }
        if title.trim().is_empty() {
            return Err(ChatWebhookError::InvalidMessage("judul kosong"));
        }
        if title.contains(['\n', '\r']) {
            return Err(ChatWebhookError::InvalidMessage("judul tidak boleh memuat baris baru"));
        }
        Ok(Self { event, title, body })
    }

    /// Nama event.
    #[must_use]
    pub fn event(&self) -> &str {
        &self.event
    }
}

/// Memeriksa URL webhook: `https://` untuk host mana pun, atau `http://` hanya untuk loopback.
/// Userinfo (`user:pass@`) ditolak.
pub fn validate_url(url: &str) -> Result<(), ChatWebhookError> {
    check_url(url).map_err(|reason| ChatWebhookError::InvalidUrl {
        target: String::new(),
        reason,
    })
}

fn check_url(raw: &str) -> Result<(), &'static str> {
    if raw.is_empty() || raw.len() > MAX_URL_BYTES {
        return Err("panjang URL tidak valid");
    }
    if raw.chars().any(|c| c.is_whitespace() || c.is_control()) {
        return Err("URL mengandung spasi atau karakter kontrol");
    }
    let lower = raw.to_ascii_lowercase();
    let (secure, rest) = if let Some(rest) = lower.strip_prefix("https://") {
        (true, rest)
    } else if let Some(rest) = lower.strip_prefix("http://") {
        (false, rest)
    } else {
        return Err("skema harus https, atau http untuk loopback");
    };
    let authority = rest
        .split(|c: char| c == '/' || c == '?' || c == '#')
        .next()
        .unwrap_or("");
    if authority.is_empty() || authority.contains('@') {
        return Err("host kosong atau memuat userinfo");
    }

    let (host, port, loopback) = if let Some(inner_start) = authority.strip_prefix('[') {
        let close = inner_start.find(']').ok_or("IPv6 tidak ditutup dengan ]")?;
        let host = &authority[..close + 2];
        let addr = inner_start[..close]
            .parse::<Ipv6Addr>()
            .map_err(|_| "host IPv6 tidak valid")?;
        let after = &authority[close + 2..];
        let port = if after.is_empty() {
            None
        } else {
            Some(after.strip_prefix(':').ok_or("format host tidak valid")?)
        };
        (host, port, addr.is_loopback())
    } else {
        let (host, port) = match authority.split_once(':') {
            Some((h, p)) => {
                if p.contains(':') {
                    return Err("format host tidak valid");
                }
                (h, Some(p))
            }
            None => (authority, None),
        };
        if host.is_empty()
            || !host
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'.')
        {
            return Err("nama host tidak valid");
        }
        let loopback = host == "localhost"
            || host
                .parse::<Ipv4Addr>()
                .is_ok_and(|ip| ip.is_loopback());
        (host, port, loopback)
    };

    if let Some(port) = port {
        if port.is_empty() || port.parse::<u16>().is_err() {
            return Err("port tidak valid");
        }
    }
    if host.is_empty() {
        return Err("host kosong");
    }
    if !secure && !loopback {
        return Err("http hanya diizinkan untuk loopback, gunakan https");
    }
    Ok(())
}

fn validate_name(name: &str) -> Result<(), ChatWebhookError> {
    let ok = !name.trim().is_empty()
        && name.len() <= MAX_NAME_BYTES
        && !name.chars().any(char::is_control);
    if ok {
        Ok(())
    } else {
        Err(ChatWebhookError::Config(format!(
            "nama target tidak valid (1-{} byte, tanpa karakter kontrol)",
            MAX_NAME_BYTES
        )))
    }
}

fn validate_header_name(header: &str) -> Result<(), ChatWebhookError> {
    let ok = !header.is_empty()
        && header.len() <= MAX_NAME_BYTES
        && header.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-');
    if !ok {
        return Err(ChatWebhookError::Config(
            "nama header hanya boleh huruf, angka, dan '-'".to_string(),
        ));
    }
    if FORBIDDEN_HEADERS
        .iter()
        .any(|f| f.eq_ignore_ascii_case(header))
    {
        return Err(ChatWebhookError::Config(format!(
            "header \"{header}\" dikelola oleh modul ini dan tidak boleh diatur"
        )));
    }
    Ok(())
}

fn validate_env_name(var: &str) -> Result<(), ChatWebhookError> {
    let mut bytes = var.bytes();
    let first_ok = bytes
        .next()
        .is_some_and(|b| b.is_ascii_alphabetic() || b == b'_');
    let ok = first_ok
        && var.len() <= MAX_ENV_NAME_BYTES
        && bytes.all(|b| b.is_ascii_alphanumeric() || b == b'_');
    if ok {
        Ok(())
    } else {
        Err(ChatWebhookError::Config(
            "nama variabel lingkungan tidak valid".to_string(),
        ))
    }
}

/// Membentuk header permintaan. Header otentikasi dibaca dari `env` dan tidak pernah dicetak.
pub fn resolve_headers(
    target: &ChatWebhookTarget,
    env: &dyn Fn(&str) -> Option<String>,
) -> Result<Vec<(String, String)>, ChatWebhookError> {
    let mut headers = vec![("Content-Type".to_string(), "application/json".to_string())];
    if let Some(auth) = &target.auth {
        let value = env(auth.env.as_str())
            .filter(|v| !v.is_empty())
            .ok_or_else(|| ChatWebhookError::MissingEnv {
                target: target.name.clone(),
                var: auth.env.clone(),
            })?;
        if value.chars().any(char::is_control) {
            return Err(ChatWebhookError::InvalidHeaderValue {
                var: auth.env.clone(),
            });
        }
        headers.push((auth.header.clone(), value));
    }
    Ok(headers)
}

/// Membentuk body JSON. Field `text` dipotong di batas karakter agar seluruh body
/// tidak melebihi [`MAX_PAYLOAD_BYTES`].
pub fn build_payload(target: &str, message: &ChatMessage) -> Result<Vec<u8>, ChatWebhookError> {
    let full = if message.body.is_empty() {
        message.title.clone()
    } else {
        format!("{}\n\n{}", message.title, message.body)
    };
    let mut cap = MAX_TEXT_BYTES;
    loop {
        let text = truncate_text(&full, cap);
        let value = serde_json::json!({
            "schema": SCHEMA,
            "event": message.event,
            "target": target,
            "text": text,
        });
        let bytes = serde_json::to_vec(&value)
            .map_err(|_| ChatWebhookError::InvalidMessage("payload tidak dapat diserialisasi"))?;
        if bytes.len() <= MAX_PAYLOAD_BYTES {
            return Ok(bytes);
        }
        if cap == 0 {
            return Err(ChatWebhookError::PayloadTooLarge);
        }
        let excess = bytes.len() - MAX_PAYLOAD_BYTES;
        cap = cap.saturating_sub(excess.max(1));
    }
}

fn truncate_text(s: &str, max_bytes: usize) -> String {
    if s.len() <= max_bytes {
        return s.to_string();
    }
    let mut end = max_bytes.saturating_sub(ELLIPSIS.len()).min(s.len());
    while !s.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}{}", &s[..end], ELLIPSIS)
}

/// Mengirim satu pesan ke satu target. Target yang tidak aktif tetap dikirim di sini;
/// pemilihan target aktif dilakukan oleh [`dispatch`].
pub fn deliver(
    poster: &dyn HttpPoster,
    target: &ChatWebhookTarget,
    message: &ChatMessage,
    env: &dyn Fn(&str) -> Option<String>,
) -> Result<(), ChatWebhookError> {
    check_url(&target.url).map_err(|reason| ChatWebhookError::InvalidUrl {
        target: target.name.clone(),
        reason,
    })?;
    let headers = resolve_headers(target, env)?;
    let body = build_payload(&target.name, message)?;
    let status = poster
        .post_json(&target.url, &headers, &body)
        .map_err(|e| ChatWebhookError::Transport {
            target: target.name.clone(),
            reason: e.0,
        })?;
    if (200..300).contains(&status) {
        Ok(())
    } else {
        Err(ChatWebhookError::Status {
            target: target.name.clone(),
            status,
        })
    }
}

/// Mengirim pesan ke setiap target yang `enabled`. Target nonaktif dilewati tanpa laporan.
pub fn dispatch(
    config: &ChatWebhookConfig,
    poster: &dyn HttpPoster,
    message: &ChatMessage,
    env: &dyn Fn(&str) -> Option<String>,
) -> Vec<DeliveryReport> {
    config
        .targets
        .iter()
        .filter(|t| t.enabled)
        .map(|t| DeliveryReport {
            target: t.name.clone(),
            result: deliver(poster, t, message, env),
        })
        .collect()
}

#[cfg(test)]
#[path = "chat_webhook_test.rs"]
mod chat_webhook_test;
