//! Adapter conformance suite (spec VS-02, VS-03). Runs the fake adapter through each
//! scripted outcome and checks the contract rules against a deliberately bad adapter.

use plaesy::adapter::{
    check_conformance, CancelFlag, ExecutionAdapter, FakeAdapter, Outcome, Script,
};

#[test]
fn adapter_conformance_fake_success_reports_evidence() {
    let mut adapter = FakeAdapter::new(Script::Succeed("artifact:run-1".into()));
    let outcome = adapter.execute("task-1", &CancelFlag::new());
    assert_eq!(
        outcome,
        Outcome::Succeeded {
            evidence: "artifact:run-1".into()
        }
    );
    assert!(check_conformance(&mut adapter).is_empty());
}

#[test]
fn adapter_conformance_fake_failure_timeout_and_cancel_map_to_outcomes() {
    let mut failing = FakeAdapter::new(Script::Fail("tool crashed".into()));
    assert_eq!(
        failing.execute("task-1", &CancelFlag::new()),
        Outcome::Failed {
            reason: "tool crashed".into()
        }
    );

    let mut timing_out = FakeAdapter::new(Script::Timeout);
    assert_eq!(
        timing_out.execute("task-1", &CancelFlag::new()),
        Outcome::TimedOut
    );

    let mut cancelling = FakeAdapter::new(Script::Cancel);
    let flag = CancelFlag::new();
    assert_eq!(cancelling.execute("task-1", &flag), Outcome::Cancelled);
    assert!(flag.is_cancelled());
}

#[test]
fn adapter_conformance_cancel_before_start_is_honored() {
    let mut adapter = FakeAdapter::new(Script::Succeed("artifact".into()));
    let flag = CancelFlag::new();
    flag.cancel();
    assert_eq!(adapter.execute("task-1", &flag), Outcome::Cancelled);
}

/// A backend that claims success without evidence. The suite must flag it.
struct SilentSuccess;

impl ExecutionAdapter for SilentSuccess {
    fn name(&self) -> &str {
        "silent"
    }

    fn execute(&mut self, _task_id: &str, _cancel: &CancelFlag) -> Outcome {
        Outcome::Succeeded {
            evidence: String::new(),
        }
    }
}

#[test]
fn adapter_conformance_flags_success_without_evidence() {
    let mut bad = SilentSuccess;
    assert_eq!(
        check_conformance(&mut bad),
        vec!["succeeded without evidence".to_owned()]
    );
}
