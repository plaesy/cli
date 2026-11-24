//! Generator OpenAPI 3.1 dari `api-ir/1` (ADR 0005, Fase 4). Murni: tidak ada I/O.
//!
//! Dokumen dicek dengan aturan semantik lebih dulu. Hasilnya OpenAPI 3.1.0 dengan skema di
//! `components/schemas`. `handler` (flow) dan `rate_limit_per_minute` tidak masuk ke skema
//! OpenAPI; keduanya dicatat sebagai ekstensi `x-` pada operasi.

use std::collections::HashSet;

use serde_json::{json, Map, Value};

use crate::rules;

/// Versi OpenAPI yang dihasilkan.
pub const OPENAPI_VERSION: &str = "3.1.0";

/// Memeriksa bahwa setiap `handler.flow_id` ada di `known_flows`.
///
/// Mengembalikan galat untuk setiap endpoint yang merujuk flow tidak dikenal.
#[must_use]
pub fn check_handlers(doc: &Value, known_flows: &HashSet<String>) -> Vec<String> {
    let mut errors = Vec::new();
    for endpoint in endpoints(doc) {
        let id = endpoint.get("id").and_then(Value::as_str).unwrap_or("?");
        match endpoint
            .get("handler")
            .and_then(|h| h.get("flow_id"))
            .and_then(Value::as_str)
        {
            Some(flow_id) if known_flows.contains(flow_id) => {}
            Some(flow_id) => errors.push(format!("{id}: flow_id {flow_id:?} tidak ada")),
            None => errors.push(format!("{id}: handler.flow_id wajib")),
        }
    }
    errors
}

/// Membangun dokumen OpenAPI 3.1 dari `doc`.
///
/// # Errors
///
/// Daftar galat semantik atau bentuk tipe bila dokumen tidak bisa dikonversi.
pub fn to_openapi(doc: &Value) -> Result<Value, Vec<String>> {
    let errors = rules::validate(doc);
    if !errors.is_empty() {
        return Err(errors);
    }

    let mut schemas = Map::new();
    let mut errors = Vec::new();
    for (name, ty) in doc
        .get("types")
        .and_then(Value::as_object)
        .into_iter()
        .flatten()
    {
        match schema(ty, name) {
            Ok(value) => {
                schemas.insert(name.clone(), value);
            }
            Err(error) => errors.push(error),
        }
    }

    let base = doc.get("base_path").and_then(Value::as_str).unwrap_or("");
    let mut paths = Map::new();
    for endpoint in endpoints(doc) {
        match operation(endpoint) {
            Ok((path, method, op)) => {
                let base = base.trim_end_matches('/');
                let full = if path == "/" {
                    if base.is_empty() {
                        "/".to_owned()
                    } else {
                        base.to_owned()
                    }
                } else {
                    format!("{base}{path}")
                };
                let item = paths.entry(full).or_insert_with(|| json!({}));
                if let Some(map) = item.as_object_mut() {
                    map.insert(method, op);
                }
            }
            Err(error) => errors.push(error),
        }
    }
    if !errors.is_empty() {
        return Err(errors);
    }

    let version = doc
        .get("version")
        .map_or_else(|| "1".to_owned(), |v| v.to_string());
    Ok(json!({
        "openapi": OPENAPI_VERSION,
        "info": {
            "title": doc.get("name").and_then(Value::as_str).unwrap_or("api"),
            "version": version
        },
        "paths": paths,
        "components": { "schemas": schemas }
    }))
}

fn endpoints(doc: &Value) -> impl Iterator<Item = &Value> {
    doc.get("endpoints")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
}

