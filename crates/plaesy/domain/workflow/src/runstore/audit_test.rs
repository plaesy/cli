use super::*;

fn entry(at: u64, action: &str, allowed: bool) -> AuditRecord {
    AuditRecord {
        at,
        principal: "agent-a".to_owned(),
        project: "proj-1".to_owned(),
        action: action.to_owned(),
        allowed,
        reason: if allowed {
            String::new()
        } else {
            "no_grant".to_owned()
        },
    }
}

fn seeded(dir: &tempfile::TempDir) -> AuditTrail {
    let trail = AuditTrail::open(dir.path().join("audit.log"));
    trail.append(&entry(100, "submit", true)).unwrap();
    trail.append(&entry(200, "tool:shell", false)).unwrap();
    trail.append(&entry(300, "submit", true)).unwrap();
    trail.append(&entry(400, "cancel", true)).unwrap();
    trail
}

#[test]
fn audit_list_returns_newest_first_with_limit() {
    let dir = tempfile::tempdir().unwrap();
    let trail = seeded(&dir);
    let listing = trail
        .list(&AuditQuery {
            limit: Some(2),
            ..AuditQuery::default()
        })
        .unwrap();
    let times: Vec<u64> = listing.entries.iter().map(|e| e.at).collect();
    assert_eq!(times, vec![400, 300]);
    assert!(!listing.torn_tail);
}

#[test]
fn audit_list_round_trips_every_field() {
    let dir = tempfile::tempdir().unwrap();
    let trail = seeded(&dir);
    let listing = trail.list(&AuditQuery::default()).unwrap();
    assert_eq!(listing.entries.len(), 4);
    assert_eq!(listing.entries[2], entry(200, "tool:shell", false));
    assert_eq!(listing.entries[3], entry(100, "submit", true));
}

#[test]
fn audit_list_filters_by_action() {
    let dir = tempfile::tempdir().unwrap();
    let trail = seeded(&dir);
    let listing = trail
        .list(&AuditQuery {
            action: Some("submit".to_owned()),
            ..AuditQuery::default()
        })
        .unwrap();
    let times: Vec<u64> = listing.entries.iter().map(|e| e.at).collect();
    assert_eq!(times, vec![300, 100]);
}

#[test]
fn audit_list_time_range_is_inclusive_on_both_ends() {
    let dir = tempfile::tempdir().unwrap();
    let trail = seeded(&dir);
    let listing = trail
        .list(&AuditQuery {
            since: Some(200),
            until: Some(300),
            ..AuditQuery::default()
        })
        .unwrap();
    let times: Vec<u64> = listing.entries.iter().map(|e| e.at).collect();
    assert_eq!(times, vec![300, 200]);
}

#[test]
fn audit_list_combines_action_time_and_limit() {
    let dir = tempfile::tempdir().unwrap();
    let trail = seeded(&dir);
    let listing = trail
        .list(&AuditQuery {
            action: Some("submit".to_owned()),
            since: Some(150),
            until: None,
            limit: Some(5),
        })
        .unwrap();
    assert_eq!(listing.entries, vec![entry(300, "submit", true)]);
}

#[test]
fn audit_list_on_missing_file_is_empty() {
    let dir = tempfile::tempdir().unwrap();
    let trail = AuditTrail::open(dir.path().join("none.log"));
    let listing = trail.list(&AuditQuery::default()).unwrap();
    assert!(listing.entries.is_empty());
    assert!(!listing.torn_tail);
}

#[test]
fn audit_append_rejects_separator_in_fields() {
    let dir = tempfile::tempdir().unwrap();
    let trail = AuditTrail::open(dir.path().join("audit.log"));
    let mut record = entry(1, "tool:a|b", true);
    assert_eq!(
        trail.append(&record),
        Err(AuditError::Log(RunLogError::InvalidPayload))
    );
    record.action = "ok".to_owned();
    record.principal = "line\nbreak".to_owned();
    assert_eq!(
        trail.append(&record),
        Err(AuditError::Log(RunLogError::InvalidPayload))
    );
}

#[test]
fn audit_list_reports_torn_tail_without_returning_it() {
    let dir = tempfile::tempdir().unwrap();
    let trail = seeded(&dir);
    let path = dir.path().join("audit.log");
    let mut raw = std::fs::OpenOptions::new()
        .append(true)
        .open(&path)
        .unwrap();
    std::io::Write::write_all(&mut raw, b"deadbeef half").unwrap();
    let listing = trail.list(&AuditQuery::default()).unwrap();
    assert_eq!(listing.entries.len(), 4);
    assert!(listing.torn_tail);
}

#[test]
fn audit_list_rejects_checksum_valid_but_malformed_record() {
    let dir = tempfile::tempdir().unwrap();
    let trail = seeded(&dir);
    // A valid checksum over a payload with the wrong layout: the verdict is not allow or deny.
    RunLog::open(dir.path().join("audit.log"))
        .append("500|agent-a|proj-1|submit|maybe|")
        .unwrap();
    assert_eq!(
        trail.list(&AuditQuery::default()),
        Err(AuditError::Malformed { record: 5 })
    );
}

#[test]
fn audit_list_propagates_checksum_corruption() {
    let dir = tempfile::tempdir().unwrap();
    let trail = seeded(&dir);
    let path = dir.path().join("audit.log");
    let mut text = std::fs::read_to_string(&path).unwrap();
    text.insert_str(0, "0000000000000000 tampered\n");
    std::fs::write(&path, text).unwrap();
    assert_eq!(
        trail.list(&AuditQuery::default()),
        Err(AuditError::Log(RunLogError::Corrupt { line: 1 }))
    );
}
