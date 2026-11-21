//! Codegen flow-ir/1 ke modul Rust (ADR 0002 dan 0003: Rust adalah target pertama untuk flow-ir).
//!
//! Cakupan sama dengan `plaesy-adapter-codegen-go`:
//! - `action` dengan retry eksponensial, timeout, dan `on_error: "fail"`.
//! - `pass`.
//! - `branch_one` dengan kondisi `==` atau `!=` antara ekspresi dan literal.
//! - `for_each` berurutan, dengan variabel `item` untuk `item.value`.
//!
//! Modul di luar cakupan dicatat sebagai kehilangan. Kode yang dihasilkan mengembalikan
//! `FlowError` di titik modul itu, sehingga tidak ada yang dilewati diam-diam.
//!
//! Kode yang dihasilkan memakai `serde_json` dan mendefinisikan `ActionRunner`, `FlowError`,
//! dan `RetryPolicy` sendiri. Pelaksanaan nyata `ActionRunner` berada di luar kode itu.

use plaesy_domain_flow::validate as validate_flow;
use serde_json::Value;
use std::collections::BTreeSet;

/// Modul yang berhasil diterjemahkan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mapping {
    pub module: String,
    pub kind: String,
}

/// Modul yang tidak diterjemahkan, dengan alasannya.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Loss {
    pub module: String,
    pub kind: String,
    pub reason: String,
}

/// Laporan codegen: setiap modul tercatat tepat sekali sebagai mapped atau lost.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Report {
    pub mapped: Vec<Mapping>,
    pub lost: Vec<Loss>,
}

/// Hasil codegen: kode Rust dan laporannya.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Generated {
    pub source: String,
    pub report: Report,
}

/// Galat: dokumen tidak valid menurut aturan flow-ir.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodegenError(pub Vec<String>);

impl std::fmt::Display for CodegenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "flow tidak valid: {}", self.0.join("; "))
    }
}

impl std::error::Error for CodegenError {}

/// Bagian tetap di awal berkas yang dihasilkan: tipe bantuan dan trait.
const PRELUDE: &str = r#"#![allow(dead_code, unused_imports, unreachable_code)]
use serde_json::{json, Value};
use std::collections::HashMap;
use std::thread::sleep;
use std::time::Duration;

/// Galat satu modul: nama modul dan pesan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FlowError {
    pub module: String,
    pub message: String,
}

/// Pemanggil operasi. Implementasi nyata berada di luar kode yang dihasilkan.
pub trait ActionRunner {
    fn action(&self, operation: &str, inputs: &Value, timeout_secs: u64) -> Result<Value, String>;
}

/// Kebijakan retry eksponensial dari flow-ir.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RetryPolicy {
    pub max_attempts: u32,
    pub base_secs: u64,
    pub multiplier: u64,
    pub cap_secs: u64,
}

fn with_retry<F>(policy: Option<&RetryPolicy>, mut call: F) -> Result<Value, String>
where
    F: FnMut() -> Result<Value, String>,
{
    let Some(policy) = policy else {
        return call();
    };
    let mut delay = policy.base_secs;
    let mut attempt = 1u32;
    loop {
        match call() {
            Ok(value) => return Ok(value),
            Err(error) if attempt >= policy.max_attempts => return Err(error),
            Err(_) => {
                sleep(Duration::from_secs(delay.min(policy.cap_secs)));
                delay = delay.saturating_mul(policy.multiplier);
                attempt += 1;
            }
        }
    }
}

/// Membaca field bertingkat. Field yang tidak ada menghasilkan `null`.
fn dig(value: &Value, keys: &[&str]) -> Value {
    let mut current = value;
    for key in keys {
        match current.get(*key) {
            Some(next) => current = next,
            None => return Value::Null,
        }
    }
    current.clone()
}
"#;

