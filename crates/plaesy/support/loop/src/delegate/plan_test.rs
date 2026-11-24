//! Unit tests for `plan.rs`: topological order, cycle detection, and the run loop.
//! No provider is called. The executor is a closure.

use std::collections::BTreeMap;
use std::time::Duration;

use super::*;
use crate::config::delegate::DelegateProviderConfig;
use crate::delegate::{sleep_for, BuildFn, DelegateError, Provider, ProviderReply};

fn step(id: &str, deps: &[&str]) -> Step {
    Step {
        id: id.to_owned(),
        prompt: format!("prompt {id}"),
        chain: String::new(),
        depends_on: deps.iter().map(|d| (*d).to_owned()).collect(),
    }
}

fn plan(steps: Vec<Step>) -> Plan {
    Plan { steps }
}

fn ids<'a>(order: &[&'a Step]) -> Vec<&'a str> {
    order.iter().map(|s| s.id.as_str()).collect()
}

#[test]
fn order_puts_dependencies_before_dependents() {
    let p = plan(vec![
        step("c", &["a", "b"]),
        step("a", &[]),
        step("b", &["a"]),
    ]);
    let order = p.order().unwrap();
    assert_eq!(ids(&order), vec!["a", "b", "c"]);
}

#[test]
fn order_breaks_ties_by_declaration_not_by_name() {
    let p = plan(vec![step("zeta", &[]), step("alpha", &[]), step("mid", &[])]);
    let order = p.order().unwrap();
    assert_eq!(ids(&order), vec!["zeta", "alpha", "mid"]);
}

#[test]
fn order_runs_an_unblocked_root_before_a_blocked_earlier_step() {
    // "x" waits for "y". "z" has no dependencies. Once "y" is done, "x" is
    // ready again and comes before "z" because it is declared first.
    let p = plan(vec![step("x", &["y"]), step("y", &[]), step("z", &[])]);
    let order = p.order().unwrap();
    assert_eq!(ids(&order), vec!["y", "x", "z"]);
}

#[test]
fn empty_plan_is_valid_and_has_no_steps() {
    let p = plan(Vec::new());
    assert!(p.validate().is_ok());
    assert!(p.order().unwrap().is_empty());
}

#[test]
fn two_step_cycle_is_reported_as_a_closed_loop() {
    let p = plan(vec![step("a", &["b"]), step("b", &["a"])]);
    assert_eq!(
        p.order(),
        Err(PlanError::Cycle(vec![
            "a".to_owned(),
            "b".to_owned(),
            "a".to_owned()
        ]))
    );
}

#[test]
fn self_dependency_is_a_cycle() {
    let p = plan(vec![step("a", &["a"])]);
    assert_eq!(
        p.validate(),
        Err(PlanError::Cycle(vec!["a".to_owned(), "a".to_owned()]))
    );
}

#[test]
fn steps_downstream_of_a_cycle_are_not_reported_as_the_cycle() {
    // "c" depends on the cycle "a" <-> "b" but is not part of it.
    let p = plan(vec![
        step("a", &["b"]),
        step("b", &["a"]),
        step("c", &["a"]),
    ]);
    assert_eq!(
        p.validate(),
        Err(PlanError::Cycle(vec![
            "a".to_owned(),
            "b".to_owned(),
            "a".to_owned()
        ]))
    );
}

#[test]
fn longer_cycle_is_found_through_a_chain_of_dependencies() {
    let p = plan(vec![
        step("a", &["c"]),
        step("b", &["a"]),
        step("c", &["b"]),
        step("root", &[]),
    ]);
    match p.validate() {
        Err(PlanError::Cycle(loop_ids)) => {
            assert_eq!(loop_ids.first(), loop_ids.last());
            assert_eq!(loop_ids.len(), 4);
            for id in ["a", "b", "c"] {
                assert!(loop_ids.iter().any(|x| x == id), "missing {id} in {loop_ids:?}");
            }
            assert!(!loop_ids.iter().any(|x| x == "root"));
        }
        other => panic!("expected a cycle, got {other:?}"),
    }
}

#[test]
fn unknown_dependency_is_an_error() {
    let p = plan(vec![step("a", &["ghost"])]);
    assert_eq!(
        p.validate(),
        Err(PlanError::UnknownDependency {
            step: "a".to_owned(),
            dep: "ghost".to_owned(),
        })
    );
}

#[test]
fn duplicate_id_is_an_error() {
    let p = plan(vec![step("a", &[]), step("a", &[])]);
    assert_eq!(p.validate(), Err(PlanError::DuplicateId("a".to_owned())));
}

#[test]
fn blank_id_is_an_error() {
    let p = plan(vec![step("  ", &[])]);
    assert_eq!(p.validate(), Err(PlanError::EmptyId));
}

#[test]
fn run_passes_only_direct_dependency_outputs_to_each_step() {
    let p = plan(vec![
        step("a", &[]),
        step("b", &[]),
        step("c", &["a"]),
    ]);
    let mut seen: BTreeMap<String, BTreeMap<String, String>> = BTreeMap::new();
    let runs = run(&p, |s: &Step, inputs: &BTreeMap<String, String>| {
        seen.insert(s.id.clone(), inputs.clone());
        Ok(format!("out-{}", s.id))
    })
    .unwrap();

    assert_eq!(
        runs,
        vec![
            StepRun {
                id: "a".to_owned(),
                text: "out-a".to_owned()
            },
            StepRun {
                id: "b".to_owned(),
                text: "out-b".to_owned()
            },
            StepRun {
                id: "c".to_owned(),
                text: "out-c".to_owned()
            },
        ]
    );
    assert!(seen["a"].is_empty());
    assert!(seen["b"].is_empty());
    assert_eq!(seen["c"].len(), 1);
    assert_eq!(seen["c"]["a"], "out-a");
}

