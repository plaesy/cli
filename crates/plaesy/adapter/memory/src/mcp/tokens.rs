//! Per-client bearer tokens for the memory MCP HTTP server.
//!
//! Each label (for example `laptop` or `server`) owns one token of the form `plsy_<id>_<secret>`.
//! The file stores only `sha256(salt || token)`, never the token. `id` is a non-secret lookup key.
//! A rotation keeps the previous token valid until `previous.expires`. Revoking a label disables it
//! without touching other labels.
//!
//! The token file is re-read when its size or mtime changes, so add, rotate, and revoke need no
//! server restart. Once a token file (or the env token) has been seen, authentication stays
//! required: a missing or empty file denies every request. A file that fails to parse keeps the
//! last valid set and logs the error. Log lines carry the label, path, and time, never a token.

use crate::memory::error::{MemoryError, Result};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use uuid::Uuid;

use super::constant_time_eq;

/// Environment variable that overrides the token file location.
pub const TOKENS_FILE_ENV: &str = "PLAESY_MCP_TOKENS_FILE";
/// Label given to the single token from `PLAESY_MCP_TOKEN` or `--auth-token`.
pub const DEFAULT_LABEL: &str = "default";

const TOKEN_PREFIX: &str = "plsy_";
const ID_LEN: usize = 12;
const MAX_LABEL_LEN: usize = 32;
const RELOAD_INTERVAL: Duration = Duration::from_secs(2);
const FAILURE_LOG_INTERVAL: Duration = Duration::from_secs(10);

/// The token file as written to disk.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct TokenFile {
    #[serde(default, rename = "token")]
    pub tokens: Vec<TokenEntry>,
}

/// One labelled client. `id`, `salt`, and `hash` describe the current token.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TokenEntry {
    pub label: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
    pub created: u64,
    pub id: String,
    pub salt: String,
    pub hash: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub previous: Option<PreviousSlot>,
}

/// The token replaced by the last rotation, valid until `expires` (unix seconds).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PreviousSlot {
    pub id: String,
    pub salt: String,
    pub hash: String,
    pub expires: u64,
}

fn default_true() -> bool {
    true
}

/// Outcome of checking one presented bearer token.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Verdict {
    /// The token matched this label.
    Allowed(String),
    /// Authentication is required and the token did not match.
    Denied,
    /// No token source has ever been seen, so the server is open (the compatibility case).
    Open,
}

/// Seconds since the unix epoch; 0 if the clock is before the epoch.
pub fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Lowercase hex encoding.
fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// `sha256(salt || token)` as lowercase hex.
pub fn hash_token(salt: &str, token: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(salt.as_bytes());
    hasher.update(token.as_bytes());
    hex_encode(hasher.finalize().as_slice())
}

/// `count` UUID v4 values as 32 hex characters each. Each contributes 122 random bits.
fn random_hex(count: usize) -> String {
    (0..count)
        .map(|_| Uuid::new_v4().simple().to_string())
        .collect()
}

/// The non-secret id in `plsy_<id>_<secret>`, or `None` if the shape is wrong.
pub fn id_of_token(token: &str) -> Option<&str> {
    let rest = token.strip_prefix(TOKEN_PREFIX)?;
    let (id, secret) = rest.split_once('_')?;
    let well_formed =
        id.len() == ID_LEN && id.bytes().all(|b| b.is_ascii_hexdigit()) && !secret.is_empty();
    well_formed.then_some(id)
}

/// Labels are 1 to 32 characters of `[a-z0-9-]`, and `default` is reserved.
pub fn validate_label(label: &str) -> Result<()> {
    let well_formed = !label.is_empty()
        && label.len() <= MAX_LABEL_LEN
        && label
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-');
    if !well_formed {
        return Err(MemoryError::ConfigError(
            "token label must be 1-32 characters of a-z, 0-9, or '-'".into(),
        ));
    }
    if label == DEFAULT_LABEL {
        return Err(MemoryError::ConfigError(
            "the label 'default' is reserved for PLAESY_MCP_TOKEN".into(),
        ));
    }
    Ok(())
}

/// A freshly generated token and the material stored for it.
struct Issued {
    id: String,
    salt: String,
    hash: String,
    token: String,
}

