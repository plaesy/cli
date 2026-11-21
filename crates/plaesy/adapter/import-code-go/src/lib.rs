//! Impor kode Go menjadi `api-ir/1` (ADR 0008, bahasa ketiga setelah Rust dan Python).
//!
//! Pilihan parser: `gosyn` 0.2.16. Alasannya: murni Rust dengan lisensi MIT dan tanpa
//! kompilasi C, sehingga tidak menambah toolchain. `tree-sitter-go` dipertimbangkan, tetapi
//! membutuhkan kompilasi C dan API yang lebih berat untuk tujuan impor tipe.
//!
//! Yang dipetakan:
//! - `type X struct { Field T }` dengan field bernama. Tipe: string, int*/uint*, float32/64,
//!   bool, `[]T`, `*T` (pointer dianggap opsional), dan tipe lain di berkas yang sama.
//! - `type X string` dengan blok const bernilai X menjadi enum.
//!
//! Yang tidak dipetakan dicatat di [`Report::lost`] dengan baris dan teks sumbernya: fungsi,
//! metode, impor, variabel, tipe generik, embedded struct, dan tipe asing.

use gosyn::ast::{ConstSpec, Declaration, Expression, Field, File, StructType, TypeSpec};
use serde_json::{json, Map, Value};
use std::collections::{BTreeMap, BTreeSet};

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

impl std::fmt::Display for ImportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "kode Go tidak bisa diparse: {}", self.0)
    }
}

impl std::error::Error for ImportError {}

