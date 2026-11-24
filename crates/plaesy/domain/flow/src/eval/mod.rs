//! Interpreter flow-ir/1 (ADR 0002, aturan eksekusi). Murni: setiap efek lewat [`Ports`].
//!
//! Interpretasi yang saya ambil dari ADR, dan yang belum bisa diverifikasi terhadap skema
//! OpenFlow karena belum saya cek ulang:
//! - `while` menjalankan body minimal sekali, lalu mengulang selama `condition` benar.
//!   Condition membaca hasil terakhir modul `while` itu sendiri (`<id>.result`).
//! - `for_each` berjalan berurutan walau `parallel` bernilai true. `parallelism` dicatat.
//! - `approval` yang ditolak atau timeout menjalankan modul `on_reject` atau `on_timeout`
//!   lalu melanjutkan. Tanpa target, langkah itu gagal.
//! - `stop_after_if` dievaluasi setelah modul sukses. Bila benar, flow berhenti dengan status
//!   `Stopped`.
//! - `early_return` dievaluasi saat modul gagal. Bila benar, flow berhenti dengan outputs dari
//!   `early_return.outputs`.
//! - `timeout_secs` diukur setelah operasi kembali. Operasi yang lewat batas dianggap galat
//!   `timeout` transient. Pemanggilan operasi tidak dihentikan paksa di sini.

pub mod expr;

use crate::port::{ActionError, ApprovalOutcome, ErrorKind, Ports};
use expr::{eval, eval_bool, parse, Env, Expr};
use serde_json::{json, Map, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::time::Duration;

/// Status akhir eksekusi.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Completed,
    Stopped,
}

/// Satu langkah dalam jejak eksekusi.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Step {
    pub module: String,
    pub event: &'static str,
}

/// Hasil eksekusi.
#[derive(Debug, Clone, PartialEq)]
pub struct Outcome {
    pub status: Status,
    pub outputs: Value,
    pub scope: Map<String, Value>,
    pub trace: Vec<Step>,
}

/// Galat eksekusi.
#[derive(Debug, Clone, PartialEq)]
pub enum RunError {
    /// Dokumen bukan flow-ir/1 atau strukturnya rusak.
    Invalid(String),
    /// Modul gagal dan tidak ditangani.
    Module { module: String, error: ActionError },
}

/// Apa yang terjadi setelah sebuah modul atau blok selesai.
enum Flow {
    Continue,
    Stop(Option<Value>),
}

struct Ctx<'a, 'p> {
    defs: BTreeMap<String, &'a Value>,
    ports: Ports<'p>,
    scope: Map<String, Value>,
    loop_items: Vec<Value>,
    inputs: Value,
    done: BTreeSet<String>,
    trace: Vec<Step>,
}

