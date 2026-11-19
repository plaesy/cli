use super::*;

#[test]
fn task_run_happy_path_succeeds_only_with_evidence() {
    let dir = tempfile::tempdir().unwrap();
    let log = RunLog::open(dir.path().join("run.log"));
    let mut run = Run::new("task-1", "run-1", 3);
    run.apply_logged(&log, &Event::Start).unwrap();
    assert_eq!(
        run.apply_logged(
            &log,
            &Event::Complete {
                evidence: "  ".into()
            }
        ),
        Err(TransitionError::EmptyEvidence)
    );
    run.apply_logged(
        &log,
        &Event::Complete {
            evidence: "sha256:abc".into(),
        },
    )
    .unwrap();
    assert_eq!(
        run.state,
        RunState::Succeeded {
            evidence: "sha256:abc".into()
        }
    );
    assert_eq!(log.recover().unwrap().records.len(), 2);
}

#[test]
fn task_run_terminal_state_refuses_further_events() {
    let mut run = Run::new("task-1", "run-1", 3);
    run.apply(&Event::Start).unwrap();
    run.apply(&Event::Timeout).unwrap();
    assert!(run.state.is_terminal());
    assert!(matches!(
        run.apply(&Event::Start),
        Err(TransitionError::Invalid { .. })
    ));
}

#[test]
fn task_run_restart_while_running_is_requeued_within_policy() {
    let dir = tempfile::tempdir().unwrap();
    let log = RunLog::open(dir.path().join("run.log"));
    let mut run = Run::new("task-1", "run-1", 3);
    run.apply_logged(&log, &Event::Start).unwrap();
    // Simulated crash: the controller never reaches a terminal event.
    let recovered = recover(&log, "task-1", "run-1", 3).unwrap();
    assert_eq!(recovered.state, RunState::Queued);
    assert_eq!(recovered.attempt, 2);
    let events = log.recover().unwrap().records;
    assert!(events.iter().any(|record| record.ends_with("|restart")));
    assert!(events.iter().any(|record| record.ends_with("|requeue")));
}

#[test]
fn task_run_retry_budget_exhausted_fails_after_restart() {
    let dir = tempfile::tempdir().unwrap();
    let log = RunLog::open(dir.path().join("run.log"));
    let mut run = Run::new("task-1", "run-1", 1);
    run.apply_logged(&log, &Event::Start).unwrap();
    let recovered = recover(&log, "task-1", "run-1", 1).unwrap();
    assert_eq!(
        recovered.state,
        RunState::Failed {
            reason: "retry_exhausted".into()
        }
    );
}

#[test]
fn task_run_replay_of_completed_run_matches_live_state() {
    let dir = tempfile::tempdir().unwrap();
    let log = RunLog::open(dir.path().join("run.log"));
    let mut run = Run::new("task-1", "run-1", 3);
    run.apply_logged(&log, &Event::Start).unwrap();
    run.apply_logged(
        &log,
        &Event::Fail {
            reason: "tool crashed".into(),
        },
    )
    .unwrap();
    let replayed = recover(&log, "task-1", "run-1", 3).unwrap();
    assert_eq!(replayed.state, run.state);
}
