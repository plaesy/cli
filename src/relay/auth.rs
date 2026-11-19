//! Authenticated message envelopes using HMAC-SHA256 (spec R6, VS-11 to VS-14).
//!
//! Each envelope carries a key id, a send time, and a random nonce. The tag covers
//! all of them, so changing any field breaks the tag. A [`ReplayGuard`] on the
//! receiver rejects reused nonces and envelopes outside a time window. Keys come
//! from a [`super::keys::KeySource`]. They never appear in envelopes.

use sha2::{Digest, Sha256};
use std::collections::HashMap;

const BLOCK: usize = 64;

/// Errors when opening or accepting an envelope.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthError {
    /// The envelope does not have the expected shape.
    Malformed,
    /// The tag does not match the content, or the key is wrong.
    BadTag,
    /// The send time is outside the accepted window.
    Expired,
    /// The nonce was already seen in the window.
    Replayed,
}

/// Fields of a verified envelope.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Opened {
    pub sender: String,
    pub project: String,
    pub key_id: String,
    pub sent_at: u64,
    pub nonce: String,
    pub body: String,
}

/// HMAC-SHA256 as defined in RFC 2104.
#[must_use]
pub fn hmac_sha256(key: &[u8], message: &[u8]) -> [u8; 32] {
    let mut key_block = [0u8; BLOCK];
    if key.len() > BLOCK {
        key_block[..32].copy_from_slice(&Sha256::digest(key));
    } else {
        key_block[..key.len()].copy_from_slice(key);
    }
    let mut ipad = [0x36u8; BLOCK];
    let mut opad = [0x5cu8; BLOCK];
    for index in 0..BLOCK {
        ipad[index] ^= key_block[index];
        opad[index] ^= key_block[index];
    }
    let mut inner = Sha256::new();
    inner.update(ipad);
    inner.update(message);
    let inner_hash = inner.finalize();
    let mut outer = Sha256::new();
    outer.update(opad);
    outer.update(inner_hash);
    outer.finalize().into()
}

fn to_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn signed_text(
    sender: &str,
    project: &str,
    key_id: &str,
    sent_at: u64,
    nonce: &str,
    body: &str,
) -> String {
    format!("{sender}\n{project}\n{key_id}\n{sent_at}\n{nonce}\n{body}")
}

/// Seals one message. The first line of the result is the tag, then the signed fields.
#[must_use]
pub fn seal(
    key: &[u8],
    key_id: &str,
    sender: &str,
    project: &str,
    sent_at: u64,
    nonce: &str,
    body: &str,
) -> String {
    let signed = signed_text(sender, project, key_id, sent_at, nonce, body);
    let tag = to_hex(&hmac_sha256(key, signed.as_bytes()));
    format!("{tag}\n{signed}")
}

/// Verifies the tag and returns the fields. Replay and time checks are separate, in [`ReplayGuard`].
///
/// # Errors
///
/// Returns [`AuthError::Malformed`] for a bad shape and [`AuthError::BadTag`] when the tag does not verify.
pub fn open(key: &[u8], envelope: &str) -> Result<Opened, AuthError> {
    let (tag, signed) = envelope.split_once('\n').ok_or(AuthError::Malformed)?;
    let fields: Vec<&str> = signed.splitn(6, '\n').collect();
    let [sender, project, key_id, sent_at, nonce, body] = fields.as_slice() else {
        return Err(AuthError::Malformed);
    };
    let sent_at: u64 = sent_at.parse().map_err(|_| AuthError::Malformed)?;
    let expected = to_hex(&hmac_sha256(key, signed.as_bytes()));
    if !constant_time_eq(expected.as_bytes(), tag.as_bytes()) {
        return Err(AuthError::BadTag);
    }
    Ok(Opened {
        sender: (*sender).to_owned(),
        project: (*project).to_owned(),
        key_id: (*key_id).to_owned(),
        sent_at,
        nonce: (*nonce).to_owned(),
        body: (*body).to_owned(),
    })
}

fn constant_time_eq(left: &[u8], right: &[u8]) -> bool {
    if left.len() != right.len() {
        return false;
    }
    left.iter()
        .zip(right)
        .fold(0u8, |acc, (a, b)| acc | (a ^ b))
        == 0
}

/// Receiver-side replay and freshness check. Remembers nonces for one window.
#[derive(Debug)]
pub struct ReplayGuard {
    window_secs: u64,
    seen: HashMap<String, u64>,
}

impl ReplayGuard {
    /// Creates a guard. Envelopes older or newer than `window_secs` are refused.
    #[must_use]
    pub fn new(window_secs: u64) -> Self {
        Self {
            window_secs,
            seen: HashMap::new(),
        }
    }

    /// Accepts an envelope once. Forgets nonces that have left the window.
    ///
    /// # Errors
    ///
    /// Returns [`AuthError::Expired`] outside the window and [`AuthError::Replayed`] for a reused nonce.
    pub fn accept(&mut self, opened: &Opened, now: u64) -> Result<(), AuthError> {
        let window = self.window_secs;
        self.seen
            .retain(|_, sent_at| now.abs_diff(*sent_at) <= window);
        if now.abs_diff(opened.sent_at) > window {
            return Err(AuthError::Expired);
        }
        if self.seen.contains_key(&opened.nonce) {
            return Err(AuthError::Replayed);
        }
        self.seen.insert(opened.nonce.clone(), opened.sent_at);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shared_messaging_hmac_matches_rfc4231_vector() {
        let key = [0x0bu8; 20];
        let tag = to_hex(&hmac_sha256(&key, b"Hi There"));
        assert_eq!(
            tag,
            "b0344c61d8db38535ca8afceaf0bf12b881dc200c9833da726e9376c2e32cff7"
        );
    }

    #[test]
    fn shared_messaging_envelope_round_trips_and_rejects_tampering() {
        let key = b"deployment-key";
        let envelope = seal(
            key,
            "k1",
            "agent-1",
            "project-a",
            1000,
            "n-1",
            "hello | world",
        );
        let opened = open(key, &envelope).unwrap();
        assert_eq!(opened.body, "hello | world");
        assert_eq!(opened.project, "project-a");
        let tampered = envelope.replace("project-a", "project-b");
        assert_eq!(open(key, &tampered), Err(AuthError::BadTag));
        assert_eq!(open(b"other-key", &envelope), Err(AuthError::BadTag));
    }

    #[test]
    fn shared_messaging_replayed_nonce_is_rejected() {
        let key = b"deployment-key";
        let envelope = seal(key, "k1", "agent-1", "project-a", 1000, "n-1", "hi");
        let opened = open(key, &envelope).unwrap();
        let mut guard = ReplayGuard::new(300);
        assert_eq!(guard.accept(&opened, 1000), Ok(()));
        assert_eq!(guard.accept(&opened, 1001), Err(AuthError::Replayed));
    }

    #[test]
    fn shared_messaging_stale_envelope_is_expired() {
        let key = b"deployment-key";
        let envelope = seal(key, "k1", "agent-1", "project-a", 1000, "n-2", "hi");
        let opened = open(key, &envelope).unwrap();
        let mut guard = ReplayGuard::new(300);
        assert_eq!(guard.accept(&opened, 1000 + 301), Err(AuthError::Expired));
    }
}
