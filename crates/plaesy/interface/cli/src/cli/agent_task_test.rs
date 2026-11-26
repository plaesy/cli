use super::{transition, TaskState};

use TaskState::{Canceled, Completed, Failed, InputRequired, Submitted, Working};

#[test]
fn wire_names_round_trip() {
    for state in [
        Submitted,
        Working,
        InputRequired,
        Completed,
        Failed,
        Canceled,
    ] {
        assert_eq!(TaskState::parse(state.as_str()), Some(state));
    }
    assert_eq!(TaskState::parse("done"), None);
    assert_eq!(TaskState::parse(""), None);
}

#[test]
fn only_the_three_end_states_are_terminal() {
    assert!(Completed.is_terminal());
    assert!(Failed.is_terminal());
    assert!(Canceled.is_terminal());
    assert!(!Submitted.is_terminal());
    assert!(!Working.is_terminal());
    assert!(!InputRequired.is_terminal());
}

#[test]
fn a_task_follows_the_normal_path() {
    let state = transition(Submitted, Working).expect("start");
    let state = transition(state, InputRequired).expect("ask");
    let state = transition(state, Working).expect("resume");
    assert_eq!(transition(state, Completed), Ok(Completed));
}

#[test]
fn terminal_states_accept_no_transition() {
    for end in [Completed, Failed, Canceled] {
        for next in [
            Submitted,
            Working,
            InputRequired,
            Completed,
            Failed,
            Canceled,
        ] {
            assert!(transition(end, next).is_err(), "{end} -> {next}");
        }
    }
}

#[test]
fn a_task_cannot_complete_without_work() {
    assert!(transition(Submitted, Completed).is_err());
    assert!(transition(InputRequired, Completed).is_err());
}

#[test]
fn refusal_names_both_states() {
    let err = transition(Completed, Working).expect_err("refused");
    assert_eq!(err, "task cannot move from completed to working");
}
