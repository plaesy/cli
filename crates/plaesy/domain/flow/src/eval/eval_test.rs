use super::*;
use crate::port::{ActionRunner, ApprovalGate, Clock, SubflowRunner};
use serde_json::json;
use std::cell::{Cell, RefCell};
use std::collections::{HashMap, VecDeque};

/// Jam palsu: `sleep` memajukan waktu, dan setiap tidur dicatat.
#[derive(Default)]
struct FakeClock {
    now: Cell<Duration>,
    sleeps: RefCell<Vec<Duration>>,
}

impl Clock for FakeClock {
    fn now(&self) -> Duration {
        self.now.get()
    }

    fn sleep(&self, duration: Duration) {
        self.sleeps.borrow_mut().push(duration);
        self.now.set(self.now.get() + duration);
    }
}

/// Operasi palsu: hasil per nama operasi diambil berurutan. Habis, pakai `Ok({ok, op})`.
#[derive(Default)]
struct FakeActions {
    scripts: RefCell<HashMap<String, VecDeque<Result<Value, ActionError>>>>,
    calls: RefCell<Vec<String>>,
}

impl FakeActions {
    fn script(&self, op: &str, results: Vec<Result<Value, ActionError>>) {
        self.scripts
            .borrow_mut()
            .insert(op.to_owned(), results.into());
    }
}

impl ActionRunner for FakeActions {
    fn run(&self, operation: &str, _input: &Value) -> Result<Value, ActionError> {
        self.calls.borrow_mut().push(operation.to_owned());
        let next = self
            .scripts
            .borrow_mut()
            .get_mut(operation)
            .and_then(VecDeque::pop_front);
        next.unwrap_or_else(|| Ok(json!({ "ok": true, "op": operation })))
    }
}

/// Operasi yang memajukan jam sebesar `cost` setiap dipanggil, untuk uji timeout dan durasi.
struct SlowActions<'a> {
    clock: &'a FakeClock,
    cost: Duration,
}

impl ActionRunner for SlowActions<'_> {
    fn run(&self, _operation: &str, _input: &Value) -> Result<Value, ActionError> {
        self.clock.now.set(self.clock.now.get() + self.cost);
        Ok(json!({}))
    }
}

struct FakeApprovals(ApprovalOutcome);

impl ApprovalGate for FakeApprovals {
    fn await_events(&self, _id: &str, _required: u64, _timeout: Duration) -> ApprovalOutcome {
        self.0.clone()
    }
}

struct FakeSubflows;

impl SubflowRunner for FakeSubflows {
    fn run(&self, flow_id: &str, flow_version: u64, inputs: &Value) -> Result<Value, String> {
        Ok(json!({ "flow": flow_id, "version": flow_version, "inputs": inputs }))
    }
}

fn flow(modules: Value) -> Value {
    json!({
        "ir": "flow-ir/1",
        "id": "flw_test",
        "version": 1,
        "name": "uji",
        "inputs": { "gudang": { "type": "string" } },
        "outputs": { "status": { "type": "string" } },
        "scope": [],
        "modules": modules
    })
}

fn inputs() -> Value {
    json!({ "gudang": "g1" })
}

fn execute_with(
    flow: &Value,
    actions: &dyn ActionRunner,
    clock: &FakeClock,
    approval: ApprovalOutcome,
    run_inputs: &Value,
) -> Result<Outcome, RunError> {
    let approvals = FakeApprovals(approval);
    let ports = Ports {
        actions,
        approvals: &approvals,
        subflows: &FakeSubflows,
        clock,
    };
    run(flow, run_inputs, ports)
}

fn execute(
    flow: &Value,
    actions: &FakeActions,
    clock: &FakeClock,
    approval: ApprovalOutcome,
) -> Result<Outcome, RunError> {
    execute_with(flow, actions, clock, approval, &inputs())
}

fn action(id: &str, op: &str) -> Value {
    json!({ "id": id, "kind": "action", "operation": op, "inputs": { "g": "flow.input.gudang" }, "outputs": {} })
}

