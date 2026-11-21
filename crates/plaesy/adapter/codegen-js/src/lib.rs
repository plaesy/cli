//! Codegen flow-ir/1 ke modul JavaScript (ADR 0003: codegen-js).
//!
//! Cakupan langkah ini sengaja sempit dan tercatat:
//! - Modul `action` dan `pass` diterjemahkan, termasuk `retry` dan `timeout_secs`.
//! - Ekspresi input hanya path (`flow.input.x`, `mID.path`) atau literal string.
//! - Modul lain (`branch_one`, `for_each`, `while`, `approval`, `subflow`, ...) dan ekspresi di
//!   luar cakupan itu tidak dilewati diam-diam. Kode yang dihasilkan melempar galat di titik
//!   modul itu, dan laporan kehilangan mencatatnya.
//!
//! Dokumen divalidasi dengan aturan flow-ir dulu. Tidak ada I/O di sini.

use plaesy_domain_flow::validate as validate_flow;
use serde_json::{json, Value};
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
    let mut report = Report::default();
    let mut body = String::new();

    for module in doc
        .get("modules")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let id = module
            .get("id")
            .and_then(Value::as_str)
            .unwrap_or("?")
            .to_owned();
        let kind = module
            .get("kind")
            .and_then(Value::as_str)
            .unwrap_or("?")
            .to_owned();
        match emit_module(&id, &kind, module) {
            Ok(code) => {
                let _ = write!(body, "{code}");
                report.mapped.push(Mapping { module: id, kind });
            }
            Err(reason) => {
                // Modul yang tidak didukung melempar galat saat dijalankan, bukan dilewati.
                let _ = writeln!(
                    body,
                    "  // {id} ({kind}): belum didukung codegen-js\n  throw new Error({});",
                    js_string(&format!("modul {id} ({kind}) belum didukung: {reason}"))
                );
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
    Ok(Generated { source, report })
}

fn emit_module(id: &str, kind: &str, module: &Value) -> Result<String, String> {
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
                    expression(expr.as_str().unwrap_or(""))?
                ));
            }
            Ok(format!("  scope.{id} = {{ {} }};\n", obj.join(", ")))
        }
        "action" => {
            let operation = module
                .get("operation")
                .and_then(Value::as_str)
                .ok_or_else(|| "operation wajib".to_owned())?;
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
                    expression(expr.as_str().unwrap_or(""))?
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
        other => Err(format!("kind {other:?} di luar cakupan codegen-js")),
    }
}

/// Mengubah ekspresi path atau literal string menjadi JavaScript.
fn expression(text: &str) -> Result<String, String> {
    let text = text.trim();
    if let Some(inner) = text.strip_prefix('\'').and_then(|s| s.strip_suffix('\'')) {
        return Ok(js_string(inner));
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
