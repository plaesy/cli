//! Impor kode Python menjadi `api-ir/1` (ADR 0008, bahasa kedua setelah Rust).
//!
//! Yang dipetakan:
//! - Kelas dengan field beranotasi di level atas (`name: type`). Field dengan nilai default
//!   tidak wajib. Kelas tanpa basis, atau dengan basis yang ditulis lokal, dipetakan.
//! - Kelas yang mewarisi `Enum` dan semua nilainya berupa penugasan sederhana menjadi enum.
//! - Anotasi: `str`, `int`, `float`, `bool`, `Optional[T]`, `List[T]`/`list[T]`, dan nama kelas
//!   lain di berkas yang sama.
//!
//! Yang lain (fungsi, impor, metode, anotasi asing, pewarisan selain `Enum`) dicatat di
//! [`Report::lost`] dengan nomor baris dan teks sumbernya. Tidak ada yang ditebak.

use rustpython_parser::ast::{Expr, Stmt};
use rustpython_parser::{parse, Mode};
use serde_json::{json, Map, Value};
use std::collections::BTreeSet;
use std::fmt;

/// Satu konstruksi yang berhasil dipetakan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mapping {
    pub line: usize,
    pub name: String,
}

/// Satu konstruksi yang tidak dipetakan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Loss {
    pub line: usize,
    pub source: String,
    pub reason: String,
}

/// Laporan impor.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Report {
    pub mapped: Vec<Mapping>,
    pub lost: Vec<Loss>,
}

/// Hasil impor.
#[derive(Debug, Clone, PartialEq)]
pub struct Imported {
    pub ir: Value,
    pub report: Report,
}

/// Galat: kode yang tidak bisa diparse.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportError(pub String);

impl fmt::Display for ImportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "kode Python tidak bisa diparse: {}", self.0)
    }
}

impl std::error::Error for ImportError {}

/// Mengimpor berkas Python `source` menjadi api-ir/1 bernama `name`.
///
/// # Errors
///
/// Kode yang tidak bisa diparse oleh parser Python.
pub fn import(source: &str, name: &str) -> Result<Imported, ImportError> {
    let module = parse(source, Mode::Module, "<impor>").map_err(|e| ImportError(e.to_string()))?;
    let body = match module {
        rustpython_parser::ast::Mod::Module(m) => m.body,
        _ => return Err(ImportError("bukan modul".to_owned())),
    };
    let defined: BTreeSet<String> = body
        .iter()
        .filter_map(|stmt| match stmt {
            Stmt::ClassDef(c) => Some(c.name.to_string()),
            _ => None,
        })
        .collect();

    let mut types = Map::new();
    let mut report = Report::default();
    for stmt in &body {
        match stmt {
            Stmt::ClassDef(class) => {
                let class_name = class.name.to_string();
                let line = line_of(source, class.range.start().to_usize());
                let bases: Vec<String> = class
                    .bases
                    .iter()
                    .filter_map(|b| match b {
                        Expr::Name(n) => Some(n.id.to_string()),
                        _ => None,
                    })
                    .collect();
                if bases.iter().any(|b| b == "Enum") {
                    match enum_variants(&class.body) {
                        Some(variants) => {
                            types.insert(class_name.clone(), json!({ "enum": variants }));
                            report.mapped.push(Mapping {
                                line,
                                name: class_name,
                            });
                        }
                        None => lose(
                            &mut report,
                            source,
                            line,
                            "enum dengan nilai bukan penugasan sederhana",
                        ),
                    }
                    continue;
                }
                if !bases.is_empty() {
                    lose(
                        &mut report,
                        source,
                        line,
                        "pewarisan selain Enum tidak didukung",
                    );
                    continue;
                }
                let mut properties = Map::new();
                let mut required = Vec::new();
                for item in &class.body {
                    match item {
                        Stmt::AnnAssign(field) => {
                            let Expr::Name(target) = field.target.as_ref() else {
                                lose(
                                    &mut report,
                                    source,
                                    line_of(source, field.range.start().to_usize()),
                                    "target anotasi bukan nama",
                                );
                                continue;
                            };
                            let field_name = target.id.to_string();
                            match annotation_schema(&field.annotation, &defined) {
                                Some((schema, optional)) => {
                                    let has_default = field.value.is_some();
                                    if !optional && !has_default {
                                        required.push(json!(field_name));
                                    }
                                    properties.insert(field_name, schema);
                                }
                                None => lose(
                                    &mut report,
                                    source,
                                    line_of(source, field.range.start().to_usize()),
                                    &format!("field {field_name}: anotasi tidak dipetakan"),
                                ),
                            }
                        }
                        Stmt::FunctionDef(f) => lose(
                            &mut report,
                            source,
                            line_of(source, f.range.start().to_usize()),
                            &format!("metode {} tidak dipetakan", f.name),
                        ),
                        other => lose(
                            &mut report,
                            source,
                            line_of(source, stmt_start(other)),
                            "pernyataan di dalam kelas tidak dipetakan",
                        ),
                    }
                }
                let mut object = Map::new();
                object.insert("properties".to_owned(), Value::Object(properties));
                if !required.is_empty() {
                    object.insert("required".to_owned(), Value::Array(required));
                }
                types.insert(class_name.clone(), json!({ "object": object }));
                report.mapped.push(Mapping {
                    line,
                    name: class_name,
                });
            }
            other => lose(
                &mut report,
                source,
                line_of(source, stmt_start(other)),
                item_kind(other),
            ),
        }
    }

    let ir = json!({
        "ir": "api-ir/1",
        "id": format!("api_{}", uuid::Uuid::new_v4().simple()),
        "version": 1,
        "name": name,
        "base_path": "/",
        "types": types,
        "endpoints": []
    });
    Ok(Imported { ir, report })
}