/// Menjalankan `flow` dengan `inputs` sampai selesai.
///
/// # Errors
///
/// [`RunError::Invalid`] bila dokumen bukan flow-ir/1, atau [`RunError::Module`] bila modul
/// gagal dan tidak ditangani oleh `on_error`, `continue_on_error`, atau `early_return`.
pub fn run(flow: &Value, inputs: &Value, ports: Ports<'_>) -> Result<Outcome, RunError> {
    if !crate::ir::is_flow(flow) {
        return Err(RunError::Invalid("dokumen bukan flow-ir/1".to_owned()));
    }
    let modules = flow
        .get("modules")
        .and_then(Value::as_array)
        .ok_or_else(|| RunError::Invalid("modules wajib berupa array".to_owned()))?;
    let mut defs = BTreeMap::new();
    for module in modules {
        let id = module
            .get("id")
            .and_then(Value::as_str)
            .ok_or_else(|| RunError::Invalid("module tanpa id".to_owned()))?;
        defs.insert(id.to_owned(), module);
    }
    let children = child_ids(modules);
    // Target on_reject/on_timeout hanya berjalan lewat jalur approval-nya, tidak berurutan.
    let fallbacks: BTreeSet<&str> = modules
        .iter()
        .flat_map(|m| {
            ["on_reject", "on_timeout"]
                .into_iter()
                .filter_map(move |key| m.get(key).and_then(Value::as_str))
        })
        .collect();
    let top: Vec<String> = modules
        .iter()
        .filter_map(|m| m.get("id").and_then(Value::as_str))
        .filter(|id| !children.contains(*id) && !fallbacks.contains(*id))
        .map(str::to_owned)
        .collect();

    let mut ctx = Ctx {
        defs,
        ports,
        scope: Map::new(),
        loop_items: Vec::new(),
        inputs: inputs.clone(),
        done: BTreeSet::new(),
        trace: Vec::new(),
    };

    let mut stop_outputs: Option<Option<Value>> = None;
    for id in &top {
        match ctx.exec(id) {
            Ok(Flow::Continue) => {}
            Ok(Flow::Stop(outputs)) => {
                stop_outputs = Some(outputs);
                break;
            }
            Err(error) => {
                if let Some(handler) = flow.get("failure_handler").and_then(Value::as_str) {
                    if let RunError::Module {
                        error: action_error,
                        ..
                    } = &error
                    {
                        let _ = ctx.exec_with_error(handler, action_error);
                    }
                }
                return Err(error);
            }
        }
    }

    let (status, outputs) = match stop_outputs {
        Some(Some(explicit)) => (Status::Stopped, explicit),
        Some(None) => (Status::Stopped, declared_outputs(flow, &ctx.scope)),
        None => (Status::Completed, declared_outputs(flow, &ctx.scope)),
    };
    Ok(Outcome {
        status,
        outputs,
        scope: ctx.scope,
        trace: ctx.trace,
    })
}

/// Id modul yang berada di dalam modul lain (cabang, loop, body).
fn child_ids(modules: &[Value]) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    let mut add = |list: Option<&Value>| {
        for id in list
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
        {
            out.insert(id.to_owned());
        }
    };
    for module in modules {
        match module.get("kind").and_then(Value::as_str) {
            Some("branch_one") => {
                for branch in module
                    .get("branches")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                {
                    add(branch.get("modules"));
                }
                add(module.get("default"));
            }
            Some("branch_all") => {
                for branch in module
                    .get("branches")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                {
                    add(branch.get("modules"));
                }
            }
            Some("for_each") | Some("while") => add(module.get("modules")),
            _ => {}
        }
    }
    out
}

/// Outputs yang dideklarasikan flow, diambil dari modul yang menyetelnya (yang terakhir menang).
fn declared_outputs(flow: &Value, scope: &Map<String, Value>) -> Value {
    let mut out = Map::new();
    let names: Vec<String> = flow
        .get("outputs")
        .and_then(Value::as_object)
        .into_iter()
        .flat_map(|o| o.keys().cloned())
        .collect();
    for name in names {
        let value = flow
            .get("modules")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|m| m.get("id").and_then(Value::as_str))
            .filter_map(|id| scope.get(id).and_then(|v| v.get(&name)).cloned())
            .next_back()
            .unwrap_or(Value::Null);
        out.insert(name, value);
    }
    Value::Object(out)
}

impl Ctx<'_, '_> {
    fn env(&self, error: Option<&ActionError>) -> Env {
        let mut env = self.scope.clone();
        env.insert("flow".to_owned(), json!({ "input": self.inputs }));
        if let Some(item) = self.loop_items.last() {
            env.insert("item".to_owned(), json!({ "value": item }));
            env.insert("item_value".to_owned(), item.clone());
        }
        if let Some(e) = error {
            env.insert(
                "error".to_owned(),
                json!({ "code": e.code, "message": e.message, "kind": e.kind.name() }),
            );
        }
        env
    }

    fn eval_value(&self, value: &Value, error: Option<&ActionError>) -> Result<Value, String> {
        match value {
            Value::String(text) => {
                let expr = parse(text)?;
                eval(&expr, &self.env(error))
            }
            other => Ok(other.clone()),
        }
    }

    fn eval_inputs(&self, module: &Value, error: Option<&ActionError>) -> Result<Value, String> {
        let mut out = Map::new();
        for (name, value) in module
            .get("inputs")
            .and_then(Value::as_object)
            .into_iter()
            .flatten()
        {
            out.insert(name.clone(), self.eval_value(value, error)?);
        }
        Ok(Value::Object(out))
    }

