//! Local users, their roles, and API key authentication (single organization).
//!
//! API keys are never stored as plain text. A user keeps a random salt and the
//! SHA-256 digest of `salt || key`. The raw key exists only in the caller during
//! creation and verification. Keys are expected to be high-entropy random strings,
//! so a single salted SHA-256 is enough; a slow password KDF is not needed here.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

/// Shortest raw API key accepted at creation time.
pub const MIN_KEY_BYTES: usize = 32;

/// Stable identifier of a local user.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct UserId(String);

impl UserId {
    /// Wraps a user identifier.
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// Returns the identifier text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Role of a user. Later variants include the permissions of earlier ones.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    /// Read-only access.
    Viewer,
    /// May submit and run tasks.
    Operator,
    /// Full access, including user management.
    Admin,
}

impl Role {
    /// Stable lowercase name, the same text used in JSON.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Viewer => "viewer",
            Self::Operator => "operator",
            Self::Admin => "admin",
        }
    }
}

/// Salted digest of an API key. Holds no raw key material.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeyRecord {
    salt: String,
    digest: String,
}

/// One local user.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct User {
    id: UserId,
    display_name: String,
    role: Role,
    key: KeyRecord,
}

impl User {
    /// Returns the user identifier.
    #[must_use]
    pub fn id(&self) -> &UserId {
        &self.id
    }

    /// Returns the display name.
    #[must_use]
    pub fn display_name(&self) -> &str {
        &self.display_name
    }

    /// Returns the role.
    #[must_use]
    pub fn role(&self) -> Role {
        self.role
    }
}

/// Errors from user management and authentication.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UserError {
    /// A user with this identifier already exists.
    Duplicate(UserId),
    /// The input does not meet the rules; the reason names the rule.
    Invalid(&'static str),
    /// Unknown user or wrong key. The two cases are not distinguished.
    Unauthenticated,
    /// The user's role is below the role an action requires.
    InsufficientRole { required: Role, actual: Role },
}

impl std::fmt::Display for UserError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Duplicate(id) => write!(f, "user {} already exists", id.as_str()),
            Self::Invalid(reason) => write!(f, "invalid user: {reason}"),
            Self::Unauthenticated => write!(f, "authentication failed"),
            Self::InsufficientRole { required, actual } => write!(
                f,
                "role {} is not enough, {} is required",
                actual.as_str(),
                required.as_str()
            ),
        }
    }
}

impl std::error::Error for UserError {}

/// Checks that `user` holds at least `required`.
///
/// # Errors
///
/// Returns [`UserError::InsufficientRole`] when the user's role is lower.
pub fn require_role(user: &User, required: Role) -> Result<(), UserError> {
    if user.role >= required {
        Ok(())
    } else {
        Err(UserError::InsufficientRole {
            required,
            actual: user.role,
        })
    }
}

/// Holds local users keyed by identifier.
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct UserStore {
    users: BTreeMap<UserId, User>,
}

impl UserStore {
    /// Creates an empty store.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a user. The caller supplies the raw key and the salt, so this function
    /// does not use randomness. The raw key is hashed and not kept.
    ///
    /// `salt` must be a non-empty hex string and should be random per user.
    ///
    /// # Errors
    ///
    /// Returns [`UserError::Invalid`] for an empty id or name, a short key, or a
    /// malformed salt, and [`UserError::Duplicate`] when the id already exists.
    pub fn create(
        &mut self,
        id: UserId,
        display_name: &str,
        role: Role,
        raw_key: &str,
        salt: &str,
    ) -> Result<(), UserError> {
        if id.as_str().trim().is_empty() {
            return Err(UserError::Invalid("id is empty"));
        }
        if display_name.trim().is_empty() {
            return Err(UserError::Invalid("display name is empty"));
        }
        if raw_key.len() < MIN_KEY_BYTES {
            return Err(UserError::Invalid("key is shorter than 32 bytes"));
        }
        if salt.is_empty() || !salt.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(UserError::Invalid("salt must be a non-empty hex string"));
        }
        if self.users.contains_key(&id) {
            return Err(UserError::Duplicate(id));
        }
        let digest = digest_hex(salt, raw_key);
        let user = User {
            id: id.clone(),
            display_name: display_name.trim().to_owned(),
            role,
            key: KeyRecord {
                salt: salt.to_owned(),
                digest,
            },
        };
        self.users.insert(id, user);
        Ok(())
    }

    /// Verifies the presented key for a user and returns the user on success.
    ///
    /// # Errors
    ///
    /// Returns [`UserError::Unauthenticated`] for an unknown user or a wrong key.
    pub fn authenticate(&self, id: &UserId, presented_key: &str) -> Result<&User, UserError> {
        let Some(user) = self.users.get(id) else {
            // Hash anyway so an unknown user costs about the same time as a known one.
            let _ = digest_hex("", presented_key);
            return Err(UserError::Unauthenticated);
        };
        let candidate = digest_hex(&user.key.salt, presented_key);
        if constant_time_eq(candidate.as_bytes(), user.key.digest.as_bytes()) {
            Ok(user)
        } else {
            Err(UserError::Unauthenticated)
        }
    }

    /// Returns the user, if present.
    #[must_use]
    pub fn get(&self, id: &UserId) -> Option<&User> {
        self.users.get(id)
    }

    /// Serializes the store for durable storage. Only salts and digests are written.
    ///
    /// # Errors
    ///
    /// Returns the serializer error, if any.
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    /// Restores a store written by [`UserStore::to_json`].
    ///
    /// # Errors
    ///
    /// Returns the parser error for malformed input.
    pub fn from_json(text: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(text)
    }
}

/// Lowercase hex of `SHA-256(salt || 0x00 || key)`.
fn digest_hex(salt: &str, key: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(salt.as_bytes());
    hasher.update([0u8]);
    hasher.update(key.as_bytes());
    hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
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
#[path = "users_test.rs"]
mod users_test;
