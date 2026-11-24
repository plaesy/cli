use super::*;

const KEY: &str = "fake-test-key-0123456789abcdef0123456789";
const OTHER_KEY: &str = "fake-other-key-fedcba9876543210fedcba98";
const SALT: &str = "a1b2c3d4e5f60718";

fn store_with_alice(role: Role) -> UserStore {
    let mut store = UserStore::new();
    store
        .create(UserId::new("alice"), "Alice", role, KEY, SALT)
        .unwrap();
    store
}

#[test]
fn users_create_registers_user_with_role_and_name() {
    let store = store_with_alice(Role::Operator);
    let user = store.get(&UserId::new("alice")).unwrap();
    assert_eq!(user.id().as_str(), "alice");
    assert_eq!(user.display_name(), "Alice");
    assert_eq!(user.role(), Role::Operator);
}

#[test]
fn users_create_stores_digest_not_plain_key() {
    let store = store_with_alice(Role::Viewer);
    let text = store.to_json().unwrap();
    assert!(!text.contains(KEY), "raw key must not be serialized");
    assert!(text.contains("digest"));
    assert_eq!(digest_hex(SALT, KEY).len(), 64);
}

#[test]
fn users_create_rejects_duplicate_id() {
    let mut store = store_with_alice(Role::Viewer);
    let result = store.create(UserId::new("alice"), "Again", Role::Admin, KEY, SALT);
    assert_eq!(result, Err(UserError::Duplicate(UserId::new("alice"))));
}

#[test]
fn users_create_rejects_empty_fields_short_key_and_bad_salt() {
    let mut store = UserStore::new();
    assert!(matches!(
        store.create(UserId::new(" "), "A", Role::Viewer, KEY, SALT),
        Err(UserError::Invalid(_))
    ));
    assert!(matches!(
        store.create(UserId::new("bob"), "", Role::Viewer, KEY, SALT),
        Err(UserError::Invalid(_))
    ));
    assert!(matches!(
        store.create(UserId::new("bob"), "Bob", Role::Viewer, "short", SALT),
        Err(UserError::Invalid(_))
    ));
    assert!(matches!(
        store.create(UserId::new("bob"), "Bob", Role::Viewer, KEY, "not-hex!"),
        Err(UserError::Invalid(_))
    ));
    assert!(store.get(&UserId::new("bob")).is_none());
}

#[test]
fn users_authenticate_accepts_correct_key() {
    let store = store_with_alice(Role::Admin);
    let user = store
        .authenticate(&UserId::new("alice"), KEY)
        .expect("correct key must verify");
    assert_eq!(user.role(), Role::Admin);
}

#[test]
fn users_authenticate_rejects_wrong_key_and_unknown_user() {
    let store = store_with_alice(Role::Admin);
    let alice = UserId::new("alice");
    let mallory = UserId::new("mallory");
    assert_eq!(
        store.authenticate(&alice, OTHER_KEY).map(User::role),
        Err(UserError::Unauthenticated)
    );
    assert_eq!(
        store.authenticate(&alice, "").map(User::role),
        Err(UserError::Unauthenticated)
    );
    assert_eq!(
        store.authenticate(&mallory, KEY).map(User::role),
        Err(UserError::Unauthenticated)
    );
}

#[test]
fn users_digest_depends_on_salt() {
    let mut store = UserStore::new();
    store
        .create(UserId::new("carol"), "Carol", Role::Viewer, KEY, "00ff")
        .unwrap();
    store
        .create(UserId::new("dave"), "Dave", Role::Viewer, KEY, "11ee")
        .unwrap();
    let carol = store.get(&UserId::new("carol")).unwrap();
    let dave = store.get(&UserId::new("dave")).unwrap();
    assert_ne!(carol.key.digest, dave.key.digest);
}

#[test]
fn users_require_role_rejects_insufficient_role() {
    let viewer = store_with_alice(Role::Viewer);
    let user = viewer.get(&UserId::new("alice")).unwrap();
    assert_eq!(
        require_role(user, Role::Operator),
        Err(UserError::InsufficientRole {
            required: Role::Operator,
            actual: Role::Viewer,
        })
    );
    assert!(require_role(user, Role::Viewer).is_ok());
}

#[test]
fn users_require_role_allows_higher_roles() {
    let operator = store_with_alice(Role::Operator);
    let op = operator.get(&UserId::new("alice")).unwrap();
    assert!(require_role(op, Role::Viewer).is_ok());
    assert!(require_role(op, Role::Operator).is_ok());
    assert!(require_role(op, Role::Admin).is_err());

    let admin = store_with_alice(Role::Admin);
    let ad = admin.get(&UserId::new("alice")).unwrap();
    assert!(require_role(ad, Role::Admin).is_ok());
}

#[test]
fn users_json_round_trip_keeps_authentication() {
    let store = store_with_alice(Role::Operator);
    let restored = UserStore::from_json(&store.to_json().unwrap()).unwrap();
    assert!(restored.authenticate(&UserId::new("alice"), KEY).is_ok());
    assert!(restored.authenticate(&UserId::new("alice"), OTHER_KEY).is_err());
    assert_eq!(
        restored.get(&UserId::new("alice")).unwrap().role(),
        Role::Operator
    );
}