    fn check(
        &self,
        module: &str,
        expr_text: &str,
        error: Option<&ActionError>,
    ) -> Result<bool, RunError> {
        let expr: Expr = parse(expr_text).map_err(|m| self.module_err(module, "ekspresi", &m))?;
        eval_bool(&expr, &self.env(error)).map_err(|m| self.module_err(module, "ekspresi", &m))
    }

    fn module_err(&self, module: &str, code: &str, message: &str) -> RunError {
        RunError::Module {
            module: module.to_owned(),
            error: ActionError::terminal(code, message),
        }
    }

    fn event(&mut self, module: &str, event: &'static str) {
        self.trace.push(Step {
            module: module.to_owned(),
            event,
        });
    }

    /// Menjalankan satu modul beserta kebijakan error dan skip-nya.
    fn exec(&mut self, id: &str) -> Result<Flow, RunError> {
        if self.done.contains(id) {
            return Ok(Flow::Continue);
        }
        let module = *self
            .defs
            .get(id)
            .ok_or_else(|| RunError::Invalid(format!("module {id:?} tidak ada")))?;
        if let Some(text) = module.get("skip_if").and_then(Value::as_str) {
            if self.check(id, text, None)? {
                self.event(id, "dilewati");
                self.done.insert(id.to_owned());
                return Ok(Flow::Continue);
            }
        }
        let result = self.run_kind(id, module);
        match result {
            Ok(Flow::Stop(outputs)) => {
                self.done.insert(id.to_owned());
                Ok(Flow::Stop(outputs))
            }
            Ok(Flow::Continue) => {
                self.done.insert(id.to_owned());
                self.event(id, "selesai");
                if let Some(text) = module.get("stop_after_if").and_then(Value::as_str) {
                    if self.check(id, text, None)? {
                        self.event(id, "berhenti");
                        return Ok(Flow::Stop(None));
                    }
                }
                Ok(Flow::Continue)
            }
            Err(error) => self.handle_error(id, module, error),
        }
    }

    fn handle_error(
        &mut self,
        id: &str,
        module: &Value,
        error: RunError,
    ) -> Result<Flow, RunError> {
        let RunError::Module {
            error: action_error,
            ..
        } = &error
        else {
            return Err(error);
        };
        let action_error = action_error.clone();
        if let Some(early) = module.get("early_return") {
            let when = early.get("when").and_then(Value::as_str).unwrap_or("false");
            if self.check(id, when, Some(&action_error))? {
                self.event(id, "berhenti");
                self.done.insert(id.to_owned());
                let mut outputs = Map::new();
                for (name, value) in early
                    .get("outputs")
                    .and_then(Value::as_object)
                    .into_iter()
                    .flatten()
                {
                    outputs.insert(
                        name.clone(),
                        self.eval_value(value, Some(&action_error))
                            .map_err(|m| self.module_err(id, "early_return", &m))?,
                    );
                }
                return Ok(Flow::Stop(Some(Value::Object(outputs))));
            }
        }
        let continue_on_error = module
            .get("continue_on_error")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        match module.get("on_error").and_then(Value::as_str) {
            _ if continue_on_error => {}
            Some("continue") => {}
            Some("fail") | None => return Err(error),
            Some(handler) => {
                self.event(id, "gagal");
                self.exec_with_error(handler, &action_error)?;
            }
        }
        self.done.insert(id.to_owned());
        self.event(id, "dilanjutkan");
        Ok(Flow::Continue)
    }

    /// Menjalankan modul penanganan dengan `error` tersedia di ekspresi.
    fn exec_with_error(&mut self, handler: &str, error: &ActionError) -> Result<Flow, RunError> {
        let module = *self
            .defs
            .get(handler)
            .ok_or_else(|| RunError::Invalid(format!("penanganan {handler:?} tidak ada")))?;
        let result = self.run_kind_with(handler, module, Some(error));
        match result {
            Ok(produced) => {
                self.scope
                    .insert(handler.to_owned(), produced_value(produced));
                self.done.insert(handler.to_owned());
                self.event(handler, "selesai");
                Ok(Flow::Continue)
            }
            Err(e) => Err(e),
        }
    }