fn issue() -> Issued {
    let id: String = random_hex(1).chars().take(ID_LEN).collect();
    let token = format!("{TOKEN_PREFIX}{id}_{}", random_hex(2));
    let salt = random_hex(2);
    let hash = hash_token(&salt, &token);
    Issued {
        id,
        salt,
        hash,
        token,
    }
}

/// The in-memory set that authorizes requests. Built from the env token and the file entries.
pub struct TokenSet {
    entries: Vec<TokenEntry>,
    env_token: Option<String>,
}

impl TokenSet {
    pub fn new(env_token: Option<String>, entries: Vec<TokenEntry>) -> Self {
        Self { entries, env_token }
    }

    /// Checks `presented` against every entry without returning early, so the work done does not
    /// reveal which label matched. Only the id is compared directly, because it is not secret.
    pub fn authorize(&self, presented: &str, now: u64) -> Verdict {
        if let Some(env) = &self.env_token {
            if constant_time_eq(presented.as_bytes(), env.as_bytes()) {
                return Verdict::Allowed(DEFAULT_LABEL.to_owned());
            }
        }
        let Some(id) = id_of_token(presented) else {
            return Verdict::Denied;
        };
        let mut matched: Option<String> = None;
        for entry in self.entries.iter().filter(|e| e.enabled) {
            if entry.id == id
                && constant_time_eq(
                    hash_token(&entry.salt, presented).as_bytes(),
                    entry.hash.as_bytes(),
                )
            {
                matched = Some(entry.label.clone());
            }
            if let Some(prev) = entry.previous.as_ref() {
                if prev.expires > now
                    && prev.id == id
                    && constant_time_eq(
                        hash_token(&prev.salt, presented).as_bytes(),
                        prev.hash.as_bytes(),
                    )
                {
                    matched = Some(entry.label.clone());
                }
            }
        }
        matched.map_or(Verdict::Denied, Verdict::Allowed)
    }
}

impl TokenFile {
    fn find_mut(&mut self, label: &str) -> Result<&mut TokenEntry> {
        self.tokens
            .iter_mut()
            .find(|e| e.label == label)
            .ok_or_else(|| MemoryError::ConfigError(format!("no token with label '{label}'")))
    }

    /// Adds a new label and returns its token. The token is not stored anywhere else.
    pub fn add(&mut self, label: &str, now: u64) -> Result<String> {
        validate_label(label)?;
        if self.tokens.iter().any(|e| e.label == label) {
            return Err(MemoryError::ConfigError(format!(
                "token label '{label}' already exists; use rotate or revoke"
            )));
        }
        let issued = issue();
        self.tokens.push(TokenEntry {
            label: label.to_owned(),
            enabled: true,
            created: now,
            id: issued.id,
            salt: issued.salt,
            hash: issued.hash,
            previous: None,
        });
        Ok(issued.token)
    }

    /// Replaces the label's token. The old token stays valid for `grace_secs`. A second rotation
    /// inside that window drops the older token immediately.
    pub fn rotate(&mut self, label: &str, now: u64, grace_secs: u64) -> Result<String> {
        let entry = self.find_mut(label)?;
        if !entry.enabled {
            return Err(MemoryError::ConfigError(format!(
                "token label '{label}' is revoked; add a new label instead"
            )));
        }
        let issued = issue();
        entry.previous = Some(PreviousSlot {
            id: entry.id.clone(),
            salt: entry.salt.clone(),
            hash: entry.hash.clone(),
            expires: now.saturating_add(grace_secs),
        });
        entry.id = issued.id;
        entry.salt = issued.salt;
        entry.hash = issued.hash;
        Ok(issued.token)
    }

    /// Disables the label and drops any previous token. Other labels are not affected.
    pub fn revoke(&mut self, label: &str) -> Result<()> {
        let entry = self.find_mut(label)?;
        entry.enabled = false;
        entry.previous = None;
        Ok(())
    }

    /// One line per label. Contains no hashes, salts, or tokens.
    pub fn summary(&self) -> Vec<String> {
        self.tokens
            .iter()
            .map(|e| {
                let state = if e.enabled { "enabled" } else { "revoked" };
                let previous = e
                    .previous
                    .as_ref()
                    .map(|p| format!("\tprevious_expires={}", p.expires))
                    .unwrap_or_default();
                format!(
                    "{}\t{}\tcreated={}\tid={}{}",
                    e.label, state, e.created, e.id, previous
                )
            })
            .collect()
    }
}

