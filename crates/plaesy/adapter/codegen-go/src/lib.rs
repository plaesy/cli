//! Codegen flow-ir/1 ke paket Go (ADR 0003: codegen-go). Cakupan dan perilaku sama dengan
//! `plaesy-adapter-codegen-js`:
//! - Modul `action` (dengan retry dan timeout) dan `pass` diterjemahkan. `action` dengan
//!   `on_error` selain `fail` belum didukung.
//! - `for_each` diterjemahkan sebagai loop berurutan. `parallel` dan `parallelism` hanya dicatat
//!   sebagai komentar. `skip_failures: true` belum didukung. Anggota harus `action` atau `pass`.
//! - Ekspresi input: path (`flow.input.x`, `mID.path`), literal string, dan `item.value` di dalam
//!   `for_each`.
//! - Modul lain dan ekspresi di luar cakupan dicatat sebagai kehilangan, dan kode yang
//!   dihasilkan mengembalikan galat di titik itu. Tidak ada yang dilewati diam-diam.
//!
//! Kode yang dihasilkan memakai tipe `Context`, `ActionOptions`, dan `RetryPolicy` di paket
//! yang sama. Tipe itu dibuat oleh runtime, bukan oleh generator ini.

use plaesy_domain_flow::validate as validate_flow;
use serde_json::Value;
use std::collections::BTreeSet;
use std::fmt::Write as _;

/// Modul yang diterjemahkan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mapping {
    pub module: String,
    pub kind: String,
}

/// Modul yang tidak diterjemahkan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Loss {
    pub module: String,
    pub kind: String,
    pub reason: String,
}

/// Laporan codegen.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Report {
    pub mapped: Vec<Mapping>,
    pub lost: Vec<Loss>,
}

/// Hasil codegen: kode Go dan laporannya.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Generated {
    pub source: String,
    pub report: Report,
}

/// Galat: dokumen tidak valid.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodegenError(pub Vec<String>);

impl std::fmt::Display for CodegenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "flow tidak valid: {}", self.0.join("; "))
    }
}

impl std::error::Error for CodegenError {}

