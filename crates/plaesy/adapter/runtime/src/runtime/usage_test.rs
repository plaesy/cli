use super::*;

#[test]
fn bounded_runtime_usage_counts_started_steps() {
    let cancel = CancelFlag::new();
    let (outcome, usage) =
        supervise_metered(Budget { max_steps: 3 }, &cancel, |_| StepResult::Continue);
    assert_eq!(
        outcome,
        Outcome::Failed {
            reason: "budget_exceeded".into()
        }
    );
    assert_eq!(usage.steps, 3);
}