/// Where the token file lives: `PLAESY_MCP_TOKENS_FILE`, else `~/.config/plaesy/mcp-tokens.toml`.
pub fn default_path() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os(TOKENS_FILE_ENV) {
        if !path.is_empty() {
            return Some(PathBuf::from(path));
        }
    }
    dirs::home_dir().map(|home| home.join(".config").join("plaesy").join("mcp-tokens.toml"))
}

fn absolute(path: &Path) -> PathBuf {
    std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf())
}

/// Refuses a token file inside the vault, because the vault is synced to other machines.
pub fn ensure_outside(path: &Path, watch_dir: &Path) -> Result<()> {
    if absolute(path).starts_with(absolute(watch_dir)) {
        return Err(MemoryError::ConfigError(
            "the MCP token file must not live inside the synced vault".into(),
        ));
    }
    Ok(())
}

/// Reads the token file. `Ok(None)` means the file does not exist.
pub fn load(path: &Path) -> Result<Option<TokenFile>> {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(source) => {
            return Err(MemoryError::IoError {
                path: path.display().to_string(),
                source,
            })
        }
    };
    // The parser message can quote file content, so it is not included.
    let file = toml::from_str::<TokenFile>(&text).map_err(|_| {
        MemoryError::ConfigError(format!("invalid MCP token file {}", path.display()))
    })?;
    warn_if_open(path);
    Ok(Some(file))
}

#[cfg(unix)]
fn warn_if_open(path: &Path) {
    use std::os::unix::fs::PermissionsExt;
    if let Ok(meta) = std::fs::metadata(path) {
        if meta.permissions().mode() & 0o077 != 0 {
            tracing::warn!(
                "MCP token file {} is readable by group or others; chmod 600 it",
                path.display()
            );
        }
    }
}

#[cfg(not(unix))]
fn warn_if_open(_path: &Path) {}

/// Writes the file atomically: a temporary file in the same folder, then a rename.
/// On Unix the temporary file is created with mode 0600.
pub fn save(path: &Path, file: &TokenFile) -> Result<()> {
    let text = toml::to_string_pretty(file)
        .map_err(|_| MemoryError::ConfigError("could not encode the MCP token file".into()))?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|source| MemoryError::IoError {
            path: parent.display().to_string(),
            source,
        })?;
    }
    let tmp = path.with_extension(format!("tmp-{}", std::process::id()));
    write_private(&tmp, text.as_bytes()).map_err(|source| MemoryError::IoError {
        path: tmp.display().to_string(),
        source,
    })?;
    std::fs::rename(&tmp, path).map_err(|source| MemoryError::IoError {
        path: path.display().to_string(),
        source,
    })
}

fn write_private(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    use std::io::Write;
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(path)?;
    file.write_all(bytes)?;
    file.sync_all()
}

/// One audit line: the event, label, path, time, and failures suppressed since the last line.
/// It never contains a token or hash.
pub fn audit_line(
    event: &str,
    label: Option<&str>,
    path: Option<&Path>,
    at: u64,
    failures: u64,
) -> String {
    let mut line = format!("event={event} at={at}");
    if let Some(label) = label {
        line.push_str(&format!(" label={label}"));
    }
    if let Some(path) = path {
        line.push_str(&format!(" path={}", path.display()));
    }
    if failures > 0 {
        line.push_str(&format!(" failures_since_last_log={failures}"));
    }
    line
}

/// Mutable part of [`TokenAuth`], guarded by one lock.
struct AuthState {
    set: TokenSet,
    /// Set once any token source has been seen. From then on authentication is mandatory.
    required: bool,
    /// (mtime, size) of the file at the last load attempt.
    stamp: Option<(Option<SystemTime>, u64)>,
    last_check: Option<Instant>,
    last_failure_log: Option<Instant>,
    failures_since_log: u64,
}

/// Bearer authentication with hot reload, shared by every request.
pub struct TokenAuth {
    path: Option<PathBuf>,
    env_token: Option<String>,
    state: Mutex<AuthState>,
}