/// Membuat modul Rust dari flow-ir/1 `doc`. Fungsi utamanya bernama dari `name`.
///
/// # Errors
///
/// [`CodegenError`] bila dokumen melanggar aturan semantik flow-ir.
pub fn generate(doc: &Value) -> Result<Generated, CodegenError> {
    let errors: Vec<String> = validate_flow(doc).iter().map(ToString::to_string).collect();
    if !errors.is_empty() {
        return Err(CodegenError(errors));
    }
    let all: Vec<Value> = doc
        .get("modules")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let children: BTreeSet<String> = all.iter().flat_map(child_ids).collect();
    let name = doc.get("name").and_then(Value::as_str).unwrap_or("flow");
    let doc_id = doc.get("id").and_then(Value::as_str).unwrap_or("?");

    let mut ctx = Ctx {
        all: &all,
        report: Report::default(),
    };
    let mut body = String::new();
    for module in &all {
        if !children.contains(&str_field(module, "id")) {
            body.push_str(&emit(&mut ctx, module, 1, false));
        }
    }

    let source = format!(
        "// Dibuat dari flow-ir/1 {name:?} ({doc_id}). Jangan diedit manual.\n{prelude}\n\
         #[allow(unused_mut, unused_variables)]\n\
         pub fn {function}(input: &Value, runner: &dyn ActionRunner) -> Result<HashMap<String, Value>, FlowError> {{\n\
         \x20   let mut scope: HashMap<String, Value> = HashMap::new();\n\
         {body}\
         \x20   Ok(scope)\n\
         }}\n",
        prelude = PRELUDE,
        function = to_snake(name),
    );
    Ok(Generated {
        source,
        report: ctx.report,
    })
}

struct Ctx<'a> {
    all: &'a [Value],
    report: Report,
}

/// Id anggota dari modul kontainer (`branch_one` dan `for_each`).
fn child_ids(module: &Value) -> Vec<String> {
    match str_field(module, "kind").as_str() {
        "branch_one" => module
            .get("branches")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .flat_map(|branch| string_list(branch, "modules"))
            .chain(string_list(module, "default"))
            .collect(),
        "for_each" => string_list(module, "modules"),
        _ => Vec::new(),
    }
}

/// Menghasilkan kode satu modul dan mencatatnya di laporan.
fn emit(ctx: &mut Ctx<'_>, module: &Value, depth: usize, in_loop: bool) -> String {
    let id = str_field(module, "id");
    let kind = str_field(module, "kind");
    let pad = indent(depth);
    let result = match kind.as_str() {
        "action" => action(module, &pad, in_loop),
        "pass" => pass(module, &pad, in_loop),
        "branch_one" => branch(ctx, module, depth, in_loop),
        "for_each" => for_each(ctx, module, depth, in_loop),
        other => Err(format!("kind {other:?} di luar cakupan codegen-rust")),
    };
    match result {
        Ok(code) => {
            ctx.report.mapped.push(Mapping { module: id, kind });
            code
        }
        Err(reason) => {
            let stub_code = stub(&id, &kind, &reason, &pad);
            ctx.report.lost.push(Loss {
                module: id,
                kind,
                reason,
            });
            stub_code
        }
    }
}

/// Menghasilkan anggota yang dirujuk container. Anggota yang hilang dicatat sebagai kehilangan.
fn emit_child(ctx: &mut Ctx<'_>, id: &str, depth: usize, in_loop: bool) -> String {
    let all = ctx.all;
    match find(all, id) {
        Some(module) => emit(ctx, module, depth, in_loop),
        None => {
            let pad = indent(depth);
            let reason = "anggota tidak ditemukan";
            ctx.report.lost.push(Loss {
                module: id.to_owned(),
                kind: "?".to_owned(),
                reason: reason.to_owned(),
            });
            stub(id, "?", reason, &pad)
        }
    }
}

/// Pernyataan yang mengembalikan `FlowError` di titik modul yang tidak didukung.
fn stub(id: &str, kind: &str, reason: &str, pad: &str) -> String {
    format!(
        "{pad}return Err(FlowError {{ module: {}.to_owned(), message: {}.to_owned() }});\n",
        rs(id),
        rs(&format!("modul {id} ({kind}) belum didukung: {reason}"))
    )
}

fn action(module: &Value, pad: &str, in_loop: bool) -> Result<String, String> {
    let id = str_field(module, "id");
    let operation = module
        .get("operation")
        .and_then(Value::as_str)
        .ok_or_else(|| "operation wajib".to_owned())?;
    let on_error = module
        .get("on_error")
        .and_then(Value::as_str)
        .unwrap_or("fail");
    if on_error != "fail" {
        return Err(format!("on_error {on_error:?} belum didukung"));
    }
    if let Some(retry) = module.get("retry") {
        let kind = retry
            .get("kind")
            .and_then(Value::as_str)
            .unwrap_or("exponential");
        if kind != "exponential" {
            return Err(format!("retry {kind:?} belum didukung"));
        }
    }
    let fields = input_fields(module, in_loop)?;
    let timeout = u64_field(module, "timeout_secs", 0);
    let call = format!(
        "runner.action({}, &json!({{ {fields} }}), {timeout})",
        rs(operation)
    );
    Ok(format!(
        "{pad}let value = with_retry({policy}, || {call}).map_err(|message| FlowError {{ module: {id_lit}.to_owned(), message }})?;\n\
         {pad}scope.insert({id_lit}.to_owned(), value);\n",
        policy = retry_policy(module),
        id_lit = rs(&id),
    ))
}