/// Mengimpor berkas Go `source` menjadi api-ir/1 bernama `name`.
///
/// # Errors
///
/// Kode yang tidak bisa diparse oleh `gosyn`.
pub fn import(source: &str, name: &str) -> Result<Imported, ImportError> {
    let file: File = gosyn::parse_source(source).map_err(|e| ImportError(e.to_string()))?;

    let mut defined: BTreeSet<String> = BTreeSet::new();
    for decl in &file.decl {
        if let Declaration::Type(group) = decl {
            for spec in &group.specs {
                defined.insert(spec.name.name.clone());
            }
        }
    }
    let enum_values = collect_enum_values(&file);

    let mut types = Map::new();
    let mut report = Report::default();

    for import in &file.imports {
        lose(
            &mut report,
            source,
            import.path.pos,
            "impor tidak dipetakan",
        );
    }

    for decl in &file.decl {
        match decl {
            Declaration::Type(group) => {
                for spec in &group.specs {
                    map_type(
                        spec,
                        source,
                        &defined,
                        &enum_values,
                        &mut types,
                        &mut report,
                    );
                }
            }
            Declaration::Const(_) => {
                // Const dipetakan lewat enum_values; yang bukan enum dicatat di bawah.
            }
            Declaration::Function(f) => lose(
                &mut report,
                source,
                f.name.pos,
                &format!("fungsi {} tidak dipetakan", f.name.name),
            ),
            Declaration::Variable(_) => lose(&mut report, source, 0, "variabel tidak dipetakan"),
        }
    }

    // Const yang bukan milik enum yang dipetakan dicatat sebagai kehilangan.
    for decl in &file.decl {
        if let Declaration::Const(group) = decl {
            for spec in &group.specs {
                for ident in &spec.name {
                    if !types.contains_key(&enum_owner_name(spec, &file)) {
                        lose(
                            &mut report,
                            source,
                            ident.pos,
                            &format!(
                                "const {} tidak terhubung ke enum yang dipetakan",
                                ident.name
                            ),
                        );
                    }
                }
            }
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

fn map_type(
    spec: &TypeSpec,
    source: &str,
    defined: &BTreeSet<String>,
    enum_values: &BTreeMap<String, Option<Vec<String>>>,
    types: &mut Map<String, Value>,
    report: &mut Report,
) {
    let name = spec.name.name.clone();
    let line_pos = spec.name.pos;
    if !spec.params.list.is_empty() {
        lose(
            report,
            source,
            line_pos,
            &format!("tipe generik {name} tidak dipetakan"),
        );
        return;
    }
    match &spec.typ {
        Expression::TypeStruct(st) => match object_schema(st, defined, source, report) {
            Some(object) => {
                types.insert(name.clone(), json!({ "object": object }));
                report.mapped.push(Mapping {
                    line: line_of(source, line_pos),
                    name,
                });
            }
            None => lose(
                report,
                source,
                line_pos,
                &format!("struct {name} tidak bisa dipetakan"),
            ),
        },
        Expression::Ident(ident) if ident.name == "string" => match enum_values.get(&name) {
            Some(Some(values)) if !values.is_empty() => {
                types.insert(name.clone(), json!({ "enum": values }));
                report.mapped.push(Mapping {
                    line: line_of(source, line_pos),
                    name,
                });
            }
            _ => lose(
                report,
                source,
                line_pos,
                &format!("tipe {name} string tanpa const enum tidak dipetakan"),
            ),
        },
        _ => lose(
            report,
            source,
            line_pos,
            &format!("tipe {name} tidak didukung"),
        ),
    }
}

fn object_schema(
    st: &StructType,
    defined: &BTreeSet<String>,
    source: &str,
    report: &mut Report,
) -> Option<Value> {
    let mut properties = Map::new();
    let mut required = Vec::new();
    for field in &st.fields {
        if field.name.is_empty() {
            lose(
                report,
                source,
                field_pos(field),
                "embedded struct tidak didukung",
            );
            continue;
        }
        for ident in &field.name {
            match field_schema(&field.typ, defined) {
                Some((schema, optional)) => {
                    if !optional {
                        required.push(json!(ident.name));
                    }
                    properties.insert(ident.name.clone(), schema);
                }
                None => lose(
                    report,
                    source,
                    ident.pos,
                    &format!("field {}: tipe tidak dipetakan", ident.name),
                ),
            }
        }
    }
    let mut object = Map::new();
    object.insert("properties".to_owned(), Value::Object(properties));
    if !required.is_empty() {
        object.insert("required".to_owned(), Value::Array(required));
    }
    Some(Value::Object(object))
}

/// Skema untuk satu tipe field, dan apakah field itu opsional (pointer).
fn field_schema(typ: &Expression, defined: &BTreeSet<String>) -> Option<(Value, bool)> {
    match typ {
        Expression::TypePointer(ptr) => field_schema(&ptr.typ, defined).map(|(s, _)| (s, true)),
        Expression::TypeSlice(slice) => {
            field_schema(&slice.typ, defined).map(|(s, _)| (json!({ "array": s }), false))
        }
        Expression::Ident(ident) => {
            let schema = match ident.name.as_str() {
                "string" => json!({ "type": "string" }),
                "bool" => json!({ "type": "boolean" }),
                "int" | "int8" | "int16" | "int32" | "int64" | "uint" | "uint8" | "uint16"
                | "uint32" | "uint64" => json!({ "type": "integer" }),
                "float32" | "float64" => json!({ "type": "number" }),
                other if defined.contains(other) => json!({ "$ref": other }),
                _ => return None,
            };
            Some((schema, false))
        }
        _ => None,
    }
}

/// Nilai const per tipe enum. Blok const memakai tipe terakhir yang ditulis (iota).
fn collect_enum_values(file: &File) -> BTreeMap<String, Option<Vec<String>>> {
    // Nilai enum adalah isi string konstanta (`"draft"`), bukan nama konstanta (`StatusDraft`).
    // Bila ada anggota yang bukan string literal, enum itu dicatat `None` dan tidak dipetakan.
    let mut out: BTreeMap<String, Option<Vec<String>>> = BTreeMap::new();
    for decl in &file.decl {
        let Declaration::Const(group) = decl else {
            continue;
        };
        let mut current: Option<String> = None;
        for spec in &group.specs {
            if let Some(Expression::Ident(ident)) = &spec.typ {
                current = Some(ident.name.clone());
            }
            let Some(owner) = &current else {
                continue;
            };
            for index in 0..spec.name.len() {
                let wire = match spec.values.get(index) {
                    Some(Expression::BasicLit(lit)) if lit.value.starts_with('"') => {
                        serde_json::from_str::<String>(&lit.value).ok()
                    }
                    _ => None,
                };
                let entry = out.entry(owner.clone()).or_insert_with(|| Some(Vec::new()));
                match (entry.as_mut(), wire) {
                    (Some(values), Some(value)) => values.push(value),
                    _ => *entry = None,
                }
            }
        }
    }
    out
}

/// Nama tipe pemilik sebuah const, untuk pelaporan kehilangan.
fn enum_owner_name(spec: &ConstSpec, _file: &File) -> String {
    match &spec.typ {
        Some(Expression::Ident(ident)) => ident.name.clone(),
        _ => String::new(),
    }
}

fn field_pos(field: &Field) -> usize {
    field.name.first().map_or(0, |i| i.pos)
}

fn lose(report: &mut Report, source: &str, pos: usize, reason: &str) {
    let line = line_of(source, pos);
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
#[path = "import_code_go_test.rs"]
mod import_code_go_test;
