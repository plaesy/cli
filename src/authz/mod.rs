//! Authorization decisions for agent actions (spec R5, VS-04, VS-05, VS-14).
//!
//! Every decision is recorded in an [`AuditLog`], whether it allows or denies.

use crate::identity::{IdentityStore, PrincipalId, ProjectId};

/// An action a principal may attempt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    /// Submit a task to the controller.
    Submit,
    /// Start a queued run.
    Start,
    /// Request use of a tool. Needs a grant for that tool.
    ToolRequest { tool: String },
    /// Send a message to another principal in the same project.
    MessageSend,
    /// Cancel a run.
    Cancel,
}

impl Action {
    /// Short stable label for audit records.
    #[must_use]
    pub fn label(&self) -> String {
        match self {
            Self::Submit => "submit".to_owned(),
            Self::Start => "start".to_owned(),
            Self::ToolRequest { tool } => format!("tool:{tool}"),
            Self::MessageSend => "message_send".to_owned(),
            Self::Cancel => "cancel".to_owned(),
        }
    }
}

/// Why an action was denied.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Denial {
    /// The principal is not registered.
    UnknownPrincipal,
    /// The principal belongs to another project.
    WrongProject,
    /// The tool was never granted to the principal.
    NoGrant,
    /// The tool grant was revoked.
    GrantRevoked,
}

impl Denial {
    /// Stable reason string for run records and audit.
    #[must_use]
    pub fn reason(&self) -> &'static str {
        match self {
            Self::UnknownPrincipal => "unknown_principal",
            Self::WrongProject => "wrong_project",
            Self::NoGrant => "no_grant",
            Self::GrantRevoked => "grant_revoked",
        }
    }
}

/// One recorded decision.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuditEvent {
    /// Principal that attempted the action.
    pub principal: String,
    /// Project the action targeted.
    pub project: String,
    /// Action label.
    pub action: String,
    /// True when the action was allowed.
    pub allowed: bool,
    /// Denial reason when not allowed.
    pub reason: Option<&'static str>,
}

/// Append-only record of authorization decisions.
#[derive(Debug, Default)]
pub struct AuditLog {
    events: Vec<AuditEvent>,
}

impl AuditLog {
    /// Creates an empty log.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns all recorded events in order.
    #[must_use]
    pub fn events(&self) -> &[AuditEvent] {
        &self.events
    }
}

/// Decides one action and records the decision.
///
/// # Errors
///
/// Returns the [`Denial`] when the action is not allowed. The denial is still recorded.
pub fn check(
    store: &IdentityStore,
    log: &mut AuditLog,
    principal: &PrincipalId,
    project: &ProjectId,
    action: &Action,
) -> Result<(), Denial> {
    let decision = decide(store, principal, project, action);
    log.events.push(AuditEvent {
        principal: principal.as_str().to_owned(),
        project: project.as_str().to_owned(),
        action: action.label(),
        allowed: decision.is_ok(),
        reason: decision.as_ref().err().map(Denial::reason),
    });
    decision
}

fn decide(
    store: &IdentityStore,
    principal: &PrincipalId,
    project: &ProjectId,
    action: &Action,
) -> Result<(), Denial> {
    let record = store.get(principal).ok_or(Denial::UnknownPrincipal)?;
    if record.project() != project {
        return Err(Denial::WrongProject);
    }
    match action {
        Action::ToolRequest { tool } => {
            if record.has_grant(tool) {
                Ok(())
            } else if record.was_revoked(tool) {
                Err(Denial::GrantRevoked)
            } else {
                Err(Denial::NoGrant)
            }
        }
        Action::Submit | Action::Start | Action::MessageSend | Action::Cancel => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup() -> (IdentityStore, PrincipalId) {
        let mut store = IdentityStore::new();
        let id = PrincipalId::new("agent-1");
        store
            .register(id.clone(), ProjectId::new("project-a"))
            .unwrap();
        (store, id)
    }

    #[test]
    fn identity_policy_denies_tool_without_grant_and_audits_it() {
        let (store, id) = setup();
        let mut log = AuditLog::new();
        let action = Action::ToolRequest {
            tool: "shell".into(),
        };
        let result = check(&store, &mut log, &id, &ProjectId::new("project-a"), &action);
        assert_eq!(result, Err(Denial::NoGrant));
        assert_eq!(log.events().len(), 1);
        assert!(!log.events()[0].allowed);
        assert_eq!(log.events()[0].reason, Some("no_grant"));
    }

    #[test]
    fn identity_policy_revoked_grant_is_denied_on_next_action() {
        let (mut store, id) = setup();
        let mut log = AuditLog::new();
        let project = ProjectId::new("project-a");
        let action = Action::ToolRequest {
            tool: "shell".into(),
        };
        store.grant(&id, "shell").unwrap();
        assert_eq!(check(&store, &mut log, &id, &project, &action), Ok(()));
        store.revoke(&id, "shell").unwrap();
        assert_eq!(
            check(&store, &mut log, &id, &project, &action),
            Err(Denial::GrantRevoked)
        );
        assert_eq!(log.events().last().unwrap().reason, Some("grant_revoked"));
    }

    #[test]
    fn identity_policy_other_project_cannot_act_on_this_project() {
        let (store, id) = setup();
        let mut log = AuditLog::new();
        let result = check(
            &store,
            &mut log,
            &id,
            &ProjectId::new("project-b"),
            &Action::MessageSend,
        );
        assert_eq!(result, Err(Denial::WrongProject));
    }

    #[test]
    fn identity_policy_unknown_principal_is_denied() {
        let store = IdentityStore::new();
        let mut log = AuditLog::new();
        let result = check(
            &store,
            &mut log,
            &PrincipalId::new("ghost"),
            &ProjectId::new("p"),
            &Action::Submit,
        );
        assert_eq!(result, Err(Denial::UnknownPrincipal));
    }
}
