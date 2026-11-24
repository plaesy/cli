//! Impor kode Rust menjadi `api-ir/1` (ADR 0008, arah kode ke visual).
//!
//! Yang dipetakan: `struct` dengan field bernama, dan `enum` yang semua variannya unit.
//! Field dengan tipe `String`, `bool`, bilangan, `f32`/`f64`, `Option<T>`, `Vec<T>`, atau
//! tipe lain yang didefinisikan di berkas yang sama. Yang lain dicatat di [`Report::lost`]
//! beserta nomor baris dan teks sumbernya.
//!
//! Blok buram (ADR 0008) belum punya node di api-ir/1, jadi kehilangan disimpan di laporan.
//! [`export`] mengubah IR kembali menjadi kode Rust. Impor, ekspor, lalu impor ulang harus
//! menghasilkan `types` yang sama.

use proc_macro2::Span;
use serde_json::{json, Map, Value};
use std::collections::BTreeSet;
use std::fmt;
use syn::{Fields, GenericArgument, Item, PathArguments, Type};

/// Satu konstruksi yang berhasil dipetakan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mapping {
    pub line: usize,
    pub name: String,
}

/// Satu konstruksi yang tidak dipetakan, dengan alasannya.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Loss {
    pub line: usize,
    /// Teks sumber konstruksi, sebagai blok buram.
    pub source: String,
    pub reason: String,
}

/// Laporan impor (ADR 0008): pemetaan dan kehilangan.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Report {
    pub mapped: Vec<Mapping>,
    pub lost: Vec<Loss>,
}

/// Hasil impor: dokumen api-ir/1 dan laporannya.
#[derive(Debug, Clone, PartialEq)]
pub struct Imported {
    pub ir: Value,
    pub report: Report,
}

/// Galat impor. Kode yang tidak bisa diparse sama sekali adalah galat, bukan kehilangan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportError(pub String);

impl fmt::Display for ImportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "kode Rust tidak bisa diparse: {}", self.0)
    }
}

impl std::error::Error for ImportError {}

/// Mengimpor berkas Rust `source` menjadi api-ir/1 bernama `name`.
///
/// # Errors
///
/// Kode yang tidak bisa diparse oleh `syn`.
pub fn import(source: &str, name: &str) -> Result<Imported, ImportError> {
    let file = syn::parse_file(source).map_err(|e| ImportError(e.to_string()))?;
    let defined: BTreeSet<String> = file
        .items
        .iter()
        .filter_map(|item| match item {
            Item::Struct(s) => Some(s.ident.to_string()),
            Item::Enum(e) => Some(e.ident.to_string()),
            _ => None,
        })
        .collect();

    let mut types = Map::new();
    let mut report = Report::default();
    for item in &file.items {
        match item {
            Item::Struct(s) => {
                let name = s.ident.to_string();
                let line = line_of(s.ident.span());
                if !s.generics.params.is_empty() {
                    lose(
                        &mut report,
                        source,
                        s.ident.span(),
                        "struct generik tidak didukung",
                    );
                    continue;
                }
                match &s.fields {
                    Fields::Named(named) => {
                        let mut properties = Map::new();
                        let mut required = Vec::new();
                        for field in &named.named {
                            let field_name = field
                                .ident
                                .as_ref()
                                .map(ToString::to_string)
                                .unwrap_or_default();
                            match field_schema(&field.ty, &defined) {
                                Some((schema, is_required)) => {
                                    if is_required {
                                        required.push(json!(field_name));
                                    }
                                    properties.insert(field_name, schema);
                                }
                                None => lose(
                                    &mut report,
                                    source,
                                    field.ty_span(),
                                    &format!("field {field_name}: tipe tidak dipetakan"),
                                ),
                            }
                        }
                        let mut object = Map::new();
                        object.insert("properties".to_owned(), Value::Object(properties));
                        if !required.is_empty() {
                            object.insert("required".to_owned(), Value::Array(required));
                        }
                        types.insert(name.clone(), json!({ "object": object }));
                        report.mapped.push(Mapping { line, name });
                    }
                    _ => lose(
                        &mut report,
                        source,
                        s.ident.span(),
                        "struct tanpa field bernama",
                    ),
                }
            }
            Item::Enum(e) => {
                let name = e.ident.to_string();
                let line = line_of(e.ident.span());
                if !e.variants.iter().all(|v| matches!(v.fields, Fields::Unit)) {
                    lose(
                        &mut report,
                        source,
                        e.ident.span(),
                        "enum dengan data pada varian tidak didukung",
                    );
                    continue;
                }
                let variants: Vec<String> =
                    e.variants.iter().map(|v| v.ident.to_string()).collect();
                types.insert(name.clone(), json!({ "enum": variants }));
                report.mapped.push(Mapping { line, name });
            }
            other => lose(&mut report, source, span_of(other), item_kind(other)),
        }
    }

    let ir = json!({
        "ir": "api-ir/1",
        "id": format!("api_{}", uuid_simple()),
        "version": 1,
        "name": name,
        "base_path": "/",
        "types": types,
        "endpoints": []
    });
    Ok(Imported { ir, report })
}