    fn run_kind(&mut self, id: &str, module: &Value) -> Result<Flow, RunError> {
        self.run_kind_with(id, module, None).map(|p| match p {
            Produced::Value(v) => {
                self.scope.insert(id.to_owned(), v);
                Flow::Continue
            }
        })
    }

    fn run_kind_with(
        &mut self,
        id: &str,
        module: &Value,
        error: Option<&ActionError>,
    ) -> Result<Produced, RunError> {
        let kind = module.get("kind").and_then(Value::as_str).unwrap_or("");
        match kind {
            "action" => self.run_action(id, module, error).map(Produced::Value),
            "pass" => {
                let inputs = self
                    .eval_inputs(module, error)
                    .map_err(|m| self.module_err(id, "pass", &m))?;
                Ok(Produced::Value(inputs))
            }
            "subflow" => self.run_subflow(id, module, error).map(Produced::Value),
            "branch_one" => self
                .run_branch_one(id, module)
                .map(|_| Produced::Value(Value::Null)),
            "branch_all" => self
                .run_branch_all(id, module)
                .map(|_| Produced::Value(Value::Null)),
            "for_each" => self.run_for_each(id, module).map(Produced::Value),
            "while" => self.run_while(id, module).map(Produced::Value),
            "approval" => self.run_approval(id, module).map(Produced::Value),
            other => Err(self.module_err(
                id,
                "kind",
                &format!("kind {other:?} tidak didukung interpreter"),
            )),
        }
    }

    fn run_action(
        &mut self,
        id: &str,
        module: &Value,
        error: Option<&ActionError>,
    ) -> Result<Value, RunError> {
        let operation = module
            .get("operation")
            .and_then(Value::as_str)
            .ok_or_else(|| self.module_err(id, "operation", "operation wajib"))?;
        let input = self
            .eval_inputs(module, error)
            .map_err(|m| self.module_err(id, "input", &m))?;
        let retry = module.get("retry");
        let max_attempts = retry
            .and_then(|r| r.get("max_attempts"))
            .and_then(Value::as_u64)
            .unwrap_or(1)
            .max(1);
        let timeout = module
            .get("timeout_secs")
            .and_then(Value::as_u64)
            .map(Duration::from_secs);
        let mut attempt = 1u64;
        loop {
            let started = self.ports.clock.now();
            let outcome = self.ports.actions.run(operation, &input);
            let elapsed = self.ports.clock.now().saturating_sub(started);
            let outcome = match (outcome, timeout) {
                (Ok(_), Some(limit)) if elapsed > limit => Err(ActionError::transient(
                    "timeout",
                    "operasi melewati timeout_secs",
                )),
                (other, _) => other,
            };
            match outcome {
                Ok(value) => {
                    self.event(id, "berhasil");
                    return Ok(value);
                }
                Err(action_error) => {
                    let retryable =
                        action_error.kind == ErrorKind::Transient && attempt < max_attempts;
                    let allowed = match retry
                        .and_then(|r| r.get("retry_if"))
                        .and_then(Value::as_str)
                    {
                        Some(text) => self.check(id, text, Some(&action_error))?,
                        None => true,
                    };
                    if !(retryable && allowed) {
                        return Err(RunError::Module {
                            module: id.to_owned(),
                            error: action_error,
                        });
                    }
                    self.event(id, "dicoba-ulang");
                    self.ports.clock.sleep(backoff(retry, attempt + 1));
                    attempt += 1;
                }
            }
        }
    }

    fn run_subflow(
        &mut self,
        id: &str,
        module: &Value,
        error: Option<&ActionError>,
    ) -> Result<Value, RunError> {
        let flow_id = module
            .get("flow_id")
            .and_then(Value::as_str)
            .ok_or_else(|| self.module_err(id, "flow_id", "flow_id wajib"))?;
        let version = module
            .get("flow_version")
            .and_then(Value::as_u64)
            .unwrap_or(1);
        let inputs = self
            .eval_inputs(module, error)
            .map_err(|m| self.module_err(id, "input", &m))?;
        self.ports
            .subflows
            .run(flow_id, version, &inputs)
            .map_err(|m| RunError::Module {
                module: id.to_owned(),
                error: ActionError::terminal("subflow_failed", &m),
            })
    }

