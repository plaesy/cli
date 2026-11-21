//! Codegen flow-ir/1 ke paket Go (ADR 0003: codegen-go). Cakupan dan perilaku sama dengan
//! `plaesy-adapter-codegen-js`:
//! - Modul `action` (dengan retry dan timeout) dan `pass` diterjemahkan.
//! - Ekspresi input hanya path (`flow.input.x`, `mID.path`) atau literal string.
//! - Modul lain dan ekspresi di luar cakupan dicatat sebagai kehilangan, dan kode yang
//!   dihasilkan mengembalikan galat di titik itu. Tidak ada yang dilewati diam-diam.
//!
//! Kode yang dihasilkan memakai tipe `Context`, `ActionOptions`, dan `RetryPolicy` di paket
//! yang sama. Tipe itu dibuat oleh runtime, bukan oleh generator ini.

use plaesy_domain_flow::validate as validate_flow;
use serde_json::Value;
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
                let _ = writeln!(
                    body,
                    "\t// {id} ({kind}): belum didukung codegen-go\n\treturn nil, fmt.Errorf({})",
                    go_string(&format!("modul {id} ({kind}) belum didukung: {reason}"))
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
         \treturn cur\n}}\n",
        doc.get("id").and_then(Value::as_str).unwrap_or("?"),
    );
    Ok(Generated { source, report })
}

fn emit_module(id: &str, kind: &str, module: &Value) -> Result<String, String> {
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
                    expression(expr.as_str().unwrap_or(""))?
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
                    expression(expr.as_str().unwrap_or(""))?
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
        other => Err(format!("kind {other:?} di luar cakupan codegen-go")),
    }
}

/// Ekspresi path atau literal string menjadi Go.
fn expression(text: &str) -> Result<String, String> {
    let text = text.trim();
    if let Some(inner) = text.strip_prefix('\'').and_then(|s| s.strip_suffix('\'')) {
        return Ok(go_string(inner));
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