#[test]
fn eval_action_result_is_in_scope_and_declared_output_is_returned() {
    let clock = FakeClock::default();
    let actions = FakeActions::default();
    actions.script("cek", vec![Ok(json!({ "status": "paid" }))]);
    let f = flow(json!([
        { "id": "m1", "kind": "action", "operation": "cek", "inputs": { "g": "flow.input.gudang" }, "outputs": { "status": { "type": "string" } } }
    ]));
    let outcome = execute(&f, &actions, &clock, ApprovalOutcome::Rejected).unwrap();
    assert_eq!(outcome.status, Status::Completed);
    assert_eq!(outcome.outputs, json!({ "status": "paid" }));
    assert_eq!(actions.calls.borrow().as_slice(), ["cek"]);
}

#[test]
fn eval_transient_error_is_retried_with_constant_backoff() {
    let clock = FakeClock::default();
    let actions = FakeActions::default();
    actions.script(
        "tarik",
        vec![
            Err(ActionError::transient("timeout", "lambat")),
            Err(ActionError::transient("timeout", "lambat")),
            Ok(json!({ "rows": 2 })),
        ],
    );
    let mut m = action("m1", "tarik");
    m["retry"] = json!({ "kind": "constant", "max_attempts": 5, "base_secs": 2, "retry_if": "error.code == 'timeout'" });
    let outcome = execute(
        &flow(json!([m])),
        &actions,
        &clock,
        ApprovalOutcome::Rejected,
    )
    .unwrap();
    assert_eq!(actions.calls.borrow().len(), 3);
    assert_eq!(
        *clock.sleeps.borrow(),
        vec![Duration::from_secs(2), Duration::from_secs(2)]
    );
    assert_eq!(outcome.scope["m1"], json!({ "rows": 2 }));
}

#[test]
fn eval_exponential_backoff_follows_the_rule_and_respects_cap() {
    let clock = FakeClock::default();
    let actions = FakeActions::default();
    actions.script(
        "tarik",
        vec![
            Err(ActionError::transient("x", "")),
            Err(ActionError::transient("x", "")),
            Err(ActionError::transient("x", "")),
            Err(ActionError::transient("x", "")),
            Ok(json!({})),
        ],
    );
    let mut m = action("m1", "tarik");
    m["retry"] = json!({ "kind": "exponential", "max_attempts": 5, "base_secs": 1, "multiplier": 2, "cap_secs": 3 });
    execute(
        &flow(json!([m])),
        &actions,
        &clock,
        ApprovalOutcome::Rejected,
    )
    .unwrap();
    // Jeda sebelum percobaan ke-2, 3, 4, 5: 1, 2, lalu 4 dan 8 dibatasi cap 3.
    assert_eq!(
        *clock.sleeps.borrow(),
        vec![
            Duration::from_secs(1),
            Duration::from_secs(2),
            Duration::from_secs(3),
            Duration::from_secs(3)
        ]
    );
}

#[test]
fn eval_terminal_error_is_never_retried() {
    let clock = FakeClock::default();
    let actions = FakeActions::default();
    actions.script(
        "tarik",
        vec![Err(ActionError::terminal("auth_failed", "kunci salah"))],
    );
    let mut m = action("m1", "tarik");
    m["retry"] = json!({ "kind": "constant", "max_attempts": 5, "base_secs": 1 });
    let error = execute(
        &flow(json!([m])),
        &actions,
        &clock,
        ApprovalOutcome::Rejected,
    )
    .unwrap_err();
    assert!(
        matches!(error, RunError::Module { ref module, ref error } if module == "m1" && error.code == "auth_failed")
    );
    assert_eq!(actions.calls.borrow().len(), 1);
    assert!(clock.sleeps.borrow().is_empty());
}