fn operation(endpoint: &Value) -> Result<(String, String, Value), String> {
    let id = endpoint.get("id").and_then(Value::as_str).unwrap_or("?");
    let method = endpoint
        .get("method")
        .and_then(Value::as_str)
        .ok_or_else(|| format!("{id}: method wajib"))?
        .to_lowercase();
    let path = endpoint
        .get("path")
        .and_then(Value::as_str)
        .ok_or_else(|| format!("{id}: path wajib"))?
        .to_owned();

    let mut parameters = Vec::new();
    for (name, ty) in endpoint
        .get("path_params")
        .and_then(Value::as_object)
        .into_iter()
        .flatten()
    {
        parameters.push(json!({
            "name": name,
            "in": "path",
            "required": true,
            "schema": schema(ty, &format!("{id}.path_params.{name}"))?
        }));
    }

    let mut op = Map::new();
    op.insert("operationId".to_owned(), json!(id));
    if !parameters.is_empty() {
        op.insert("parameters".to_owned(), json!(parameters));
    }
    if let Some(body) = endpoint.get("body") {
        op.insert(
            "requestBody".to_owned(),
            json!({
                "required": true,
                "content": { "application/json": { "schema": schema(body, &format!("{id}.body"))? } }
            }),
        );
    }
    let mut responses = Map::new();
    for (code, ty) in endpoint
        .get("responses")
        .and_then(Value::as_object)
        .into_iter()
        .flatten()
    {
        let description = format!("HTTP {code}");
        if ty.get("type").and_then(Value::as_str) == Some("null") {
            // Respons tanpa isi (misalnya 204): tidak ada `content`.
            responses.insert(code.clone(), json!({ "description": description }));
            continue;
        }
        responses.insert(
            code.clone(),
            json!({
                "description": description,
                "content": { "application/json": { "schema": schema(ty, &format!("{id}.responses.{code}"))? } }
            }),
        );
    }
    if responses.is_empty() {
        responses.insert(
            "default".to_owned(),
            json!({ "description": "tanpa respons terdefinisi" }),
        );
    }
    op.insert("responses".to_owned(), Value::Object(responses));

    if let Some(scopes) = endpoint.get("auth").and_then(|a| a.get("scope")) {
        op.insert("x-scopes".to_owned(), scopes.clone());
    }
    if let Some(limit) = endpoint.get("rate_limit_per_minute") {
        op.insert("x-rate-limit-per-minute".to_owned(), limit.clone());
    }
    Ok((path, method, Value::Object(op)))
}

/// Mengonversi satu bentuk tipe api-ir ke JSON Schema. `where_` dipakai untuk pesan galat.
fn schema(ty: &Value, where_: &str) -> Result<Value, String> {
    if let Some(reference) = ty.get("$ref").and_then(Value::as_str) {
        return Ok(json!({ "$ref": format!("#/components/schemas/{reference}") }));
    }
    if let Some(values) = ty.get("enum").and_then(Value::as_array) {
        return Ok(json!({ "type": "string", "enum": values }));
    }
    if let Some(item) = ty.get("array") {
        return Ok(json!({ "type": "array", "items": schema(item, where_)? }));
    }
    if let Some(object) = ty.get("object") {
        let mut out = Map::new();
        out.insert("type".to_owned(), json!("object"));
        let mut props = Map::new();
        for (name, prop) in object
            .get("properties")
            .and_then(Value::as_object)
            .into_iter()
            .flatten()
        {
            props.insert(name.clone(), schema(prop, &format!("{where_}.{name}"))?);
        }
        out.insert("properties".to_owned(), Value::Object(props));
        if let Some(required) = object
            .get("required")
            .filter(|r| r.as_array().is_some_and(|a| !a.is_empty()))
        {
            out.insert("required".to_owned(), required.clone());
        }
        return Ok(Value::Object(out));
    }
    if let Some(primitive) = ty.get("type").and_then(Value::as_str) {
        if matches!(
            primitive,
            "string" | "number" | "integer" | "boolean" | "null"
        ) {
            return Ok(json!({ "type": primitive }));
        }
    }
    Err(format!("{where_}: bentuk tipe tidak dikenal"))
}

#[cfg(test)]
#[path = "openapi_test.rs"]
mod openapi_test;