/// Membuat paket Go dari flow-ir/1 `doc`. Paket bernama `flows`.
///
/// # Errors
///
/// [`CodegenError`] bila dokumen melanggar aturan semantik flow-ir.
pub fn generate(doc: &Value) -> Result<Generated, CodegenError> {
    let errors: Vec<String> = validate_flow(doc).iter().map(ToString::to_string).collect();
    if !errors.is_empty() {
        return Err(CodegenError(errors));
    }

    let name = doc.get("name").and_then(Value::as_str).unwrap_or("flow");
    let function = to_pascal(name);
    let all: Vec<Value> = doc
        .get("modules")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    // Anggota for_each dikerjakan oleh induknya, bukan di tingkat atas.
    let members: BTreeSet<String> = all.iter().flat_map(|m| nested_ids(&all, m)).collect();
    let mut report = Report::default();
    let mut body = String::new();

    for module in &all {
        let id = str_field(module, "id");
        let kind = str_field(module, "kind");
        if members.contains(&id) {
            continue;
        }
        if kind == "branch_one" {
            match emit_branch(&id, module, &all) {
                Ok((code, outcomes)) => {
                    let _ = write!(body, "{code}");
                    report.mapped.push(Mapping {
                        module: id.clone(),
                        kind: kind.clone(),
                    });
                    for outcome in outcomes {
                        match outcome.error {
                            None => report.mapped.push(Mapping {
                                module: outcome.id,
                                kind: outcome.kind,
                            }),
                            Some(reason) => report.lost.push(Loss {
                                module: outcome.id,
                                kind: outcome.kind,
                                reason,
                            }),
                        }
                    }
                }
                Err(reason) => {
                    let _ = writeln!(
                        body,
                        "\t// {id} ({kind}): belum didukung codegen-go\n\treturn nil, fmt.Errorf({})",
                        go_string(&format!("modul {id} ({kind}) belum didukung: {reason}"))
                    );
                    for nested in nested_ids(&all, module) {
                        report.lost.push(Loss {
                            module: nested.clone(),
                            kind: kind_of(&all, &nested),
                            reason: format!("anggota {id}: {reason}"),
                        });
                    }
                    report.lost.push(Loss {
                        module: id,
                        kind,
                        reason,
                    });
                }
            }
            continue;
        }
        match emit_module(&id, &kind, module, &all, false) {
            Ok(code) => {
                let _ = write!(body, "{code}");
                report.mapped.push(Mapping {
                    module: id.clone(),
                    kind: kind.clone(),
                });
                for child in string_list(module, "modules") {
                    if let Some(child_module) = find(&all, &child) {
                        report.mapped.push(Mapping {
                            module: child,
                            kind: str_field(child_module, "kind"),
                        });
                    }
                }
            }
            Err(reason) => {
                let _ = writeln!(
                    body,
                    "\t// {id} ({kind}): belum didukung codegen-go\n\treturn nil, fmt.Errorf({})",
                    go_string(&format!("modul {id} ({kind}) belum didukung: {reason}"))
                );
                for child in string_list(module, "modules") {
                    if let Some(child_module) = find(&all, &child) {
                        report.lost.push(Loss {
                            module: child,
                            kind: str_field(child_module, "kind"),
                            reason: format!("anggota {id}: {reason}"),
                        });
                    }
                }
                report.lost.push(Loss {
                    module: id,
                    kind,
                    reason,
                });
            }
        }
    }

    let source = format!(
        "// Dibuat dari flow-ir/1 {name:?} ({}). Jangan diedit manual.\n\
         package flows\n\n\
         import \"fmt\"\n\n\
         var _ = fmt.Sprintf\n\n\
         func {function}(input map[string]any, ctx Context) (map[string]any, error) {{\n\
         \tvar err error\n\
         \t_ = err\n\
         \tscope := map[string]any{{}}\n\
         {body}\treturn scope, nil\n}}\n\n\
         func lookup(root map[string]any, keys ...string) any {{\n\
         \tvar cur any = root\n\
         \tfor _, k := range keys {{\n\
         \t\tm, ok := cur.(map[string]any)\n\
         \t\tif !ok {{\n\
         \t\t\treturn nil\n\
         \t\t}}\n\
         \t\tcur = m[k]\n\
         \t}}\n\
         \treturn cur\n}}\n\n\
         func lookupList(root map[string]any, keys ...string) []any {{\n\
         \tv, _ := lookup(root, keys...).([]any)\n\
         \treturn v\n}}\n",
        doc.get("id").and_then(Value::as_str).unwrap_or("?"),
    );
    dedupe(&mut report);
    Ok(Generated { source, report })
}

fn emit_module(
    id: &str,
    kind: &str,
    module: &Value,
    all: &[Value],
    in_loop: bool,
) -> Result<String, String> {
    match kind {
        "pass" => {
            let mut fields = Vec::new();
            for (name, expr) in module
                .get("inputs")
                .and_then(Value::as_object)
                .into_iter()
                .flatten()
            {
                fields.push(format!(
                    "{}: {}",
                    go_string(name),
                    expression(expr.as_str().unwrap_or(""), in_loop)?
                ));
            }
            Ok(format!(
                "\tscope[{}] = map[string]any{{{}}}\n",
                go_string(id),
                fields.join(", ")
            ))
        }
        "action" => {
            let operation = module
                .get("operation")
                .and_then(Value::as_str)
                .ok_or_else(|| "operation wajib".to_owned())?;
            if let Some(on_error) = module.get("on_error").and_then(Value::as_str) {
                if on_error != "fail" {
                    return Err(format!("on_error {on_error:?} belum didukung"));
                }
            }
            let mut inputs = Vec::new();
            for (name, expr) in module
                .get("inputs")
                .and_then(Value::as_object)
                .into_iter()
                .flatten()
            {
                inputs.push(format!(
                    "{}: {}",
                    go_string(name),
                    expression(expr.as_str().unwrap_or(""), in_loop)?
                ));
            }
            let timeout = module
                .get("timeout_secs")
                .and_then(Value::as_u64)
                .unwrap_or(0);
            let call = format!(
                "ctx.Action({}, map[string]any{{{}}}, ActionOptions{{TimeoutSecs: {timeout}}})",
                go_string(operation),
                inputs.join(", ")
            );
            let call = match module.get("retry") {
                Some(retry) => format!(
                    "ctx.WithRetry({}, func() (any, error) {{ return {call} }})",
                    retry_policy(retry)
                ),
                None => call,
            };
            Ok(format!(
                "\tscope[{}], err = {call}\n\tif err != nil {{\n\t\treturn nil, fmt.Errorf({}, err)\n\t}}\n",
                go_string(id),
                go_string(&format!("modul {id}: %w"))
            ))
        }
        "for_each" => {
            if in_loop {
                return Err("for_each bersarang belum didukung".to_owned());
            }
            emit_for_each(id, module, all)
        }
        other => Err(format!("kind {other:?} di luar cakupan codegen-go")),
    }
}