#[test]
fn eval_retry_if_false_stops_retrying() {
    let clock = FakeClock::default();
    let actions = FakeActions::default();
    actions.script(
        "tarik",
        vec![
            Err(ActionError::transient("lain", "gangguan")),
            Ok(json!({})),
        ],
    );
    let mut m = action("m1", "tarik");
    m["retry"] = json!({ "kind": "constant", "max_attempts": 5, "base_secs": 1, "retry_if": "error.code == 'timeout'" });
    assert!(execute(
        &flow(json!([m])),
        &actions,
        &clock,
        ApprovalOutcome::Rejected
    )
    .is_err());
    assert_eq!(actions.calls.borrow().len(), 1);
}

#[test]
fn eval_operation_over_timeout_secs_becomes_a_timeout_error() {
    let clock = FakeClock::default();
    let slow = SlowActions {
        clock: &clock,
        cost: Duration::from_secs(5),
    };
    let mut m = action("m1", "lambat");
    m["timeout_secs"] = json!(2);
    let approvals = FakeApprovals(ApprovalOutcome::Rejected);
    let ports = Ports {
        actions: &slow,
        approvals: &approvals,
        subflows: &FakeSubflows,
        clock: &clock,
    };
    let error = run(&flow(json!([m])), &inputs(), ports).unwrap_err();
    assert!(matches!(error, RunError::Module { ref error, .. } if error.code == "timeout"));
}

#[test]
fn eval_on_error_continue_lets_the_flow_go_on() {
    let clock = FakeClock::default();
    let actions = FakeActions::default();
    actions.script("rusak", vec![Err(ActionError::terminal("x", "y"))]);
    let mut m1 = action("m1", "rusak");
    m1["on_error"] = json!("continue");
    let m2 = json!({ "id": "m2", "kind": "pass", "inputs": { "status": "'ok'" }, "outputs": {} });
    let outcome = execute(
        &flow(json!([m1, m2])),
        &actions,
        &clock,
        ApprovalOutcome::Rejected,
    )
    .unwrap();
    assert_eq!(outcome.outputs, json!({ "status": "ok" }));
    assert!(outcome
        .trace
        .iter()
        .any(|s| s.module == "m1" && s.event == "dilanjutkan"));
}

#[test]
fn eval_continue_on_error_flag_is_honoured() {
    let clock = FakeClock::default();
    let actions = FakeActions::default();
    actions.script("rusak", vec![Err(ActionError::terminal("x", "y"))]);
    let mut m1 = action("m1", "rusak");
    m1["continue_on_error"] = json!(true);
    assert!(execute(
        &flow(json!([m1])),
        &actions,
        &clock,
        ApprovalOutcome::Rejected
    )
    .is_ok());
}

#[test]
fn eval_early_return_stops_with_outputs_when_condition_matches() {
    let clock = FakeClock::default();
    let actions = FakeActions::default();
    actions.script(
        "tarik",
        vec![Err(ActionError::terminal("auth_failed", "ditolak"))],
    );
    let mut m1 = action("m1", "tarik");
    m1["early_return"] =
        json!({ "when": "error.code == 'auth_failed'", "outputs": { "status": "'ditolak'" } });
    let m2 = action("m2", "tidak_jalan");
    let outcome = execute(
        &flow(json!([m1, m2])),
        &actions,
        &clock,
        ApprovalOutcome::Rejected,
    )
    .unwrap();
    assert_eq!(outcome.status, Status::Stopped);
    assert_eq!(outcome.outputs, json!({ "status": "ditolak" }));
    assert_eq!(actions.calls.borrow().as_slice(), ["tarik"]);
}

#[test]
fn eval_failure_handler_runs_before_the_error_is_returned() {
    let clock = FakeClock::default();
    let actions = FakeActions::default();
    actions.script("rusak", vec![Err(ActionError::terminal("x", "y"))]);
    let mut f = flow(json!([
        action("m1", "rusak"),
        { "id": "m6", "kind": "action", "operation": "lapor", "inputs": { "code": "error.code" }, "outputs": {} }
    ]));
    f["failure_handler"] = json!("m6");
    assert!(execute(&f, &actions, &clock, ApprovalOutcome::Rejected).is_err());
    assert_eq!(actions.calls.borrow().as_slice(), ["rusak", "lapor"]);
}

