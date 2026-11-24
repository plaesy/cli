use super::*;

fn spec(id: &str, deps: &[&str]) -> StepSpec {
    StepSpec::new(id, deps)
}

fn complete(evidence: &str) -> Event {
    Event::Complete {
        evidence: evidence.to_owned(),
    }
}

#[test]
fn chain_is_ordered_by_dependency() {
    let pipeline = Pipeline::new(&[
        spec("build", &["fetch"]),
        spec("test", &["build"]),
        spec("fetch", &[]),
    ])
    .unwrap();
    assert_eq!(pipeline.order(), vec!["fetch", "build", "test"]);
}

#[test]
fn diamond_runs_shared_root_first_and_join_last() {
    let pipeline = Pipeline::new(&[
        spec("join", &["left", "right"]),
        spec("left", &["root"]),
        spec("right", &["root"]),
        spec("root", &[]),
    ])
    .unwrap();
    assert_eq!(pipeline.order(), vec!["root", "left", "right", "join"]);
}

#[test]
fn independent_steps_keep_declaration_order() {
    let pipeline =
        Pipeline::new(&[spec("zeta", &[]), spec("alpha", &[]), spec("mid", &[])]).unwrap();
    assert_eq!(pipeline.order(), vec!["zeta", "alpha", "mid"]);
}

#[test]
fn duplicate_dependency_is_accepted_once() {
    let pipeline = Pipeline::new(&[spec("a", &[]), spec("b", &["a", "a"])]).unwrap();
    assert_eq!(pipeline.order(), vec!["a", "b"]);
}

#[test]
fn two_step_cycle_is_rejected_with_its_members() {
    let err = Pipeline::new(&[spec("a", &["b"]), spec("b", &["a"])]).unwrap_err();
    assert_eq!(
        err,
        PipelineError::Cycle {
            steps: vec!["a".into(), "b".into()]
        }
    );
    assert_eq!(err.to_string(), "dependency cycle: a -> b -> a");
}

#[test]
fn cycle_reports_only_members_not_steps_merely_downstream() {
    // x is free, y and z form a cycle, and w waits on the cycle.
    let err = Pipeline::new(&[
        spec("x", &[]),
        spec("y", &["z"]),
        spec("z", &["y"]),
        spec("w", &["y"]),
    ])
    .unwrap_err();
    assert_eq!(
        err,
        PipelineError::Cycle {
            steps: vec!["y".into(), "z".into()]
        }
    );
}

#[test]
fn longer_cycle_is_reported_in_dependency_order() {
    let err =
        Pipeline::new(&[spec("a", &["b"]), spec("b", &["c"]), spec("c", &["a"])]).unwrap_err();
    assert_eq!(
        err,
        PipelineError::Cycle {
            steps: vec!["a".into(), "b".into(), "c".into()]
        }
    );
}

#[test]
fn self_dependency_is_rejected() {
    assert_eq!(
        Pipeline::new(&[spec("a", &["a"])]).unwrap_err(),
        PipelineError::SelfDependency { step: "a".into() }
    );
}

#[test]
fn unknown_dependency_is_rejected() {
    assert_eq!(
        Pipeline::new(&[spec("a", &["ghost"])]).unwrap_err(),
        PipelineError::UnknownDependency {
            step: "a".into(),
            dependency: "ghost".into()
        }
    );
}

#[test]
fn duplicate_and_empty_ids_are_rejected() {
    assert_eq!(
        Pipeline::new(&[spec("a", &[]), spec("a", &[])]).unwrap_err(),
        PipelineError::DuplicateStep { id: "a".into() }
    );
    assert_eq!(
        Pipeline::new(&[spec("  ", &[])]).unwrap_err(),
        PipelineError::EmptyId
    );
}

#[test]
fn empty_pipeline_is_valid_and_complete() {
    let pipeline = Pipeline::new(&[]).unwrap();
    assert!(pipeline.order().is_empty());
    assert!(pipeline.start().is_complete());
}

