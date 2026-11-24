//! Impor kode TypeScript (dan JavaScript tanpa anotasi) menjadi `api-ir/1` (ADR 0008).
//!
//! Parser: `oxc_parser` 0.159. Dipilih karena aktif dikembangkan, mendukung TypeScript penuh
//! lewat `SourceType::ts()`, dan AST-nya stabil di satu versi. Sumber dibaca dari
//! `docs.rs/oxc_parser` dan sumber crate-nya, bukan dari ingatan.
//!
//! Yang dipetakan:
//! - `class` dengan field bernama dan anotasi tipe. `name?: T` tidak wajib.
//! - `enum` TypeScript dengan anggota tanpa nilai awal.
//! - Tipe: `string`, `number`, `boolean`, `T[]`, `T | null` / `T | undefined` (opsional), dan
//!   referensi ke kelas lain di berkas yang sama.
//!
//! Yang tidak dipetakan (fungsi, impor, pewarisan, metode, `export`, anotasi asing, enum
//! dengan nilai) dicatat di [`Report::lost`] dengan nomor baris dan teks sumbernya. Ekspor
//! (`export class ...`) saat ini juga dicatat sebagai kehilangan karena struktur deklarasinya
//! perlu diverifikasi lebih lanjut.

use oxc_allocator::Allocator;
use oxc_ast::ast::{ClassElement, PropertyKey, Statement, TSEnumMemberName, TSType, TSTypeName};
use oxc_parser::Parser;
use oxc_span::{GetSpan, SourceType};
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
        write!(f, "kode TypeScript tidak bisa diparse: {}", self.0)
    }
}

impl std::error::Error for ImportError {}