#[test]
fn eval_branch_one_takes_the_first_matching_branch_or_default() {
    let clock = FakeClock::default();
    let f = flow(json!([
        { "id": "m1", "kind": "pass", "inputs": { "status": "'draft'" }, "outputs": {} },
        {
            "id": "m2",
            "kind": "branch_one",
            "branches": [{ "when": "m1.status == 'draft'", "modules": ["m3"] }],
            "default": ["m4"]
        },
        action("m3", "draf"),
        action("m4", "lain")
    ]));
    let actions = FakeActions::default();
    execute(&f, &actions, &clock, ApprovalOutcome::Rejected).unwrap();
    assert_eq!(actions.calls.borrow().as_slice(), ["draf"]);

    let mut f2 = f.clone();
    f2["modules"][0]["inputs"]["status"] = json!("'paid'");
    let actions2 = FakeActions::default();
    execute(&f2, &actions2, &clock, ApprovalOutcome::Rejected).unwrap();
    assert_eq!(actions2.calls.borrow().as_slice(), ["lain"]);
}

#[test]
fn eval_branch_one_without_match_and_without_default_is_an_error() {
    let clock = FakeClock::default();
    let actions = FakeActions::default();
    let f = flow(json!([
        { "id": "m2", "kind": "branch_one", "branches": [{ "when": "false", "modules": ["m3"] }] },
        action("m3", "tidak_jalan")
    ]));
    assert!(execute(&f, &actions, &clock, ApprovalOutcome::Rejected).is_err());
    assert!(actions.calls.borrow().is_empty());
}

#[test]
fn eval_branch_all_runs_every_branch_and_honours_on_error() {
    let clock = FakeClock::default();
    let actions = FakeActions::default();
    actions.script("gagal", vec![Err(ActionError::terminal("x", "y"))]);
    let f = flow(json!([
        {
            "id": "m1",
            "kind": "branch_all",
            "branches": [
                { "modules": ["m2"], "on_error": "continue" },
                { "modules": ["m3"], "on_error": "fail" }
            ]
        },
        action("m2", "gagal"),
        action("m3", "ok")
    ]));
    execute(&f, &actions, &clock, ApprovalOutcome::Rejected).unwrap();
    assert_eq!(actions.calls.borrow().as_slice(), ["gagal", "ok"]);
}

#[test]
fn eval_for_each_runs_body_per_item_and_collects_results() {
    let clock = FakeClock::default();
    let actions = FakeActions::default();
    let f = flow(json!([
        {
            "id": "m4",
            "kind": "for_each",
            "iterator": "flow.input.lines",
            "modules": ["m5"],
            "parallel": true,
            "parallelism": 10,
            "skip_failures": false
        },
        { "id": "m5", "kind": "pass", "inputs": { "v": "item.value" }, "outputs": {} }
    ]));
    let run_inputs = json!({ "gudang": "g1", "lines": [1, 2, 3] });
    let outcome =
        execute_with(&f, &actions, &clock, ApprovalOutcome::Rejected, &run_inputs).unwrap();
    assert_eq!(
        outcome.scope["m4"],
        json!([{ "v": 1 }, { "v": 2 }, { "v": 3 }])
    );
    assert!(outcome
        .trace
        .iter()
        .any(|s| s.event == "paralel-dijalankan-berurutan"));
}

#[test]
fn eval_for_each_body_action_runs_once_per_item() {
    let clock = FakeClock::default();
    let actions = FakeActions::default();
    let f = flow(json!([
        { "id": "m4", "kind": "for_each", "iterator": "flow.input.lines", "modules": ["m5"] },
        action("m5", "kirim")
    ]));
    let run_inputs = json!({ "gudang": "g1", "lines": ["a", "b"] });
    execute_with(&f, &actions, &clock, ApprovalOutcome::Rejected, &run_inputs).unwrap();
    assert_eq!(actions.calls.borrow().as_slice(), ["kirim", "kirim"]);
}