fn pass(module: &Value, pad: &str, in_loop: bool) -> Result<String, String> {
    let id = str_field(module, "id");
    let fields = input_fields(module, in_loop)?;
    Ok(format!(
        "{pad}scope.insert({}.to_owned(), json!({{ {fields} }}));\n",
        rs(&id)
    ))
}

/// Pasangan `"kunci": ekspresi` dari `inputs` modul, dipisah koma.
fn input_fields(module: &Value, in_loop: bool) -> Result<String, String> {
    let mut fields = Vec::new();
    for (key, expr) in module
        .get("inputs")
        .and_then(Value::as_object)
        .into_iter()
        .flatten()
    {
        fields.push(format!(
            "{}: {}",
            rs(key),
            expression(expr.as_str().unwrap_or(""), in_loop)?
        ));
    }
    Ok(fields.join(", "))
}

/// `branch_one` menjadi `if`, `else if`, dan `else`. Anggota cabang dihasilkan di dalam blok.
fn branch(
    ctx: &mut Ctx<'_>,
    module: &Value,
    depth: usize,
    in_loop: bool,
) -> Result<String, String> {
    let pad = indent(depth);
    let mut arms: Vec<(String, Vec<String>)> = Vec::new();
    for branch in module
        .get("branches")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let cond = condition(&str_field(branch, "when"), in_loop)?;
        arms.push((cond, string_list(branch, "modules")));
    }
    let default = string_list(module, "default");

    let mut out = String::new();
    for (index, (cond, members)) in arms.iter().enumerate() {
        let keyword = if index == 0 { "if" } else { "} else if" };
        out.push_str(&format!("{pad}{keyword} {cond} {{\n"));
        for member in members {
            out.push_str(&emit_child(ctx, member, depth + 1, in_loop));
        }
    }
    if !default.is_empty() {
        if arms.is_empty() {
            out.push_str(&format!("{pad}{{\n"));
        } else {
            out.push_str(&format!("{pad}}} else {{\n"));
        }
        for member in &default {
            out.push_str(&emit_child(ctx, member, depth + 1, in_loop));
        }
        out.push_str(&format!("{pad}}}\n"));
    } else if !arms.is_empty() {
        out.push_str(&format!("{pad}}}\n"));
    }
    Ok(out)
}

/// `for_each` menjadi loop berurutan. `parallel` dan `parallelism` dicatat sebagai komentar.
fn for_each(
    ctx: &mut Ctx<'_>,
    module: &Value,
    depth: usize,
    in_loop: bool,
) -> Result<String, String> {
    let pad = indent(depth);
    if module
        .get("skip_failures")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        return Err("skip_failures belum didukung".to_owned());
    }
    let iterator = str_field(module, "iterator");
    if iterator.is_empty() {
        return Err("iterator wajib".to_owned());
    }
    let source = expression(&iterator, in_loop)?;
    let members = string_list(module, "modules");
    if members.is_empty() {
        return Err("for_each tanpa anggota".to_owned());
    }
    let parallel = module
        .get("parallel")
        .and_then(Value::as_bool)
        .unwrap_or(false);

    let mut out = format!("{pad}// parallel={parallel}: dijalankan berurutan\n");
    out.push_str(&format!(
        "{pad}for item in {source}.as_array().cloned().unwrap_or_default() {{\n"
    ));
    for member in &members {
        out.push_str(&emit_child(ctx, member, depth + 1, true));
    }
    out.push_str(&format!("{pad}}}\n"));
    Ok(out)
}

/// Kondisi `when` dalam bentuk `ekspresi == literal` atau `ekspresi != literal`.
fn condition(when: &str, in_loop: bool) -> Result<String, String> {
    let (left, op, right) =
        split_comparison(when).ok_or_else(|| format!("kondisi {when:?} di luar cakupan"))?;
    Ok(format!(
        "{} {op} {}",
        expression(left, in_loop)?,
        expression(right, in_loop)?
    ))
}

