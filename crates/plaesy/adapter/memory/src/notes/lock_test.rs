use super::*;

#[test]
fn fresh_daemon_lock_blocks_a_second_daemon() {
    let root = tempfile::tempdir().unwrap();
    std::fs::create_dir(root.path().join(DAEMON_LOCK_NAME)).unwrap();

    let result = WriteLock::acquire_daemon(root.path());

    assert!(result.is_err(), "a live daemon lock must not be taken over");
    assert!(root.path().join(DAEMON_LOCK_NAME).is_dir());
}

#[test]
fn stale_daemon_lock_with_heartbeat_file_is_reclaimed() {
    let root = tempfile::tempdir().unwrap();
    let daemon = root.path().join(DAEMON_LOCK_NAME);
    std::fs::create_dir(&daemon).unwrap();
    std::fs::write(daemon.join("heartbeat"), b"").unwrap();
    // A killed daemon leaves this file behind; the age check needs a non-zero elapsed time.
    std::thread::sleep(Duration::from_millis(20));

    WriteLock::reject_live_daemon(root.path(), Duration::ZERO).unwrap();

    assert!(!daemon.exists(), "stale lock directory must be removed");
}

#[test]
fn acquire_daemon_takes_the_lock_when_vault_is_free() {
    let root = tempfile::tempdir().unwrap();

    let lock = WriteLock::acquire_daemon(root.path()).unwrap();

    assert!(root.path().join(DAEMON_LOCK_NAME).is_dir());
    drop(lock);
    assert!(!root.path().join(DAEMON_LOCK_NAME).exists());
}
