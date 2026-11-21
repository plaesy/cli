//! Codegen flow-ir/1 ke modul JavaScript (ADR 0003: codegen-js).
//!
//! Cakupan langkah ini sengaja sempit dan tercatat:
//! - Modul `action` dan `pass` diterjemahkan, termasuk `retry` dan `timeout_secs`.
//!   `action` dengan `on_error` selain `fail` belum didukung.
//! - `for_each` diterjemahkan sebagai loop berurutan. `parallel` dan `parallelism` hanya dicatat
//!   sebagai komentar. `skip_failures: true` belum didukung. Modul di dalam `for_each` harus
//!   `action` atau `pass`, dan tidak boleh bersarang.
//! - Ekspresi input: path (`flow.input.x`, `mID.path`), literal string, dan `item.value` di dalam
//!   `for_each` (menunjuk ke elemen iterasi).
//! - Modul lain (`branch_one`, `while`, `approval`, `subflow`, ...) dan ekspresi di luar cakupan
//!   itu tidak dilewati diam-diam. Kode yang dihasilkan melempar galat di titik modul itu, dan
//!   laporan kehilangan mencatatnya.
//!
//! Dokumen divalidasi dengan aturan flow-ir dulu. Tidak ada I/O di sini.

use plaesy_domain_flow::validate as validate_flow;
use serde_json::{json, Value};
use std::collections::BTreeSet;
use std::fmt::Write as _;

/// Modul yang diterjemahkan ke JavaScript.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mapping {
    pub module: String,
    pub kind: String,
}

/// Modul yang tidak diterjemahkan, dengan alasan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Loss {
    pub module: String,
    pub kind: String,
    pub reason: String,
}

/// Laporan codegen (ADR 0008 berlaku juga di sini: tidak ada yang dibuang diam-diam).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Report {
    pub mapped: Vec<Mapping>,
    pub lost: Vec<Loss>,
}

/// Hasil codegen: kode JavaScript dan laporannya.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Generated {
    pub source: String,
    pub report: Report,
}

/// Galat codegen: dokumen tidak valid.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodegenError(pub Vec<String>);

impl std::fmt::Display for CodegenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "flow tidak valid: {}", self.0.join("; "))
    }
}

impl std::error::Error for CodegenError {}

/// Membuat modul JavaScript dari flow-ir/1 `doc`.
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
    let function = to_camel(name);
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
                        "  // {id} ({kind}): belum didukung codegen-js\n  throw new Error({});",
                        js_string(&format!("modul {id} ({kind}) belum didukung: {reason}"))
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
                // Modul yang tidak didukung melempar galat saat dijalankan, bukan dilewati.
                let _ = writeln!(
                    body,
                    "  // {id} ({kind}): belum didukung codegen-js\n  throw new Error({});",
                    js_string(&format!("modul {id} ({kind}) belum didukung: {reason}"))
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
         export async function {function}(input, ctx) {{\n  const scope = {{}};\n{body}  return scope;\n}}\n",
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
            let mut obj = Vec::new();
            for (name, expr) in module
                .get("inputs")
                .and_then(Value::as_object)
                .into_iter()
                .flatten()
            {
                obj.push(format!(
                    "{}: {}",
                    js_key(name),
                    expression(expr.as_str().unwrap_or(""), in_loop)?
                ));
            }
            Ok(format!("  scope.{id} = {{ {} }};\n", obj.join(", ")))
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
                    js_key(name),
                    expression(expr.as_str().unwrap_or(""), in_loop)?
                ));
            }
            let timeout = module.get("timeout_secs").and_then(Value::as_u64);
            let call = format!(
                "ctx.action({}, {{ {} }}{})",
                js_string(operation),
                inputs.join(", "),
                timeout.map_or(String::new(), |t| format!(", {{ timeoutSecs: {t} }}"))
            );
            let call = match module.get("retry") {
                Some(retry) => format!("ctx.withRetry({}, () => {call})", retry_policy(retry)),
                None => call,
            };
            Ok(format!("  scope.{id} = await {call};\n"))
        }
        "for_each" => {
            if in_loop {
                return Err("for_each bersarang belum didukung".to_owned());
            }
            emit_for_each(id, module, all)
        }
        other => Err(format!("kind {other:?} di luar cakupan codegen-js")),
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
    let mut out = String::new();
    let _ = writeln!(out, "  scope.{id} = [];");
    if module
        .get("parallel")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        let _ = writeln!(out, "  // {id}: parallel: true dijalankan berurutan");
    }
    if let Some(p) = module.get("parallelism").and_then(Value::as_u64) {
        let _ = writeln!(
            out,
            "  // {id}: parallelism {p} dicatat, tidak dipakai (berurutan)"
        );
    }
    let _ = writeln!(out, "  for (const item of {source}) {{");
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
        out.push_str(&indent(&code, "  "));
    }
    out.push_str("  }\n");
    Ok(out)
}

