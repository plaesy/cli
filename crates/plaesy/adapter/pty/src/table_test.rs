use super::*;

#[test]
fn insert_stops_at_limit_and_reports_it() {
    let mut table = SessionTable::new(2);
    assert!(table.insert("a").is_ok());
    assert!(table.insert("b").is_ok());
    assert!(!table.has_capacity());
    assert!(matches!(
        table.insert("c"),
        Err(PtyError::SessionLimitReached { limit: 2 })
    ));
}

#[test]
fn zero_limit_rejects_every_insert() {
    let mut table: SessionTable<u8> = SessionTable::new(0);
    assert!(!table.has_capacity());
    assert!(matches!(
        table.insert(1),
        Err(PtyError::SessionLimitReached { limit: 0 })
    ));
}

#[test]
fn ids_are_unique_and_increasing() {
    let mut table = SessionTable::new(3);
    let first = table.insert(1u8).unwrap();
    let second = table.insert(2u8).unwrap();
    assert!(first < second);
    assert_eq!(first.get(), 1);
    assert_eq!(second.get(), 2);
}

#[test]
fn remove_frees_a_slot_and_ids_are_not_reused() {
    let mut table = SessionTable::new(1);
    let first = table.insert("first").unwrap();
    assert_eq!(table.remove(first).unwrap(), "first");
    assert!(table.has_capacity());

    let second = table.insert("second").unwrap();
    assert_ne!(first, second);
}

#[test]
fn unknown_id_is_not_found_for_get_and_remove() {
    let mut table: SessionTable<u8> = SessionTable::new(4);
    let missing = SessionId::new(99);
    assert!(matches!(
        table.get_mut(missing),
        Err(PtyError::SessionNotFound(id)) if id == missing
    ));
    assert!(matches!(
        table.remove(missing),
        Err(PtyError::SessionNotFound(_))
    ));
}

#[test]
fn get_mut_returns_the_stored_value() {
    let mut table = SessionTable::new(1);
    let id = table.insert(10u32).unwrap();
    *table.get_mut(id).unwrap() += 5;
    assert_eq!(*table.get_mut(id).unwrap(), 15);
}

#[test]
fn session_id_displays_its_raw_value() {
    assert_eq!(SessionId::new(42).to_string(), "42");
}