/// Mengimpor berkas TypeScript `source` menjadi api-ir/1 bernama `name`.
///
/// # Errors
///
/// Kode yang tidak bisa diparse oleh `oxc_parser`.
pub fn import(source: &str, name: &str) -> Result<Imported, ImportError> {
    let allocator = Allocator::default();
    let parsed = Parser::new(&allocator, source, SourceType::ts()).parse();
    if parsed.fatal_error || parsed.diagnostics.has_errors() {
        return Err(ImportError(format!(
            "{} galat parser",
            parsed.diagnostics.errors().count()
        )));
    }
    let body = &parsed.program.body;

    let defined: BTreeSet<String> = body
        .iter()
        .filter_map(|stmt| match stmt {
            Statement::ClassDeclaration(c) => c.id.as_ref().map(|id| id.name.as_str().to_owned()),
            Statement::TSEnumDeclaration(e) => Some(e.id.name.as_str().to_owned()),
            _ => None,
        })
        .collect();

    let mut types = Map::new();
    let mut report = Report::default();
    for stmt in body {
        let start = stmt.span().start as usize;
        let line = line_of(source, start);
        match stmt {
            Statement::ClassDeclaration(class) => {
                let Some(id) = &class.id else {
                    lose(
                        &mut report,
                        source,
                        line,
                        "kelas tanpa nama tidak dipetakan",
                    );
                    continue;
                };
                let class_name = id.name.as_str().to_owned();
                if class.heritage.is_some() {
                    lose(&mut report, source, line, "pewarisan tidak didukung");
                    continue;
                }
                let mut properties = Map::new();
                let mut required = Vec::new();
                for element in &class.body.body {
                    match element {
                        ClassElement::PropertyDefinition(field) => {
                            let PropertyKey::StaticIdentifier(key) = &field.key else {
                                lose(
                                    &mut report,
                                    source,
                                    line_of(source, field.span.start as usize),
                                    "nama field tidak sederhana",
                                );
                                continue;
                            };
                            let field_name = key.name.as_str().to_owned();
                            let field_line = line_of(source, field.span.start as usize);
                            let Some(annotation) = &field.type_annotation else {
                                lose(
                                    &mut report,
                                    source,
                                    field_line,
                                    &format!("field {field_name}: tanpa anotasi tipe"),
                                );
                                continue;
                            };
                            match type_schema(&annotation.type_annotation, &defined) {
                                Some((schema, nullable)) => {
                                    if !(field.optional || nullable) {
                                        required.push(json!(field_name));
                                    }
                                    properties.insert(field_name, schema);
                                }
                                None => lose(
                                    &mut report,
                                    source,
                                    field_line,
                                    &format!("field {field_name}: tipe tidak dipetakan"),
                                ),
                            }
                        }
                        other => lose(
                            &mut report,
                            source,
                            line_of(source, other.span().start as usize),
                            "anggota kelas tidak dipetakan (metode, accessor, atau blok statis)",
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
            Statement::TSEnumDeclaration(decl) => {
                let enum_name = decl.id.name.as_str().to_owned();
                let mut variants = Vec::new();
                let mut ok = true;
                for member in &decl.body.members {
                    match (&member.id, &member.initializer) {
                        (TSEnumMemberName::Identifier(name), None) => {
                            variants.push(name.name.as_str().to_owned());
                        }
                        _ => {
                            ok = false;
                            break;
                        }
                    }
                }
                if ok {
                    types.insert(enum_name.clone(), json!({ "enum": variants }));
                    report.mapped.push(Mapping {
                        line,
                        name: enum_name,
                    });
                } else {
                    lose(
                        &mut report,
                        source,
                        line,
                        "enum dengan nilai atau nama anggota tidak sederhana",
                    );
                }
            }
            Statement::ExportNamedDeclaration(_) | Statement::ExportDefaultDeclaration(_) => {
                lose(&mut report, source, line, "export belum didukung");
            }
            Statement::FunctionDeclaration(_) => {
                lose(&mut report, source, line, "fungsi tidak dipetakan")
            }
            Statement::ImportDeclaration(_) => {
                lose(&mut report, source, line, "impor tidak dipetakan")
            }
            Statement::VariableDeclaration(_) => lose(
                &mut report,
                source,
                line,
                "variabel tingkat atas tidak dipetakan",
            ),
            Statement::TSInterfaceDeclaration(_) => {
                lose(&mut report, source, line, "interface tidak dipetakan")
            }
            Statement::TSTypeAliasDeclaration(_) => {
                lose(&mut report, source, line, "alias tipe tidak dipetakan")
            }
            _ => lose(&mut report, source, line, "pernyataan tidak dipetakan"),
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

/// Skema untuk satu tipe TypeScript. Mengembalikan `(skema, bisa_null)`.
fn type_schema(ty: &TSType, defined: &BTreeSet<String>) -> Option<(Value, bool)> {
    match ty {
        TSType::TSStringKeyword(_) => Some((json!({ "type": "string" }), false)),
        TSType::TSNumberKeyword(_) => Some((json!({ "type": "number" }), false)),
        TSType::TSBooleanKeyword(_) => Some((json!({ "type": "boolean" }), false)),
        TSType::TSArrayType(array) => {
            type_schema(&array.element_type, defined).map(|(s, _)| (json!({ "array": s }), false))
        }
        TSType::TSTypeReference(reference) => match &reference.type_name {
            TSTypeName::IdentifierReference(id) if defined.contains(id.name.as_str()) => {
                Some((json!({ "$ref": id.name.as_str() }), false))
            }
            _ => None,
        },
        TSType::TSUnionType(union) => {
            // `T | null` atau `T | undefined` menjadi opsional. Union lain tidak dipetakan.
            let mut nullable = false;
            let mut inner: Option<&TSType> = None;
            for member in &union.types {
                match member {
                    TSType::TSNullKeyword(_) | TSType::TSUndefinedKeyword(_) => nullable = true,
                    other => {
                        if inner.is_some() {
                            return None;
                        }
                        inner = Some(other);
                    }
                }
            }
            if !nullable {
                return None;
            }
            inner
                .and_then(|t| type_schema(t, defined))
                .map(|(s, _)| (s, true))
        }
        _ => None,
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
#[path = "import_code_javascript_test.rs"]
mod import_code_javascript_test;