impl TokenAuth {
    /// Returns `None` when there is neither an env token nor a token file path, which keeps the
    /// server open as before. The file location is refused if it is inside `watch_dir`.
    pub fn new(
        env_token: Option<String>,
        path: Option<PathBuf>,
        watch_dir: &Path,
    ) -> Result<Option<Arc<Self>>> {
        if env_token.is_none() && path.is_none() {
            return Ok(None);
        }
        if let Some(path) = &path {
            ensure_outside(path, watch_dir)?;
        }
        let auth = Self {
            state: Mutex::new(AuthState {
                set: TokenSet::new(env_token.clone(), Vec::new()),
                required: env_token.is_some(),
                stamp: None,
                last_check: None,
                last_failure_log: None,
                failures_since_log: 0,
            }),
            path,
            env_token,
        };
        {
            let mut state = auth.state.lock();
            auth.refresh(&mut state, Instant::now());
        }
        Ok(Some(Arc::new(auth)))
    }

    /// Checks a presented bearer value now.
    pub fn check(&self, presented: Option<&str>) -> Verdict {
        self.check_at(presented, now_unix(), Instant::now())
    }

    /// Same as [`check`](Self::check), with the clock passed in for tests.
    pub fn check_at(&self, presented: Option<&str>, now_unix: u64, now: Instant) -> Verdict {
        let mut state = self.state.lock();
        self.refresh(&mut state, now);
        if !state.required {
            return Verdict::Open;
        }
        let verdict = match presented {
            Some(token) => state.set.authorize(token, now_unix),
            None => Verdict::Denied,
        };
        match &verdict {
            Verdict::Allowed(label) => {
                tracing::info!(
                    "{}",
                    audit_line(
                        "auth_ok",
                        Some(label.as_str()),
                        self.path.as_deref(),
                        now_unix,
                        0
                    )
                );
            }
            Verdict::Denied => {
                state.failures_since_log += 1;
                let due = state
                    .last_failure_log
                    .is_none_or(|last| now.duration_since(last) >= FAILURE_LOG_INTERVAL);
                if due {
                    let failures = std::mem::take(&mut state.failures_since_log);
                    state.last_failure_log = Some(now);
                    tracing::warn!(
                        "{}",
                        audit_line(
                            "auth_failed",
                            None,
                            self.path.as_deref(),
                            now_unix,
                            failures
                        )
                    );
                }
            }
            Verdict::Open => {}
        }
        verdict
    }

    /// Reloads the token file when its stamp changed. Checks at most once per `RELOAD_INTERVAL`.
    fn refresh(&self, state: &mut AuthState, now: Instant) {
        if state
            .last_check
            .is_some_and(|last| now.duration_since(last) < RELOAD_INTERVAL)
        {
            return;
        }
        state.last_check = Some(now);
        let Some(path) = self.path.as_deref() else {
            return;
        };
        match std::fs::metadata(path) {
            Ok(meta) => {
                let stamp = (meta.modified().ok(), meta.len());
                if state.stamp == Some(stamp) {
                    return;
                }
                // Record the stamp even on failure, so a broken file is reported once per change.
                state.stamp = Some(stamp);
                match load(path) {
                    Ok(Some(file)) => {
                        state.set = TokenSet::new(self.env_token.clone(), file.tokens);
                        state.required = true;
                        tracing::info!(
                            "{}",
                            audit_line("token_file_loaded", None, Some(path), now_unix(), 0)
                        );
                    }
                    Ok(None) => {}
                    Err(error) => {
                        tracing::error!(
                            "MCP token file rejected; keeping the last valid set: {error}"
                        );
                    }
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                if state.stamp.take().is_some() {
                    // Fail closed: the file is gone, so only the env token (if any) still works.
                    state.set = TokenSet::new(self.env_token.clone(), Vec::new());
                    tracing::warn!(
                        "MCP token file {} is gone; only PLAESY_MCP_TOKEN is accepted",
                        path.display()
                    );
                }
            }
            Err(error) => {
                tracing::error!("MCP token file unreadable; keeping the last valid set: {error}");
            }
        }
    }
}

#[cfg(test)]
#[path = "tokens_test.rs"]
mod tokens_test;
