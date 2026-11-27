use std::process::Command;
use tempfile::TempDir;

#[test]
fn relay_wait_returns_pending_message_without_acknowledging_it() {
    let project = TempDir::new().expect("create temporary project");
    let binary = env!("CARGO_BIN_EXE_plaesy");
    let run = |args: &[&str]| {
        Command::new(binary)
            .args(args)
            .current_dir(project.path())
            .output()
            .expect("run plaesy CLI")
    };

    let send = run(&[
        "relay",
        "send",
        "--id",
        "wait-message",
        "--from",
        "worker-a",
        "--to",
        "worker-b",
        "arrived",
    ]);
    assert!(
        send.status.success(),
        "{}",
        String::from_utf8_lossy(&send.stderr)
    );

    let claim = run(&[
        "relay",
        "claim",
        "--agent",
        "worker-b",
        "--lease-seconds",
        "60",
    ]);
    assert!(claim.status.success());
    let claimed: serde_json::Value =
        serde_json::from_slice(&claim.stdout).expect("claim emits JSON");
    assert!(claimed["lease_owner"].as_str().is_some());

    let resend = run(&[
        "relay",
        "send",
        "--id",
        "wait-message",
        "--from",
        "worker-a",
        "--to",
        "worker-b",
        "arrived",
    ]);
    assert!(resend.status.success());
    let resent: serde_json::Value =
        serde_json::from_slice(&resend.stdout).expect("resend emits JSON");
    assert!(resent.get("lease_owner").is_none());

    let inbox = run(&["relay", "inbox", "--agent", "worker-b"]);
    let pending: serde_json::Value =
        serde_json::from_slice(&inbox.stdout).expect("inbox emits JSON");
    assert!(pending[0].get("lease_owner").is_none());
    assert_eq!(
        pending[0]["lease_expiry_epoch_ms"],
        claimed["lease_expiry_epoch_ms"]
    );

    let wait = run(&[
        "relay",
        "wait",
        "--agent",
        "worker-b",
        "--timeout",
        "1",
        "--poll-interval",
        "10",
    ]);
    assert!(
        wait.status.success(),
        "{}",
        String::from_utf8_lossy(&wait.stderr)
    );
    let messages: serde_json::Value =
        serde_json::from_slice(&wait.stdout).expect("wait emits JSON");
    assert_eq!(messages["id"], "wait-message");
    assert_eq!(messages["body"], "arrived");
    assert!(messages.get("lease_owner").is_none());

    let inbox = run(&["relay", "inbox", "--agent", "worker-b"]);
    assert!(inbox.status.success());
    let pending: serde_json::Value =
        serde_json::from_slice(&inbox.stdout).expect("inbox emits JSON");
    assert_eq!(pending.as_array().expect("array").len(), 1);
    assert!(pending[0].get("lease_owner").is_none());
}

#[test]
fn relay_claim_is_exclusive_and_ack_requires_owner_token() {
    let project = TempDir::new().expect("create temporary project");
    let binary = env!("CARGO_BIN_EXE_plaesy");
    let run = |args: &[&str]| {
        Command::new(binary)
            .args(args)
            .current_dir(project.path())
            .output()
            .expect("run plaesy CLI")
    };
    let sent = run(&[
        "relay", "send", "--id", "claim-1", "--from", "a", "--to", "b", "payload",
    ]);
    assert!(sent.status.success());

    let claim = run(&["relay", "claim", "--agent", "b", "--lease-seconds", "60"]);
    assert!(
        claim.status.success(),
        "{}",
        String::from_utf8_lossy(&claim.stderr)
    );
    let claimed: serde_json::Value = serde_json::from_slice(&claim.stdout).unwrap();
    assert_eq!(claimed["id"], "claim-1");
    let token = claimed["lease_owner"].as_str().unwrap();
    assert!(claimed["lease_expiry_epoch_ms"].as_i64().is_some());

    let competing = run(&["relay", "claim", "--agent", "b", "--lease-seconds", "60"]);
    assert!(competing.status.success());
    assert_eq!(String::from_utf8_lossy(&competing.stdout).trim(), "null");

    let stale_ack = run(&[
        "relay",
        "ack",
        "--id",
        "claim-1",
        "--from",
        "a",
        "--agent",
        "b",
        "--lease-token",
        "stale-token",
    ]);
    assert!(!stale_ack.status.success());
    let obsolete_flag = run(&[
        "relay", "ack", "--id", "claim-1", "--from", "a", "--agent", "b", "--owner", token,
    ]);
    assert!(!obsolete_flag.status.success());
    let ack = run(&[
        "relay",
        "ack",
        "--id",
        "claim-1",
        "--from",
        "a",
        "--agent",
        "b",
        "--lease-token",
        token,
    ]);
    assert!(
        ack.status.success(),
        "{}",
        String::from_utf8_lossy(&ack.stderr)
    );
}

#[test]
fn relay_wait_exits_after_timeout_when_inbox_is_empty() {
    let project = TempDir::new().expect("create temporary project");
    let output = Command::new(env!("CARGO_BIN_EXE_plaesy"))
        .args([
            "relay",
            "wait",
            "--agent",
            "worker-b",
            "--timeout",
            "1",
            "--poll-interval",
            "10",
        ])
        .current_dir(project.path())
        .output()
        .expect("run plaesy relay wait");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "null");
}