/// Nilai enum bila semua pernyataan kelas berupa penugasan `NAMA = ...`.
fn enum_variants(body: &[Stmt]) -> Option<Vec<String>> {
    let mut variants = Vec::new();
    for stmt in body {
        match stmt {
            Stmt::Assign(assign) => {
                let Some(Expr::Name(name)) = assign.targets.first() else {
                    return None;
                };
                variants.push(name.id.to_string());
            }
            // String dokumentasi di awal kelas dilewati.
            Stmt::Expr(_) => {}
            _ => return None,
        }
    }
    Some(variants)
}

/// Skema untuk satu anotasi, dan apakah anotasi itu opsional.
fn annotation_schema(annotation: &Expr, defined: &BTreeSet<String>) -> Option<(Value, bool)> {
    if let Expr::Subscript(sub) = annotation {
        let wrapper = match sub.value.as_ref() {
            Expr::Name(n) => n.id.to_string(),
            _ => return None,
        };
        return match wrapper.as_str() {
            "Optional" => annotation_schema(&sub.slice, defined).map(|(s, _)| (s, true)),
            "List" | "list" => {
                annotation_schema(&sub.slice, defined).map(|(s, _)| (json!({ "array": s }), false))
            }
            _ => None,
        };
    }
    let Expr::Name(name) = annotation else {
        return None;
    };
    let schema = match name.id.as_str() {
        "str" => json!({ "type": "string" }),
        "int" => json!({ "type": "integer" }),
        "float" => json!({ "type": "number" }),
        "bool" => json!({ "type": "boolean" }),
        other if defined.contains(other) => json!({ "$ref": other }),
        _ => return None,
    };
    Some((schema, false))
}

fn stmt_start(stmt: &Stmt) -> usize {
    match stmt {
        Stmt::FunctionDef(s) => s.range.start().to_usize(),
        Stmt::Import(s) => s.range.start().to_usize(),
        Stmt::ImportFrom(s) => s.range.start().to_usize(),
        Stmt::Assign(s) => s.range.start().to_usize(),
        Stmt::AnnAssign(s) => s.range.start().to_usize(),
        Stmt::ClassDef(s) => s.range.start().to_usize(),
        Stmt::Expr(s) => s.range.start().to_usize(),
        _ => 0,
    }
}

fn item_kind(stmt: &Stmt) -> &'static str {
    match stmt {
        Stmt::FunctionDef(_) => "fungsi tidak dipetakan",
        Stmt::Import(_) | Stmt::ImportFrom(_) => "impor tidak dipetakan",
        Stmt::Assign(_) | Stmt::AnnAssign(_) => "penugasan level atas tidak dipetakan",
        _ => "pernyataan tidak dipetakan",
    }
}

fn lose(report: &mut Report, source: &str, line: usize, reason: &str) {
    report.lost.push(Loss {
        line,
        source: source
            .lines()
            .nth(line.saturating_sub(1))
            .unwrap_or("")
            .trim()
            .to_owned(),
        reason: reason.to_owned(),
    });
}

/// Nomor baris (mulai 1) dari posisi byte.
fn line_of(source: &str, offset: usize) -> usize {
    source.as_bytes()[..offset.min(source.len())]
        .iter()
        .filter(|&&b| b == b'\n')
        .count()
        + 1
}

#[cfg(test)]
#[path = "import_code_python_test.rs"]
mod import_code_python_test;