fn split_comparison(when: &str) -> Option<(&str, &str, &str)> {
    for op in ["==", "!="] {
        if let Some((left, right)) = when.split_once(op) {
            return Some((left.trim(), op, right.trim()));
        }
    }
    None
}

/// Ekspresi menjadi kode Rust yang menghasilkan `Value`. Hanya literal, path `flow.input.x`,
/// path `mID.x`, dan `item.x` di dalam loop yang didukung.
fn expression(text: &str, in_loop: bool) -> Result<String, String> {
    let text = text.trim();
    if let Some(inner) = text.strip_prefix('\'').and_then(|s| s.strip_suffix('\'')) {
        return Ok(format!("json!({})", rs(inner)));
    }
    if text == "true" || text == "false" {
        return Ok(format!("json!({text})"));
    }
    if let Ok(number) = text.parse::<i64>() {
        return Ok(format!("json!({number})"));
    }
    let segments: Vec<&str> = text.split('.').collect();
    if segments.len() < 2 || segments.iter().any(|s| !is_identifier(s)) {
        return Err(format!("ekspresi {text:?} di luar cakupan"));
    }
    match segments[0] {
        "flow" if segments[1] == "input" => {
            Ok(format!("dig(input, &[{}])", keys_list(&segments[2..])))
        }
        "item" if in_loop => Ok(format!("dig(&item, &[{}])", keys_list(&segments[1..]))),
        root if is_module_ref(root) => Ok(format!(
            "dig(scope.get({}).unwrap_or(&Value::Null), &[{}])",
            rs(root),
            keys_list(&segments[1..])
        )),
        _ => Err(format!("ekspresi {text:?} di luar cakupan")),
    }
}

/// `RetryPolicy` untuk modul, atau `None` bila tanpa retry.
fn retry_policy(module: &Value) -> String {
    match module.get("retry") {
        Some(retry) => format!(
            "Some(&RetryPolicy {{ max_attempts: {}, base_secs: {}, multiplier: {}, cap_secs: {} }})",
            u64_field(retry, "max_attempts", 1),
            u64_field(retry, "base_secs", 0),
            u64_field(retry, "multiplier", 1),
            u64_field(retry, "cap_secs", 0),
        ),
        None => "None".to_owned(),
    }
}

fn keys_list(keys: &[&str]) -> String {
    keys.iter().map(|k| rs(k)).collect::<Vec<_>>().join(", ")
}

fn is_module_ref(s: &str) -> bool {
    s.len() > 1 && s.starts_with('m') && s[1..].chars().all(|c| c.is_ascii_digit())
}

fn is_identifier(s: &str) -> bool {
    let mut chars = s.chars();
    matches!(chars.next(), Some(c) if c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

fn find<'a>(all: &'a [Value], id: &str) -> Option<&'a Value> {
    all.iter().find(|m| str_field(m, "id") == id)
}

fn str_field(value: &Value, key: &str) -> String {
    value
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_owned()
}

fn string_list(value: &Value, key: &str) -> Vec<String> {
    value
        .get(key)
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(str::to_owned)
        .collect()
}

fn u64_field(value: &Value, key: &str, default: u64) -> u64 {
    value.get(key).and_then(Value::as_u64).unwrap_or(default)
}

fn indent(depth: usize) -> String {
    "    ".repeat(depth)
}

/// Literal string Rust yang aman untuk teks apa pun.
fn rs(text: &str) -> String {
    Value::String(text.to_owned()).to_string()
}

/// Nama fungsi Rust dari nama flow: huruf kecil dengan garis bawah.
fn to_snake(name: &str) -> String {
    let mut out = String::new();
    let mut gap = false;
    for c in name.chars() {
        if c.is_ascii_alphanumeric() {
            if gap && !out.is_empty() {
                out.push('_');
            }
            out.push(c.to_ascii_lowercase());
            gap = false;
        } else {
            gap = true;
        }
    }
    if out.is_empty() || out.starts_with(|c: char| c.is_ascii_digit()) {
        out.insert_str(0, "flow_");
    }
    out
}

#[cfg(test)]
#[path = "codegen_rust_test.rs"]
mod codegen_rust_test;