#[test]
fn run_stops_at_the_first_failing_step_and_skips_the_rest() {
    let p = plan(vec![
        step("a", &[]),
        step("b", &["a"]),
        step("c", &["b"]),
    ]);
    let mut calls: Vec<String> = Vec::new();
    let result = run(&p, |s: &Step, _inputs: &BTreeMap<String, String>| {
        calls.push(s.id.clone());
        if s.id == "b" {
            Err("provider down".to_owned())
        } else {
            Ok("ok".to_owned())
        }
    });
    assert_eq!(
        result,
        Err(RunError::Step {
            id: "b".to_owned(),
            message: "provider down".to_owned(),
        })
    );
    assert_eq!(calls, vec!["a".to_owned(), "b".to_owned()]);
}

#[test]
fn run_rejects_a_cyclic_plan_before_calling_any_step() {
    let p = plan(vec![step("a", &["b"]), step("b", &["a"])]);
    let mut calls = 0usize;
    let result = run(&p, |_s: &Step, _inputs: &BTreeMap<String, String>| {
        calls += 1;
        Ok(String::new())
    });
    assert!(matches!(result, Err(RunError::Plan(PlanError::Cycle(_)))));
    assert_eq!(calls, 0);
}

/// Answers each prompt with its own text, so a test can see what a step received.
struct Echo;

impl Provider for Echo {
    fn name(&self) -> &str {
        "echo"
    }
    fn complete(&self, req: &Request) -> Result<ProviderReply, DelegateError> {
        Ok(ProviderReply {
            text: format!("echo: {}", req.prompt),
            input_tokens: None,
            output_tokens: None,
        })
    }
}

/// Fails every call with a terminal error.
struct Refuse;

impl Provider for Refuse {
    fn name(&self) -> &str {
        "refuse"
    }
    fn complete(&self, _req: &Request) -> Result<ProviderReply, DelegateError> {
        Err(DelegateError::terminal("bad key"))
    }
}

fn chain_cfg() -> DelegateConfig {
    let mut chains = BTreeMap::new();
    chains.insert("main".to_owned(), vec!["echo".to_owned()]);
    chains.insert("broken".to_owned(), vec!["refuse".to_owned()]);
    DelegateConfig {
        providers: vec![
            DelegateProviderConfig {
                name: "echo".into(),
                kind: "cli".into(),
                enabled: true,
                command: "x".into(),
                ..Default::default()
            },
            DelegateProviderConfig {
                name: "refuse".into(),
                kind: "cli".into(),
                enabled: true,
                command: "x".into(),
                ..Default::default()
            },
        ],
        chains,
    }
}

fn echo_build(_entry: &DelegateProviderConfig) -> Result<Box<dyn Provider>, DelegateError> {
    Ok(Box::new(Echo))
}

fn refuse_build(_entry: &DelegateProviderConfig) -> Result<Box<dyn Provider>, DelegateError> {
    Ok(Box::new(Refuse))
}

fn options<'a>(build: BuildFn<'a>) -> ChainOptions<'a> {
    ChainOptions {
        chain: String::new(),
        confirm: false,
        max_retry_after: Duration::from_secs(60),
        build,
        check: None,
        sleep: &sleep_for,
    }
}

#[test]
fn chain_run_gives_each_step_its_dependencies_output_through_the_chain() {
    let p = plan(vec![step("a", &[]), step("b", &["a"])]);
    let base = Request {
        timeout: Duration::from_secs(5),
        ..Default::default()
    };
    let runs = run_with_chains(&p, &chain_cfg(), "main", &base, &options(&echo_build)).unwrap();
    assert_eq!(runs[0].text, "echo: prompt a");
    assert_eq!(
        runs[1].text,
        "echo: prompt b\n\n--- output of step a ---\necho: prompt a"
    );
}

#[test]
fn chain_run_refuses_an_unknown_chain_before_any_call_is_made() {
    let mut p = plan(vec![step("a", &[]), step("b", &[])]);
    p.steps[1].chain = "missing".to_owned();
    let base = Request::default();
    let result = run_with_chains(&p, &chain_cfg(), "main", &base, &options(&echo_build));
    assert_eq!(
        result,
        Err(RunError::UnknownChain {
            id: "b".to_owned(),
            chain: "missing".to_owned(),
        })
    );
}

#[test]
fn chain_run_reports_a_terminal_provider_failure_as_a_step_error() {
    let mut p = plan(vec![step("a", &[])]);
    p.steps[0].chain = "broken".to_owned();
    let base = Request::default();
    match run_with_chains(&p, &chain_cfg(), "main", &base, &options(&refuse_build)) {
        Err(RunError::Step { id, message }) => {
            assert_eq!(id, "a");
            assert!(message.contains("terminal"), "{message}");
        }
        other => panic!("expected a step error, got {other:?}"),
    }
}