/// `for_each` menjadi loop berurutan. Anggota dihasilkan di dalam loop.
fn emit_for_each(id: &str, module: &Value, all: &[Value]) -> Result<String, String> {
    if module
        .get("skip_failures")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        return Err("skip_failures: true belum didukung".to_owned());
    }
    let iterator = module
        .get("iterator")
        .and_then(Value::as_str)
        .ok_or_else(|| "iterator wajib".to_owned())?;
    let source = expression(iterator, false)?;
    let Some(list) = source.strip_prefix("lookup(") else {
        return Err(format!("iterator {iterator:?} harus path"));
    };
    let list = format!("lookupList({list}");
    let mut out = String::new();
    let _ = writeln!(out, "\tscope[{}] = []any{{}}", go_string(id));
    if module
        .get("parallel")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        let _ = writeln!(out, "\t// {id}: parallel: true dijalankan berurutan");
    }
    if let Some(p) = module.get("parallelism").and_then(Value::as_u64) {
        let _ = writeln!(
            out,
            "\t// {id}: parallelism {p} dicatat, tidak dipakai (berurutan)"
        );
    }
    let _ = writeln!(out, "\tfor _, item := range {list} {{");
    let _ = writeln!(out, "\t\t_ = item");
    let members = string_list(module, "modules");
    if members.is_empty() {
        return Err("for_each tanpa anggota".to_owned());
    }
    for member in members {
        let child =
            find(all, &member).ok_or_else(|| format!("anggota {member} tidak ditemukan"))?;
        let child_kind = str_field(child, "kind");
        let code = emit_module(&member, &child_kind, child, all, true)
            .map_err(|reason| format!("anggota {member}: {reason}"))?;
        out.push_str(&indent(&code, "\t"));
    }
    out.push_str("\t}\n");
    Ok(out)
}

/// Ekspresi path, literal string, atau `item.value` (di dalam loop) menjadi Go.
fn expression(text: &str, in_loop: bool) -> Result<String, String> {
    let text = text.trim();
    if let Some(inner) = text.strip_prefix('\'').and_then(|s| s.strip_suffix('\'')) {
        return Ok(go_string(inner));
    }
    if in_loop && text == "item.value" {
        return Ok("item".to_owned());
    }
    let segments: Vec<&str> = text.split('.').collect();
    if segments.len() < 2 || segments.iter().any(|s| !is_identifier(s)) {
        return Err(format!("ekspresi {text:?} di luar cakupan"));
    }
    if segments[0] == "flow" && segments[1] == "input" {
        return Ok(lookup_call("input", &segments[2..]));
    }
    if segments[0].starts_with('m') && segments[0][1..].chars().all(|c| c.is_ascii_digit()) {
        return Ok(lookup_call("scope", &segments));
    }
    Err(format!("ekspresi {text:?} di luar cakupan"))
}

fn lookup_call(root: &str, keys: &[&str]) -> String {
    let quoted: Vec<String> = keys.iter().map(|k| go_string(k)).collect();
    format!("lookup({root}, {})", quoted.join(", "))
}

