use super::*;

#[test]
fn runstore_round_trip_keeps_order() {
    let dir = tempfile::tempdir().unwrap();
    let log = RunLog::open(dir.path().join("run.log"));
    log.append("first").unwrap();
    log.append("second").unwrap();
    let recovered = log.recover().unwrap();
    assert_eq!(recovered.records, vec!["first", "second"]);
    assert!(!recovered.torn_tail);
}

#[test]
fn runstore_torn_tail_is_reported_and_not_a_record() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("run.log");
    let log = RunLog::open(&path);
    log.append("complete-record").unwrap();
    let mut raw = OpenOptions::new().append(true).open(&path).unwrap();
    raw.write_all(b"deadbeef half-written").unwrap();
    let recovered = log.recover().unwrap();
    assert_eq!(recovered.records, vec!["complete-record"]);
    assert!(recovered.torn_tail);
}

#[test]
fn runstore_corruption_before_tail_is_an_error() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("run.log");
    let log = RunLog::open(&path);
    log.append("good").unwrap();
    let mut raw = OpenOptions::new().append(true).open(&path).unwrap();
    raw.write_all(b"0000000000000000 tampered\n").unwrap();
    log.append("after").unwrap();
    assert_eq!(log.recover(), Err(RunLogError::Corrupt { line: 2 }));
}

#[test]
fn runstore_rejects_payload_with_newline() {
    let dir = tempfile::tempdir().unwrap();
    let log = RunLog::open(dir.path().join("run.log"));
    assert_eq!(log.append("a\nb"), Err(RunLogError::InvalidPayload));
}
