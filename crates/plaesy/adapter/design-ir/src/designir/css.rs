//! Variabel CSS dari token `design-ir/1` (ADR 0010, "Konversi").
//!
//! Tema punya dua mode. Nilai dokumen adalah mode terang dan ditulis di `:root`.
//! Mode gelap memakai `overrides` terpisah, sehingga hanya token yang ditimpa yang
//! ditulis, di `@media (prefers-color-scheme: dark)` dan `[data-theme="dark"]`.
//! Skema `design-ir/1` menutup objek leaf token, jadi nilai gelap tidak disimpan di
//! dokumen; ia diberikan sebagai input terpisah.
//!
//! Pemetaan nama: `color.primary` menjadi `--color-primary` (titik dan garis bawah
//! menjadi tanda hubung). Tipografi menjadi `-family`, `-size`, dan `-weight`. Bayangan
//! menjadi satu nilai `offsetX offsetY blur [spread] color`. Rujukan `{color.primary}`
//! menjadi `var(--color-primary)`.

use std::collections::BTreeMap;
use std::fmt::Write as _;

use serde_json::{Map, Value};
use thiserror::Error;

use super::{flatten_tokens, str_of};

/// Galat pemetaan token ke variabel CSS.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum CssError {
    /// Nama token tidak bisa dipetakan ke nama variabel CSS yang valid.
    #[error("nama token tidak bisa dipetakan ke variabel CSS: {0}")]
    BadName(String),
    /// Dua token menghasilkan nama variabel CSS yang sama.
    #[error("dua token memakai variabel CSS yang sama: {0}")]
    Collision(String),
    /// Mode gelap menimpa token yang tidak ada di dokumen.
    #[error("override mode gelap untuk token yang tidak ada: {0}")]
    UnknownToken(String),
    /// Nilai token tidak sesuai tipenya, atau berisi karakter yang tidak aman di CSS.
    #[error("nilai token {0} tidak valid: {1}")]
    Malformed(String, String),
}

/// Mode tema yang bisa diminta.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThemeMode {
    Light,
    Dark,
}

/// Tema yang sudah dipetakan ke variabel CSS. Kunci adalah nama lengkap, misalnya
/// `--color-primary`. Kedua peta terurut sehingga keluarannya deterministik.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Theme {
    light: BTreeMap<String, String>,
    dark: BTreeMap<String, String>,
}

impl Theme {
    /// Variabel mode terang, semua token dokumen.
    pub fn light(&self) -> &BTreeMap<String, String> {
        &self.light
    }

    /// Hanya token yang ditimpa untuk mode gelap.
    pub fn dark(&self) -> &BTreeMap<String, String> {
        &self.dark
    }

    /// Variabel lengkap untuk `mode`. Mode gelap mewarisi nilai terang untuk token
    /// yang tidak ditimpa.
    pub fn resolved(&self, mode: ThemeMode) -> BTreeMap<String, String> {
        let mut out = self.light.clone();
        if mode == ThemeMode::Dark {
            out.extend(self.dark.clone());
        }
        out
    }

    /// Lembar gaya CSS: blok `:root` untuk mode terang, dan blok mode gelap hanya
    /// bila ada override. Blok mode gelap ditulis dua kali, untuk preferensi sistem
    /// dan untuk atribut `data-theme`.
    pub fn css(&self) -> String {
        let mut out = String::new();
        push_block(&mut out, ":root", &self.light, "");
        if !self.dark.is_empty() {
            out.push_str("@media (prefers-color-scheme: dark) {\n");
            push_block(
                &mut out,
                ":root:not([data-theme=\"light\"])",
                &self.dark,
                "  ",
            );
            out.push_str("}\n");
            push_block(&mut out, ":root[data-theme=\"dark\"]", &self.dark, "");
        }
        out
    }
}

/// Membangun tema dari `doc` (mode terang) dan `dark` (override mode gelap, dengan
/// kunci nama token lengkap seperti `color.surface`).
pub fn build_theme(doc: &Value, dark: &BTreeMap<String, Value>) -> Result<Theme, CssError> {
    let tokens = flatten_tokens(doc);
    let mut light: BTreeMap<String, String> = BTreeMap::new();
    let mut by_name: BTreeMap<&str, &Value> = BTreeMap::new();
    for (name, token) in &tokens {
        by_name.insert(name.as_str(), token);
        let Some(value) = token.get("$value") else {
            continue;
        };
        for (var, text) in expand(name, str_of(token, "$type"), value)? {
            if light.contains_key(&var) {
                return Err(CssError::Collision(var));
            }
            light.insert(var, text);
        }
    }

    let mut overrides: BTreeMap<String, String> = BTreeMap::new();
    for (name, value) in dark {
        let token: &Value = by_name
            .get(name.as_str())
            .copied()
            .ok_or_else(|| CssError::UnknownToken(name.clone()))?;
        for (var, text) in expand(name, str_of(token, "$type"), value)? {
            overrides.insert(var, text);
        }
    }

    Ok(Theme {
        light,
        dark: overrides,
    })
}

