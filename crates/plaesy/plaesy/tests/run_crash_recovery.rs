//! Process-kill crash recovery and restore drill (spec VS-09, VS-10, VS-16).
//! Release checklist gates 2 and 5. Synthetic data only.

use plaesy::evidence::EvidenceStore;
use plaesy::runstore::RunLog;
use plaesy::task_run::{recover, Event, Run, RunState};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

const WORKER_ENV: &str = "PLAESY_CRASH_WORKER_LOG";

/// Runs only when re-invoked as a worker by the test below. Otherwise it returns at once.
#[test]
fn task_run_crash_worker_entry() {
    let Some(log_path) = std::env::var_os(WORKER_ENV) else {
        return;
    };
    let log = RunLog::open(log_path);
    let mut run = Run::new("task-1", "run-1", 3);
    run.apply_logged(&log, &Event::Start).unwrap();
    // Wait for the parent to kill this process. It never reaches a terminal event.
    loop {
        std::thread::sleep(Duration::from_secs(60));
    }
}

#[test]
fn task_run_real_process_kill_is_recovered_within_policy() {
    let dir = tempfile::tempdir().unwrap();
    let log_path = dir.path().join("run.log");
    let mut child = Command::new(std::env::current_exe().unwrap())
        .args(["task_run_crash_worker_entry", "--exact", "--nocapture"])
        .env(WORKER_ENV, &log_path)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();

    let deadline = Instant::now() + Duration::from_secs(60);
    loop {
        let written = std::fs::read_to_string(&log_path)
            .map(|text| text.contains("|start"))
            .unwrap_or(false);
        if written {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "worker never wrote its start record"
        );
        std::thread::sleep(Duration::from_millis(20));
    }

    child.kill().unwrap();
    child.wait().unwrap();

    let log = RunLog::open(&log_path);
    let recovered = recover(&log, "task-1", "run-1", 3).unwrap();
    assert_eq!(recovered.state, RunState::Queued);
    assert_eq!(recovered.attempt, 2);
    assert!(log
        .recover()
        .unwrap()
        .records
        .iter()
        .any(|r| r.ends_with("|restart")));
}

#[test]
fn task_run_restore_drill_recovers_log_and_evidence_from_backup() {
    let primary = tempfile::tempdir().unwrap();
    let backup = tempfile::tempdir().unwrap();
    let restored = tempfile::tempdir().unwrap();

    let log_path = primary.path().join("run.log");
    let evidence_root = primary.path().join("evidence");
    let log = RunLog::open(&log_path);
    let store = EvidenceStore::open(&evidence_root);
    let mut run = Run::new("task-9", "run-9", 3);
    run.apply_logged(&log, &Event::Start).unwrap();
    let reference = store.put(b"synthetic artifact").unwrap();
    run.apply_logged(
        &log,
        &Event::Complete {
            evidence: reference.sha256.clone(),
        },
    )
    .unwrap();

    // Backup: the log file and every evidence artifact.
    std::fs::copy(&log_path, backup.path().join("run.log")).unwrap();
    std::fs::create_dir_all(backup.path().join("evidence")).unwrap();
    for entry in std::fs::read_dir(&evidence_root).unwrap() {
        let entry = entry.unwrap();
        std::fs::copy(
            entry.path(),
            backup.path().join("evidence").join(entry.file_name()),
        )
        .unwrap();
    }

    // Disaster: the primary data is lost.
    std::fs::remove_file(&log_path).unwrap();
    std::fs::remove_dir_all(&evidence_root).unwrap();

    // Restore into an empty location and check the run and the artifact.
    std::fs::copy(
        backup.path().join("run.log"),
        restored.path().join("run.log"),
    )
    .unwrap();
    std::fs::create_dir_all(restored.path().join("evidence")).unwrap();
    for entry in std::fs::read_dir(backup.path().join("evidence")).unwrap() {
        let entry = entry.unwrap();
        std::fs::copy(
            entry.path(),
            restored.path().join("evidence").join(entry.file_name()),
        )
        .unwrap();
    }
    let restored_log = RunLog::open(restored.path().join("run.log"));
    let recovered = recover(&restored_log, "task-9", "run-9", 3).unwrap();
    assert_eq!(
        recovered.state,
        RunState::Succeeded {
            evidence: reference.sha256.clone()
        }
    );
    let restored_store = EvidenceStore::open(restored.path().join("evidence"));
    assert_eq!(
        restored_store.get(&reference).unwrap(),
        b"synthetic artifact"
    );
}