#[test]
fn run_starts_with_roots_ready_and_others_queued() {
    let pipeline = Pipeline::new(&[spec("a", &[]), spec("b", &["a"])]).unwrap();
    let run = pipeline.start();
    assert_eq!(run.ready(), vec!["a"]);
    assert_eq!(run.state("b").unwrap(), &RunState::Queued);
    assert!(!run.is_complete());
}

#[test]
fn step_cannot_start_before_dependency_succeeds() {
    let pipeline = Pipeline::new(&[spec("a", &[]), spec("b", &["a"])]).unwrap();
    let mut run = pipeline.start();
    assert_eq!(
        run.apply("b", &Event::Start),
        Err(PipelineError::DependencyNotSatisfied {
            step: "b".into(),
            dependency: "a".into()
        })
    );
    assert_eq!(run.state("b").unwrap(), &RunState::Queued);
}

#[test]
fn full_run_moves_steps_through_lifecycle_in_order() {
    let pipeline = Pipeline::new(&[
        spec("a", &[]),
        spec("b", &["a"]),
        spec("c", &["a"]),
        spec("d", &["b", "c"]),
    ])
    .unwrap();
    let mut run = pipeline.start();

    run.apply("a", &Event::Start).unwrap();
    assert_eq!(run.state("a").unwrap(), &RunState::Running);
    assert!(run.ready().is_empty());

    run.apply("a", &complete("sha256:a")).unwrap();
    assert_eq!(run.ready(), vec!["b", "c"]);

    run.apply("b", &Event::Start).unwrap();
    run.apply("b", &complete("sha256:b")).unwrap();
    assert_eq!(run.ready(), vec!["c"]);

    run.apply("c", &Event::Start).unwrap();
    run.apply("c", &complete("sha256:c")).unwrap();
    assert_eq!(run.ready(), vec!["d"]);

    run.apply("d", &Event::Start).unwrap();
    run.apply("d", &complete("sha256:d")).unwrap();
    assert!(run.is_complete());
    assert!(run.ready().is_empty());
    assert!(run.blocked().is_empty());
}

#[test]
fn failed_step_blocks_its_downstream_steps() {
    let pipeline = Pipeline::new(&[
        spec("a", &[]),
        spec("b", &["a"]),
        spec("c", &["b"]),
        spec("other", &[]),
    ])
    .unwrap();
    let mut run = pipeline.start();
    run.apply("a", &Event::Start).unwrap();
    run.apply(
        "a",
        &Event::Fail {
            reason: "boom".into(),
        },
    )
    .unwrap();
    assert_eq!(run.blocked(), vec!["b"]);
    assert_eq!(run.ready(), vec!["other"]);
    assert!(!run.is_complete());
}

#[test]
fn lifecycle_refusal_is_passed_through_and_leaves_state_unchanged() {
    let pipeline = Pipeline::new(&[spec("a", &[])]).unwrap();
    let mut run = pipeline.start();
    assert_eq!(
        run.apply("a", &complete("sha256:a")),
        Err(PipelineError::Transition(TransitionError::Invalid {
            state: "Queued".into(),
            event: "complete:sha256:a".into()
        }))
    );
    run.apply("a", &Event::Start).unwrap();
    assert_eq!(
        run.apply("a", &complete("  ")),
        Err(PipelineError::Transition(TransitionError::EmptyEvidence))
    );
    assert_eq!(run.state("a").unwrap(), &RunState::Running);
}

#[test]
fn unknown_step_is_rejected() {
    let pipeline = Pipeline::new(&[spec("a", &[])]).unwrap();
    let mut run = pipeline.start();
    assert_eq!(
        run.state("nope"),
        Err(PipelineError::UnknownStep { id: "nope".into() })
    );
    assert_eq!(
        run.apply("nope", &Event::Start),
        Err(PipelineError::UnknownStep { id: "nope".into() })
    );
}