fn retry_policy(retry: &Value) -> String {
    format!(
        "RetryPolicy{{Kind: {}, MaxAttempts: {}, BaseSecs: {}, Multiplier: {}, CapSecs: {}}}",
        go_string(retry.get("kind").and_then(Value::as_str).unwrap_or("")),
        retry
            .get("max_attempts")
            .and_then(Value::as_u64)
            .unwrap_or(1),
        retry.get("base_secs").and_then(Value::as_u64).unwrap_or(0),
        retry.get("multiplier").and_then(Value::as_u64).unwrap_or(1),
        retry.get("cap_secs").and_then(Value::as_u64).unwrap_or(0),
    )
}

/// Anggota langsung yang dikerjakan oleh induknya, bukan di tingkat atas.
fn members_of(module: &Value) -> Vec<String> {
    match module.get("kind").and_then(Value::as_str) {
        Some("for_each") => string_list(module, "modules"),
        Some("branch_one") => branch_children(module),
        _ => Vec::new(),
    }
}

fn branch_children(module: &Value) -> Vec<String> {
    let mut out: Vec<String> = module
        .get("branches")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .flat_map(|b| string_list(b, "modules"))
        .collect();
    out.extend(string_list(module, "default"));
    out
}

/// Semua anggota turunan, rekursif. Dipakai agar setiap modul tercatat tepat sekali.
fn nested_ids(all: &[Value], module: &Value) -> Vec<String> {
    let mut out = Vec::new();
    for member in members_of(module) {
        if let Some(child) = find(all, &member) {
            out.extend(nested_ids(all, child));
        }
        out.push(member);
    }
    out
}

fn kind_of(all: &[Value], id: &str) -> String {
    find(all, id)
        .map(|m| str_field(m, "kind"))
        .unwrap_or_else(|| "?".to_owned())
}

/// Hasil satu anggota cabang. `error` None berarti diterjemahkan.
struct ChildOutcome {
    id: String,
    kind: String,
    error: Option<String>,
}

/// `branch_one` menjadi if/else berurutan. Modul yang tidak didukung di dalam cabang
/// mengembalikan galat di titik itu. Modul lain di cabang yang sama tetap diterjemahkan.
fn emit_branch(
    _id: &str,
    module: &Value,
    all: &[Value],
) -> Result<(String, Vec<ChildOutcome>), String> {
    let branches: Vec<Value> = module
        .get("branches")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut out = String::new();
    let mut outcomes = Vec::new();
    for (index, branch) in branches.iter().enumerate() {
        let condition = condition(&str_field(branch, "when"))?;
        let keyword = if index == 0 { "if" } else { "} else if" };
        let _ = writeln!(out, "\t{keyword} {condition} {{");
        emit_children(
            &mut out,
            &string_list(branch, "modules"),
            all,
            &mut outcomes,
        );
    }
    let default = string_list(module, "default");
    if !default.is_empty() {
        if branches.is_empty() {
            out.push_str("\t{\n");
        } else {
            out.push_str("\t} else {\n");
        }
        emit_children(&mut out, &default, all, &mut outcomes);
    }
    if !branches.is_empty() || !default.is_empty() {
        out.push_str("\t}\n");
    }
    Ok((out, outcomes))
}