fn push_block(out: &mut String, selector: &str, vars: &BTreeMap<String, String>, indent: &str) {
    let _ = writeln!(out, "{indent}{selector} {{");
    for (name, value) in vars {
        let _ = writeln!(out, "{indent}  {name}: {value};");
    }
    let _ = writeln!(out, "{indent}}}");
}

/// Mengubah satu nilai token menjadi pasangan (nama variabel, nilai CSS). Tipografi
/// dan bayangan menghasilkan lebih dari satu pasangan atau satu nilai gabungan.
fn expand(token: &str, ty: Option<&str>, value: &Value) -> Result<Vec<(String, String)>, CssError> {
    let base = var_name(token)?;
    match value {
        Value::String(text) => Ok(vec![(base, css_text(token, text)?)]),
        Value::Number(number) => Ok(vec![(base, number.to_string())]),
        Value::Object(object) => match ty {
            Some("typography") => expand_typography(token, &base, object),
            Some("shadow") => expand_shadow(token, &base, object),
            _ => Err(malformed(
                token,
                "objek hanya dipakai untuk typography atau shadow",
            )),
        },
        _ => Err(malformed(token, "nilai bukan teks, angka, atau objek")),
    }
}

fn expand_typography(
    token: &str,
    base: &str,
    object: &Map<String, Value>,
) -> Result<Vec<(String, String)>, CssError> {
    let mut family = field(token, object, "fontFamily")?;
    if family.contains(' ') && !(family.starts_with('"') || family.starts_with('\'')) {
        family = format!("\"{family}\"");
    }
    Ok(vec![
        (format!("{base}-family"), family),
        (format!("{base}-size"), field(token, object, "fontSize")?),
        (
            format!("{base}-weight"),
            field(token, object, "fontWeight")?,
        ),
    ])
}

fn expand_shadow(
    token: &str,
    base: &str,
    object: &Map<String, Value>,
) -> Result<Vec<(String, String)>, CssError> {
    let mut parts = vec![
        field(token, object, "offsetX")?,
        field(token, object, "offsetY")?,
        field(token, object, "blur")?,
    ];
    if object.contains_key("spread") {
        parts.push(field(token, object, "spread")?);
    }
    parts.push(field(token, object, "color")?);
    Ok(vec![(base.to_string(), parts.join(" "))])
}

/// Membaca satu field teks atau angka dari objek token.
fn field(token: &str, object: &Map<String, Value>, key: &str) -> Result<String, CssError> {
    match object.get(key) {
        Some(Value::String(text)) => css_text(token, text),
        Some(Value::Number(number)) => Ok(number.to_string()),
        Some(_) => Err(malformed(token, &format!("{key} bukan teks atau angka"))),
        None => Err(malformed(token, &format!("{key} tidak ada"))),
    }
}

/// Nama variabel untuk token `token`. Hanya huruf kecil, angka, dan tanda hubung
/// yang boleh; titik dan garis bawah menjadi tanda hubung.
fn var_name(token: &str) -> Result<String, CssError> {
    let mapped: String = token
        .chars()
        .map(|c| if c == '.' || c == '_' { '-' } else { c })
        .collect();
    let valid = mapped
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-');
    if mapped.is_empty() || !valid {
        return Err(CssError::BadName(token.to_string()));
    }
    Ok(format!("--{mapped}"))
}

/// Nilai teks sebagai nilai CSS. Rujukan `{nama.token}` menjadi `var(...)`. Karakter
/// yang bisa menutup deklarasi atau blok ditolak.
fn css_text(token: &str, raw: &str) -> Result<String, CssError> {
    if let Some(inner) = raw.strip_prefix('{').and_then(|s| s.strip_suffix('}')) {
        return Ok(format!("var({})", var_name(inner)?));
    }
    if raw
        .chars()
        .any(|c| matches!(c, ';' | '{' | '}' | '<' | '>' | '\n' | '\r'))
    {
        return Err(malformed(token, "mengandung karakter yang tidak diizinkan"));
    }
    Ok(raw.to_string())
}

fn malformed(token: &str, why: &str) -> CssError {
    CssError::Malformed(token.to_string(), why.to_string())
}

#[cfg(test)]
#[path = "css_test.rs"]
mod css_test;