/// Mengubah `types` api-ir menjadi kode Rust. Hanya bentuk yang dihasilkan [`import`] yang didukung.
#[must_use]
pub fn export(ir: &Value) -> String {
    let mut out = String::new();
    let Some(types) = ir.get("types").and_then(Value::as_object) else {
        return out;
    };
    for (name, ty) in types {
        if let Some(variants) = ty.get("enum").and_then(Value::as_array) {
            out.push_str("pub enum ");
            out.push_str(name);
            out.push_str(" {\n");
            for variant in variants {
                out.push_str("    ");
                out.push_str(variant.as_str().unwrap_or("Unknown"));
                out.push_str(",\n");
            }
            out.push_str("}\n\n");
        } else if let Some(object) = ty.get("object") {
            out.push_str("pub struct ");
            out.push_str(name);
            out.push_str(" {\n");
            let required: Vec<&str> = object
                .get("required")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                .collect();
            for (field, schema) in object
                .get("properties")
                .and_then(Value::as_object)
                .into_iter()
                .flatten()
            {
                let rust = rust_type(schema);
                let rust = if required.contains(&field.as_str()) {
                    rust
                } else {
                    format!("Option<{rust}>")
                };
                out.push_str(&format!("    pub {field}: {rust},\n"));
            }
            out.push_str("}\n\n");
        }
    }
    out
}

fn rust_type(schema: &Value) -> String {
    if let Some(target) = schema.get("$ref").and_then(Value::as_str) {
        return target.to_owned();
    }
    if let Some(items) = schema.get("array") {
        return format!("Vec<{}>", rust_type(items));
    }
    match schema.get("type").and_then(Value::as_str) {
        Some("boolean") => "bool".to_owned(),
        Some("integer") => "i64".to_owned(),
        Some("number") => "f64".to_owned(),
        _ => "String".to_owned(),
    }
}

/// Skema field dan apakah field itu wajib. `None` bila tipenya tidak dipetakan.
fn field_schema(ty: &Type, defined: &BTreeSet<String>) -> Option<(Value, bool)> {
    if let Some(inner) = generic_arg(ty, "Option") {
        return field_schema(inner, defined).map(|(schema, _)| (schema, false));
    }
    if let Some(inner) = generic_arg(ty, "Vec") {
        return field_schema(inner, defined).map(|(schema, _)| (json!({ "array": schema }), true));
    }
    let ident = single_ident(ty)?;
    let schema = match ident.as_str() {
        "String" | "str" => json!({ "type": "string" }),
        "bool" => json!({ "type": "boolean" }),
        "i8" | "i16" | "i32" | "i64" | "isize" | "u8" | "u16" | "u32" | "u64" | "usize" => {
            json!({ "type": "integer" })
        }
        "f32" | "f64" => json!({ "type": "number" }),
        other if defined.contains(other) => json!({ "$ref": other }),
        _ => return None,
    };
    Some((schema, true))
}

/// Argumen generik tunggal `Wrapper<T>`, bila `ty` memang bentuk itu.
fn generic_arg<'a>(ty: &'a Type, wrapper: &str) -> Option<&'a Type> {
    let Type::Path(path) = ty else { return None };
    let segment = path.path.segments.last()?;
    if segment.ident != wrapper {
        return None;
    }
    let PathArguments::AngleBracketed(args) = &segment.arguments else {
        return None;
    };
    match args.args.first()? {
        GenericArgument::Type(inner) if args.args.len() == 1 => Some(inner),
        _ => None,
    }
}

fn single_ident(ty: &Type) -> Option<String> {
    let Type::Path(path) = ty else { return None };
    if path.qself.is_some() || path.path.segments.len() != 1 {
        return None;
    }
    let segment = &path.path.segments[0];
    if !matches!(segment.arguments, PathArguments::None) {
        return None;
    }
    Some(segment.ident.to_string())
}

fn lose(report: &mut Report, source: &str, span: Span, reason: &str) {
    let line = line_of(span);
    report.lost.push(Loss {
        line,
        source: source_line(source, line),
        reason: reason.to_owned(),
    });
}

fn line_of(span: Span) -> usize {
    span.start().line
}

fn source_line(source: &str, line: usize) -> String {
    source
        .lines()
        .nth(line.saturating_sub(1))
        .unwrap_or("")
        .trim()
        .to_owned()
}

/// Rentang sumber dari item yang bukan struct atau enum. `syn` memberi span lewat `ToTokens`.
fn span_of(item: &Item) -> Span {
    use syn::spanned::Spanned;
    item.span()
}

trait FieldSpan {
    fn ty_span(&self) -> Span;
}

impl FieldSpan for syn::Field {
    fn ty_span(&self) -> Span {
        use syn::spanned::Spanned;
        self.ty.span()
    }
}

fn item_kind(item: &Item) -> &'static str {
    match item {
        Item::Fn(_) => "fungsi tidak dipetakan",
        Item::Impl(_) => "impl tidak dipetakan",
        Item::Use(_) => "use tidak dipetakan",
        Item::Const(_) | Item::Static(_) => "konstanta tidak dipetakan",
        Item::Trait(_) => "trait tidak dipetakan",
        Item::Mod(_) => "modul tidak dipetakan",
        Item::Type(_) => "alias tipe tidak dipetakan",
        _ => "item tidak dipetakan",
    }
}

fn uuid_simple() -> String {
    uuid::Uuid::new_v4().simple().to_string()
}

#[cfg(test)]
#[path = "import_code_rust_test.rs"]
mod import_code_rust_test;