fn emit_children(
    out: &mut String,
    members: &[String],
    all: &[Value],
    outcomes: &mut Vec<ChildOutcome>,
) {
    for member in members {
        let Some(child) = find(all, member) else {
            let _ = writeln!(
                out,
                "\t\treturn nil, fmt.Errorf({})",
                go_string(&format!("anggota {member} tidak ditemukan"))
            );
            outcomes.push(ChildOutcome {
                id: member.clone(),
                kind: "?".to_owned(),
                error: Some("anggota tidak ditemukan".to_owned()),
            });
            continue;
        };
        let kind = str_field(child, "kind");
        match emit_module(member, &kind, child, all, false) {
            Ok(code) => {
                out.push_str(&indent(&code, "\t"));
                outcomes.push(ChildOutcome {
                    id: member.clone(),
                    kind,
                    error: None,
                });
                for nested in nested_ids(all, child) {
                    outcomes.push(ChildOutcome {
                        kind: kind_of(all, &nested),
                        id: nested,
                        error: None,
                    });
                }
            }
            Err(reason) => {
                let _ = writeln!(
                    out,
                    "\t\t// {member} ({kind}): belum didukung codegen-go\n\t\treturn nil, fmt.Errorf({})",
                    go_string(&format!("modul {member} ({kind}) belum didukung: {reason}"))
                );
                for nested in nested_ids(all, child) {
                    outcomes.push(ChildOutcome {
                        kind: kind_of(all, &nested),
                        id: nested,
                        error: Some(format!("anggota {member}: {reason}")),
                    });
                }
                outcomes.push(ChildOutcome {
                    id: member.clone(),
                    kind,
                    error: Some(reason),
                });
            }
        }
    }
}

/// Kondisi `when` dengan bentuk `<ekspresi> == <literal>` atau `!=`.
fn condition(when: &str) -> Result<String, String> {
    let unsupported = || format!("kondisi {when:?} di luar cakupan");
    let (left, op, right) = split_comparison(when).ok_or_else(unsupported)?;
    let left = expression(left, false).map_err(|_| unsupported())?;
    let right = literal(right).ok_or_else(unsupported)?;
    Ok(format!("{left} {op} {right}"))
}

fn split_comparison(when: &str) -> Option<(&str, &str, &str)> {
    ["==", "!="].into_iter().find_map(|op| {
        when.split_once(&format!(" {op} "))
            .map(|(left, right)| (left.trim(), op, right.trim()))
    })
}

fn literal(text: &str) -> Option<String> {
    if let Some(inner) = text.strip_prefix('\'').and_then(|s| s.strip_suffix('\'')) {
        return Some(go_string(inner));
    }
    let numeric = !text.is_empty()
        && text
            .chars()
            .all(|c| c.is_ascii_digit() || matches!(c, '.' | '-' | '+' | 'e' | 'E'))
        && text.parse::<f64>().is_ok();
    if numeric || text == "true" || text == "false" {
        return Some(text.to_owned());
    }
    None
}

/// Setiap modul hanya tercatat sekali (kemunculan pertama).
fn dedupe(report: &mut Report) {
    let mut seen = BTreeSet::new();
    report.mapped.retain(|m| seen.insert(m.module.clone()));
    let mut seen = BTreeSet::new();
    report.lost.retain(|l| seen.insert(l.module.clone()));
}

fn str_field(value: &Value, key: &str) -> String {
    value
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or("?")
        .to_owned()
}

fn string_list(value: &Value, key: &str) -> Vec<String> {
    value
        .get(key)
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|v| v.as_str().map(str::to_owned))
        .collect()
}

fn find<'a>(all: &'a [Value], id: &str) -> Option<&'a Value> {
    all.iter()
        .find(|m| m.get("id").and_then(Value::as_str) == Some(id))
}

fn indent(code: &str, pad: &str) -> String {
    code.lines().map(|line| format!("{pad}{line}\n")).collect()
}

fn is_identifier(s: &str) -> bool {
    let mut chars = s.chars();
    matches!(chars.next(), Some(c) if c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

fn go_string(text: &str) -> String {
    Value::String(text.to_owned()).to_string()
}

fn to_pascal(name: &str) -> String {
    let mut out = String::new();
    let mut upper = true;
    for c in name.chars() {
        if c.is_ascii_alphanumeric() {
            if upper {
                out.push(c.to_ascii_uppercase());
            } else {
                out.push(c);
            }
            upper = false;
        } else {
            upper = true;
        }
    }
    if out.is_empty() || out.starts_with(|c: char| c.is_ascii_digit()) {
        out.insert_str(0, "Flow");
    }
    out
}

#[cfg(test)]
#[path = "codegen_go_test.rs"]
mod codegen_go_test;
