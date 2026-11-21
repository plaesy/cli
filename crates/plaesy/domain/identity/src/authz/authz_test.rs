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