#[test]
fn eval_for_each_skip_failures_records_null_and_continues() {
    let clock = FakeClock::default();
    let actions = FakeActions::default();
    actions.script(
        "risk",
        vec![Err(ActionError::terminal("x", "y")), Ok(json!({ "ok": 2 }))],
    );
    let f = flow(json!([
        { "id": "m4", "kind": "for_each", "iterator": "flow.input.lines", "modules": ["m5"], "skip_failures": true },
        action("m5", "risk")
    ]));
    let run_inputs = json!({ "gudang": "g1", "lines": [1, 2] });
    let outcome =
        execute_with(&f, &actions, &clock, ApprovalOutcome::Rejected, &run_inputs).unwrap();
    assert_eq!(outcome.scope["m4"], json!([null, { "ok": 2 }]));
}

#[test]
fn eval_while_runs_body_until_condition_fails() {
    let clock = FakeClock::default();
    let actions = FakeActions::default();
    actions.script(
        "poll",
        vec![
            Ok(json!({ "pending": true })),
            Ok(json!({ "pending": true })),
            Ok(json!({ "pending": false })),
        ],
    );
    let f = flow(json!([
        {
            "id": "m6",
            "kind": "while",
            "condition": "m6.result.pending == true",
            "modules": ["m8"],
            "max_iterations": 10,
            "max_duration_secs": 3600
        },
        action("m8", "poll")
    ]));
    let outcome = execute(&f, &actions, &clock, ApprovalOutcome::Rejected).unwrap();
    assert_eq!(actions.calls.borrow().len(), 3);
    assert_eq!(outcome.scope["m6"]["iterations"], json!(3));
}

#[test]
fn eval_while_stops_at_max_iterations() {
    let clock = FakeClock::default();
    let actions = FakeActions::default();
    let f = flow(json!([
        {
            "id": "m6",
            "kind": "while",
            "condition": "true",
            "modules": ["m8"],
            "max_iterations": 3,
            "max_duration_secs": 3600
        },
        action("m8", "tick")
    ]));
    let error = execute(&f, &actions, &clock, ApprovalOutcome::Rejected).unwrap_err();
    assert!(matches!(error, RunError::Module { ref error, .. } if error.code == "max_iterations"));
    assert_eq!(actions.calls.borrow().len(), 3);
}

#[test]
fn eval_while_stops_at_max_duration() {
    let clock = FakeClock::default();
    // Setiap operasi memakan 10 detik; batas 25 detik berarti iterasi ke-4 ditolak.
    let slow = SlowActions {
        clock: &clock,
        cost: Duration::from_secs(10),
    };
    let f = flow(json!([
        {
            "id": "m6",
            "kind": "while",
            "condition": "true",
            "modules": ["m8"],
            "max_iterations": 1000,
            "max_duration_secs": 25
        },
        action("m8", "lambat")
    ]));
    let approvals = FakeApprovals(ApprovalOutcome::Rejected);
    let ports = Ports {
        actions: &slow,
        approvals: &approvals,
        subflows: &FakeSubflows,
        clock: &clock,
    };
    let error = run(&f, &inputs(), ports).unwrap_err();
    assert!(
        matches!(error, RunError::Module { ref error, .. } if error.code == "max_duration_secs")
    );
}

#[test]
fn eval_approval_approved_records_outcome() {
    let clock = FakeClock::default();
    let actions = FakeActions::default();
    let f = flow(json!([
        { "id": "m3", "kind": "approval", "required_events": 1, "timeout_secs": 86400, "on_timeout": "m7", "on_reject": "m7" },
        { "id": "m7", "kind": "pass", "inputs": { "status": "'ditolak'" }, "outputs": {} }
    ]));
    let outcome = execute(
        &f,
        &actions,
        &clock,
        ApprovalOutcome::Approved { events: 1 },
    )
    .unwrap();
    assert_eq!(
        outcome.scope["m3"],
        json!({ "approved": true, "events": 1 })
    );
    assert!(
        !outcome.scope.contains_key("m7"),
        "jalur penolakan tidak boleh berjalan"
    );
}