    fn run_branch_one(&mut self, id: &str, module: &Value) -> Result<(), RunError> {
        for branch in module
            .get("branches")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            let when = branch
                .get("when")
                .and_then(Value::as_str)
                .unwrap_or("false");
            if self.check(id, when, None)? {
                self.run_block(branch.get("modules"))?;
                self.event(id, "cabang-dipilih");
                return Ok(());
            }
        }
        match module.get("default") {
            Some(default) => {
                self.run_block(Some(default))?;
                self.event(id, "cabang-default");
                Ok(())
            }
            None => Err(self.module_err(id, "default", "cabang default wajib")),
        }
    }

    fn run_branch_all(&mut self, id: &str, module: &Value) -> Result<(), RunError> {
        for branch in module
            .get("branches")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            match self.run_block(branch.get("modules")) {
                Ok(()) => {}
                Err(error) => {
                    if branch.get("on_error").and_then(Value::as_str) == Some("continue") {
                        self.event(id, "cabang-dilanjutkan");
                    } else {
                        return Err(error);
                    }
                }
            }
        }
        self.event(id, "semua-cabang-selesai");
        Ok(())
    }

    /// Menjalankan daftar id berurutan. `Stop` dari dalam blok dinaikkan sebagai galat berhenti
    /// yang ditangani pemanggil lewat `Flow`; di sini cukup dilaporkan sebagai berhenti.
    fn run_block(&mut self, ids: Option<&Value>) -> Result<(), RunError> {
        for id in ids
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
        {
            match self.exec(id)? {
                Flow::Continue => {}
                Flow::Stop(_) => {
                    return Err(RunError::Invalid(
                        "early_return di dalam blok tidak didukung".to_owned(),
                    ))
                }
            }
        }
        Ok(())
    }

    fn run_for_each(&mut self, id: &str, module: &Value) -> Result<Value, RunError> {
        let iterator = module
            .get("iterator")
            .and_then(Value::as_str)
            .ok_or_else(|| self.module_err(id, "iterator", "iterator wajib"))?;
        let items = self
            .eval_value(&Value::String(iterator.to_owned()), None)
            .map_err(|m| self.module_err(id, "iterator", &m))?;
        let items = items
            .as_array()
            .cloned()
            .ok_or_else(|| self.module_err(id, "iterator", "iterator bukan array"))?;
        let skip_failures = module
            .get("skip_failures")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let body: Vec<String> = module
            .get("modules")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .map(str::to_owned)
            .collect();
        if module
            .get("parallel")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        {
            self.event(id, "paralel-dijalankan-berurutan");
        }
        let mut results = Vec::with_capacity(items.len());
        for item in items {
            self.loop_items.push(item);
            let outcome = self.run_body(&body);
            self.loop_items.pop();
            match outcome {
                Ok(last) => results.push(last),
                Err(error) => {
                    if skip_failures {
                        results.push(Value::Null);
                    } else {
                        return Err(error);
                    }
                }
            }
        }
        Ok(Value::Array(results))
    }

    fn run_body(&mut self, body: &[String]) -> Result<Value, RunError> {
        // Setiap iterasi menjalankan ulang body; tanda `done` harus dihapus lebih dulu.
        for id in body {
            self.done.remove(id);
            self.exec(id)?;
        }
        Ok(body
            .last()
            .and_then(|id| self.scope.get(id))
            .cloned()
            .unwrap_or(Value::Null))
    }

    fn run_while(&mut self, id: &str, module: &Value) -> Result<Value, RunError> {
        let max_iterations = module
            .get("max_iterations")
            .and_then(Value::as_u64)
            .ok_or_else(|| self.module_err(id, "max_iterations", "max_iterations wajib"))?;
        let max_duration = module
            .get("max_duration_secs")
            .and_then(Value::as_u64)
            .map(Duration::from_secs)
            .ok_or_else(|| self.module_err(id, "max_duration_secs", "max_duration_secs wajib"))?;
        let condition = module
            .get("condition")
            .and_then(Value::as_str)
            .ok_or_else(|| self.module_err(id, "condition", "condition wajib"))?;
        let body: Vec<String> = module
            .get("modules")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .map(str::to_owned)
            .collect();
        let started = self.ports.clock.now();
        let mut iterations = 0u64;
        loop {
            if iterations >= max_iterations {
                return Err(self.module_err(id, "max_iterations", "batas max_iterations tercapai"));
            }
            if self.ports.clock.now().saturating_sub(started) >= max_duration {
                return Err(self.module_err(
                    id,
                    "max_duration_secs",
                    "batas max_duration_secs tercapai",
                ));
            }
            let last = self.run_body(&body)?;
            iterations += 1;
            self.scope.insert(id.to_owned(), json!({ "result": last }));
            if !self.check(id, condition, None)? {
                break;
            }
        }
        self.event(id, "loop-selesai");
        Ok(
            json!({ "result": self.scope.get(id).and_then(|v| v.get("result")).cloned().unwrap_or(Value::Null), "iterations": iterations }),
        )
    }

    fn run_approval(&mut self, id: &str, module: &Value) -> Result<Value, RunError> {
        let required = module
            .get("required_events")
            .and_then(Value::as_u64)
            .unwrap_or(1);
        let timeout = module
            .get("timeout_secs")
            .and_then(Value::as_u64)
            .map(Duration::from_secs)
            .ok_or_else(|| self.module_err(id, "timeout_secs", "timeout_secs wajib"))?;
        match self.ports.approvals.await_events(id, required, timeout) {
            ApprovalOutcome::Approved { events } => {
                self.event(id, "disetujui");
                Ok(json!({ "approved": true, "events": events }))
            }
            ApprovalOutcome::Rejected => {
                self.event(id, "ditolak");
                self.run_fallback(id, module, "on_reject")?;
                Ok(json!({ "approved": false, "reason": "rejected" }))
            }
            ApprovalOutcome::TimedOut => {
                self.event(id, "timeout");
                self.run_fallback(id, module, "on_timeout")?;
                Ok(json!({ "approved": false, "reason": "timeout" }))
            }
        }
    }

    fn run_fallback(&mut self, id: &str, module: &Value, key: &str) -> Result<(), RunError> {
        match module.get(key).and_then(Value::as_str) {
            Some(target) => match self.exec(target)? {
                Flow::Continue => Ok(()),
                Flow::Stop(_) => Err(RunError::Invalid(
                    "early_return dari modul fallback tidak didukung".to_owned(),
                )),
            },
            None => Err(self.module_err(
                id,
                key,
                &format!("{key} wajib saat persetujuan tidak lolos"),
            )),
        }
    }
}

