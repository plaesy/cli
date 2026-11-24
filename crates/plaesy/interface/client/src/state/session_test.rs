use super::*;

fn alice() -> User {
    User {
        id: "u1".to_owned(),
        name: "Alice".to_owned(),
        roles: vec!["editor".to_owned()],
    }
}

#[test]
fn session_starts_signed_out() {
    let session = Session::new();
    assert!(!session.is_signed_in());
    assert!(session.token().is_none());
    assert!(session.user().is_none());
}

#[test]
fn session_sign_in_stores_token_and_user() {
    let mut session = Session::new();
    session.sign_in("abc", alice()).unwrap();
    assert!(session.is_signed_in());
    assert_eq!(session.token(), Some("abc"));
    assert_eq!(session.user().unwrap().name, "Alice");
}

#[test]
fn session_empty_token_is_refused_and_state_is_unchanged() {
    let mut session = Session::new();
    assert_eq!(
        session.sign_in("   ", alice()),
        Err(SessionError::EmptyToken)
    );
    assert!(!session.is_signed_in());
}

#[test]
fn session_sign_out_clears_everything() {
    let mut session = Session::new();
    session.sign_in("abc", alice()).unwrap();
    session.sign_out();
    assert!(!session.is_signed_in());
    assert!(session.user().is_none());
}

#[test]
fn session_has_role_checks_the_signed_in_user_only() {
    let mut session = Session::new();
    assert!(!session.has_role("editor"));
    session.sign_in("abc", alice()).unwrap();
    assert!(session.has_role("editor"));
    assert!(!session.has_role("admin"));
}