#[test]
fn eval_approval_rejected_runs_on_reject_target() {
    let clock = FakeClock::default();
    let actions = FakeActions::default();
    let f = flow(json!([
        { "id": "m3", "kind": "approval", "required_events": 1, "timeout_secs": 60, "on_timeout": "m7", "on_reject": "m7" },
        { "id": "m7", "kind": "pass", "inputs": { "status": "'ditolak'" }, "outputs": {} }
    ]));
    let outcome = execute(&f, &actions, &clock, ApprovalOutcome::Rejected).unwrap();
    assert_eq!(
        outcome.scope["m3"],
        json!({ "approved": false, "reason": "rejected" })
    );
    assert_eq!(outcome.scope["m7"], json!({ "status": "ditolak" }));
}

#[test]
fn eval_approval_timeout_runs_on_timeout_target() {
    let clock = FakeClock::default();
    let actions = FakeActions::default();
    let f = flow(json!([
        { "id": "m3", "kind": "approval", "required_events": 1, "timeout_secs": 60, "on_timeout": "m7", "on_reject": "m8" },
        { "id": "m7", "kind": "pass", "inputs": { "status": "'waktu_habis'" }, "outputs": {} },
        { "id": "m8", "kind": "pass", "inputs": {}, "outputs": {} }
    ]));
    let outcome = execute(&f, &actions, &clock, ApprovalOutcome::TimedOut).unwrap();
    assert_eq!(
        outcome.scope["m3"],
        json!({ "approved": false, "reason": "timeout" })
    );
    assert_eq!(outcome.scope["m7"], json!({ "status": "waktu_habis" }));
    assert!(!outcome.scope.contains_key("m8"));
}

#[test]
fn eval_approval_rejected_without_target_is_an_error() {
    let clock = FakeClock::default();
    let actions = FakeActions::default();
    let f =
        flow(json!([{ "id": "m3", "kind": "approval", "required_events": 1, "timeout_secs": 60 }]));
    assert!(execute(&f, &actions, &clock, ApprovalOutcome::Rejected).is_err());
}

#[test]
fn eval_subflow_result_goes_into_scope() {
    let clock = FakeClock::default();
    let actions = FakeActions::default();
    let f = flow(json!([
        { "id": "m8", "kind": "subflow", "flow_id": "flw_anak", "flow_version": 2, "inputs": { "g": "flow.input.gudang" }, "outputs": {} }
    ]));
    let outcome = execute(&f, &actions, &clock, ApprovalOutcome::Rejected).unwrap();
    assert_eq!(
        outcome.scope["m8"],
        json!({ "flow": "flw_anak", "version": 2, "inputs": { "g": "g1" } })
    );
}

#[test]
fn eval_stop_after_if_stops_the_flow() {
    let clock = FakeClock::default();
    let actions = FakeActions::default();
    let mut m1 = action("m1", "cek");
    m1["stop_after_if"] = json!("flow.input.gudang == 'g1'");
    let outcome = execute(
        &flow(json!([m1, action("m2", "tidak_jalan")])),
        &actions,
        &clock,
        ApprovalOutcome::Rejected,
    )
    .unwrap();
    assert_eq!(outcome.status, Status::Stopped);
    assert_eq!(actions.calls.borrow().as_slice(), ["cek"]);
}

#[test]
fn eval_skip_if_skips_the_module() {
    let clock = FakeClock::default();
    let actions = FakeActions::default();
    let mut m1 = action("m1", "dilewati");
    m1["skip_if"] = json!("flow.input.gudang == 'g1'");
    let outcome = execute(
        &flow(json!([m1])),
        &actions,
        &clock,
        ApprovalOutcome::Rejected,
    )
    .unwrap();
    assert!(actions.calls.borrow().is_empty());
    assert!(!outcome.scope.contains_key("m1"));
}

#[test]
fn eval_rejects_a_document_that_is_not_flow_ir() {
    let clock = FakeClock::default();
    let actions = FakeActions::default();
    let bad = json!({ "ir": "api-ir/1", "modules": [] });
    assert!(matches!(
        execute(&bad, &actions, &clock, ApprovalOutcome::Rejected),
        Err(RunError::Invalid(_))
    ));
}