enum Produced {
    Value(Value),
}

fn produced_value(p: Produced) -> Value {
    match p {
        Produced::Value(v) => v,
    }
}

/// Jeda sebelum percobaan ke-`attempt` (mulai dari 2). Konstanta atau eksponensial dengan batas.
fn backoff(retry: Option<&Value>, attempt: u64) -> Duration {
    let base = retry
        .and_then(|r| r.get("base_secs"))
        .and_then(Value::as_f64)
        .unwrap_or(1.0);
    let cap = retry
        .and_then(|r| r.get("cap_secs"))
        .and_then(Value::as_f64)
        .unwrap_or(60.0);
    let kind = retry
        .and_then(|r| r.get("kind"))
        .and_then(Value::as_str)
        .unwrap_or("constant");
    let secs = if kind == "exponential" {
        let multiplier = retry
            .and_then(|r| r.get("multiplier"))
            .and_then(Value::as_f64)
            .unwrap_or(2.0);
        base * multiplier.powi(i32::try_from(attempt.saturating_sub(2)).unwrap_or(i32::MAX))
    } else {
        base
    };
    Duration::from_secs_f64(secs.min(cap).max(0.0))
}

#[cfg(test)]
#[path = "eval_test.rs"]
mod eval_test;

#[cfg(test)]
#[path = "expr_test.rs"]
mod expr_test;
