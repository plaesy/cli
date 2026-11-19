//! Agent principals and grants (spec R5, single organization).
//!
//! A principal is bound to one project. Grants name tools. A revoked grant stays
//! recorded so a later denial can say why it was denied.

use std::collections::{BTreeMap, BTreeSet};

/// Stable identifier of an agent principal.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PrincipalId(String);

impl PrincipalId {
    /// Wraps a principal identifier.
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

/// Stable identifier of a project.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ProjectId(String);

impl ProjectId {
    /// Wraps a project identifier.
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

/// One agent principal, bound to one project.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Principal {
    project: ProjectId,
    grants: BTreeSet<String>,
    revoked: BTreeSet<String>,
}

impl Principal {
    /// Returns the project this principal belongs to.
    #[must_use]
    pub fn project(&self) -> &ProjectId {
        &self.project
    }

    /// Returns true when the tool is currently granted.
    #[must_use]
    pub fn has_grant(&self, tool: &str) -> bool {
        self.grants.contains(tool)
    }

    /// Returns true when the tool grant was granted and later revoked.
    #[must_use]
    pub fn was_revoked(&self, tool: &str) -> bool {
        self.revoked.contains(tool)
    }
}

/// Errors from identity management.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IdentityError {
    /// The principal identifier is already registered.
    Duplicate(PrincipalId),
    /// No principal with this identifier exists.
    Unknown(PrincipalId),
}

impl std::fmt::Display for IdentityError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Duplicate(id) => write!(f, "principal {} already registered", id.as_str()),
            Self::Unknown(id) => write!(f, "unknown principal {}", id.as_str()),
        }
    }
}

impl std::error::Error for IdentityError {}

/// Holds principals and their grants for the organization.
#[derive(Debug, Default)]
pub struct IdentityStore {
    principals: BTreeMap<PrincipalId, Principal>,
}

impl IdentityStore {
    /// Creates an empty store.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers a principal bound to one project, with no grants.
    ///
    /// # Errors
    ///
    /// Returns [`IdentityError::Duplicate`] when the identifier already exists.
    pub fn register(&mut self, id: PrincipalId, project: ProjectId) -> Result<(), IdentityError> {
        if self.principals.contains_key(&id) {
            return Err(IdentityError::Duplicate(id));
        }
        self.principals.insert(
            id,
            Principal {
                project,
                grants: BTreeSet::new(),
                revoked: BTreeSet::new(),
            },
        );
        Ok(())
    }

    /// Grants a tool to a principal. A previous revocation of the same tool is cleared.
    ///
    /// # Errors
    ///
    /// Returns [`IdentityError::Unknown`] when the principal does not exist.
    pub fn grant(&mut self, id: &PrincipalId, tool: &str) -> Result<(), IdentityError> {
        let principal = self
            .principals
            .get_mut(id)
            .ok_or_else(|| IdentityError::Unknown(id.clone()))?;
        principal.revoked.remove(tool);
        principal.grants.insert(tool.to_owned());
        Ok(())
    }

    /// Revokes a tool grant. Takes effect on the next authorization check.
    ///
    /// # Errors
    ///
    /// Returns [`IdentityError::Unknown`] when the principal does not exist.
    pub fn revoke(&mut self, id: &PrincipalId, tool: &str) -> Result<(), IdentityError> {
        let principal = self
            .principals
            .get_mut(id)
            .ok_or_else(|| IdentityError::Unknown(id.clone()))?;
        if principal.grants.remove(tool) {
            principal.revoked.insert(tool.to_owned());
        }
        Ok(())
    }

    /// Returns the principal, if registered.
    #[must_use]
    pub fn get(&self, id: &PrincipalId) -> Option<&Principal> {
        self.principals.get(id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_policy_grant_then_revoke_is_recorded() {
        let mut store = IdentityStore::new();
        let id = PrincipalId::new("agent-1");
        store.register(id.clone(), ProjectId::new("x")).unwrap();
        store.grant(&id, "shell").unwrap();
        assert!(store.get(&id).unwrap().has_grant("shell"));
        store.revoke(&id, "shell").unwrap();
        let principal = store.get(&id).unwrap();
        assert!(!principal.has_grant("shell"));
        assert!(principal.was_revoked("shell"));
    }

    #[test]
    fn identity_policy_duplicate_and_unknown_are_errors() {
        let mut store = IdentityStore::new();
        let id = PrincipalId::new("agent-1");
        store.register(id.clone(), ProjectId::new("x")).unwrap();
        assert!(matches!(
            store.register(id.clone(), ProjectId::new("x")),
            Err(IdentityError::Duplicate(_))
        ));
        assert!(matches!(
            store.grant(&PrincipalId::new("nobody"), "shell"),
            Err(IdentityError::Unknown(_))
        ));
    }
}
