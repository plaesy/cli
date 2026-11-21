use super::*;

#[test]
fn identity_policy_store_survives_restart_via_json() {
    use super::*;
    let mut store = IdentityStore::new();
    let id = PrincipalId::new("agent-1");
    store.register(id.clone(), ProjectId::new("x")).unwrap();
    store.grant(&id, "shell").unwrap();
    store.revoke(&id, "shell").unwrap();
    let restored = IdentityStore::from_json(&store.to_json().unwrap()).unwrap();
    let principal = restored.get(&id).unwrap();
    assert!(!principal.has_grant("shell"));
    assert!(principal.was_revoked("shell"));
    assert_eq!(principal.project().as_str(), "x");
}

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