/// Mengubah ekspresi path, literal string, atau `item.value` (di dalam loop) menjadi JavaScript.
fn expression(text: &str, in_loop: bool) -> Result<String, String> {
    let text = text.trim();
    if let Some(inner) = text.strip_prefix('\'').and_then(|s| s.strip_suffix('\'')) {
        return Ok(js_string(inner));
    }
    if in_loop && text == "item.value" {
        return Ok("item".to_owned());
    }
    let segments: Vec<&str> = text.split('.').collect();
    if segments.len() < 2 || segments.iter().any(|s| !is_identifier(s)) {
        return Err(format!("ekspresi {text:?} di luar cakupan"));
    }
    if segments[0] == "flow" && segments[1] == "input" {
        return Ok(format!("input{}", path(&segments[2..])));
    }
    if segments[0].starts_with('m') && segments[0][1..].chars().all(|c| c.is_ascii_digit()) {
        return Ok(format!("scope.{}{}", segments[0], path(&segments[1..])));
    }
    Err(format!("ekspresi {text:?} di luar cakupan"))
}

fn path(segments: &[&str]) -> String {
    segments.iter().map(|s| format!(".{s}")).collect()
}

fn retry_policy(retry: &Value) -> String {
    json!({
        "kind": retry.get("kind"),
        "maxAttempts": retry.get("max_attempts"),
        "baseSecs": retry.get("base_secs"),
        "multiplier": retry.get("multiplier"),
        "capSecs": retry.get("cap_secs"),
    })
    .to_string()
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

/// `branch_one` menjadi if/else berurutan. Modul yang tidak didukung di dalam cabang melempar
/// galat di titik itu. Modul lain di cabang yang sama tetap diterjemahkan.
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
        let _ = writeln!(out, "  {keyword} ({condition}) {{");
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
            out.push_str("  {\n");
        } else {
            out.push_str("  } else {\n");
        }
        emit_children(&mut out, &default, all, &mut outcomes);
    }
    if !branches.is_empty() || !default.is_empty() {
        out.push_str("  }\n");
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
                "    throw new Error({});",
                js_string(&format!("anggota {member} tidak ditemukan"))
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
                out.push_str(&indent(&code, "  "));
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
                    "    // {member} ({kind}): belum didukung codegen-js\n    throw new Error({});",
                    js_string(&format!("modul {member} ({kind}) belum didukung: {reason}"))
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
    let js_op = if op == "==" { "===" } else { "!==" };
    Ok(format!("{left} {js_op} {right}"))
}

fn split_comparison(when: &str) -> Option<(&str, &str, &str)> {
    ["==", "!="].into_iter().find_map(|op| {
        when.split_once(&format!(" {op} "))
            .map(|(left, right)| (left.trim(), op, right.trim()))
    })
}

fn literal(text: &str) -> Option<String> {
    if let Some(inner) = text.strip_prefix('\'').and_then(|s| s.strip_suffix('\'')) {
        return Some(js_string(inner));
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

fn js_key(name: &str) -> String {
    if is_identifier(name) {
        name.to_owned()
    } else {
        js_string(name)
    }
}

fn js_string(text: &str) -> String {
    Value::String(text.to_owned()).to_string()
}

fn to_camel(name: &str) -> String {
    let mut out = String::new();
    let mut upper = false;
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
        out.insert_str(0, "flow");
    }
    out
}

#[cfg(test)]
#[path = "codegen_js_test.rs"]
mod codegen_js_test;
