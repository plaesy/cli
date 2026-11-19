//! Read-only status line for one run (spec R10, minimal operator visibility).
//!
//! Shows persisted state and reason only. It does not offer any control action.

use super::{Run, RunState};

/// One line describing a run's persisted state, attempt, and reason.
#[must_use]
pub fn status_line(run: &Run) -> String {
    let state = match &run.state {
        RunState::Queued => "queued".to_owned(),
        RunState::Running => "running".to_owned(),
        RunState::Succeeded { evidence } => format!("succeeded evidence={evidence}"),
        RunState::Failed { reason } => format!("failed reason={reason}"),
        RunState::TimedOut => "timed_out".to_owned(),
        RunState::Cancelled => "cancelled".to_owned(),
        RunState::Interrupted => "interrupted".to_owned(),
    };
    format!(
        "task={} run={} attempt={}/{} state={}",
        run.task_id, run.run_id, run.attempt, run.max_attempts, state
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn task_run_status_line_shows_state_and_reason() {
        let mut run = Run::new("task-1", "run-1", 3);
        run.apply(&super::super::Event::Start).unwrap();
        run.apply(&super::super::Event::Fail {
            reason: "tool crashed".into(),
        })
        .unwrap();
        let line = status_line(&run);
        assert_eq!(
            line,
            "task=task-1 run=run-1 attempt=1/3 state=failed reason=tool crashed"
        );
    }
}
