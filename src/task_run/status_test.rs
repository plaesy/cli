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
