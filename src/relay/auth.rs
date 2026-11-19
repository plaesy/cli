//! Authenticated message envelopes using HMAC-SHA256 (spec R6, VS-11 to VS-14).
//!
//! A sealed envelope carries a tag over sender, project, key id, and body. The
//! receiver recomputes the tag and compares in constant time. Keys are shared per
//! deployment in this slice. Key distribution is out of scope.

use sha2::{Digest, Sha256};

const BLOCK: usize = 64;

/// Errors when opening an envelope.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthError {
    /// The envelope does not have the expected shape.
    Malformed,
    /// The tag does not match the content, or the key is wrong.
    BadTag,
}

/// Fields of an authenticated envelope.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Opened {
    pub sender: String,
    pub project: String,
    pub key_id: String,
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

fn signed_text(sender: &str, project: &str, key_id: &str, body: &str) -> String {
    format!("{sender}\n{project}\n{key_id}\n{body}")
}

/// Seals one message. The first line of the result is the tag, then the signed fields.
#[must_use]
pub fn seal(key: &[u8], sender: &str, project: &str, key_id: &str, body: &str) -> String {
    let signed = signed_text(sender, project, key_id, body);
    let tag = to_hex(&hmac_sha256(key, signed.as_bytes()));
    format!("{tag}\n{signed}")
}

/// Verifies and opens a sealed message.
///
/// # Errors
///
/// Returns [`AuthError::Malformed`] for a bad shape and [`AuthError::BadTag`] when the tag does not verify.
pub fn open(key: &[u8], envelope: &str) -> Result<Opened, AuthError> {
    let (tag, signed) = envelope.split_once('\n').ok_or(AuthError::Malformed)?;
    let fields: Vec<&str> = signed.splitn(4, '\n').collect();
    let [sender, project, key_id, body] = fields.as_slice() else {
        return Err(AuthError::Malformed);
    };
    let expected = to_hex(&hmac_sha256(key, signed.as_bytes()));
    if !constant_time_eq(expected.as_bytes(), tag.as_bytes()) {
        return Err(AuthError::BadTag);
    }
    Ok(Opened {
        sender: (*sender).to_owned(),
        project: (*project).to_owned(),
        key_id: (*key_id).to_owned(),
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
        let envelope = seal(key, "agent-1", "project-a", "k1", "hello | world");
        let opened = open(key, &envelope).unwrap();
        assert_eq!(opened.body, "hello | world");
        assert_eq!(opened.project, "project-a");
        let tampered = envelope.replace("project-a", "project-b");
        assert_eq!(open(key, &tampered), Err(AuthError::BadTag));
        assert_eq!(open(b"other-key", &envelope), Err(AuthError::BadTag));
    }
}
