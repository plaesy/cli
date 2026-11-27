//! Membaca dokumen `data-ir/1` menjadi tabel dan relasi untuk tampilan. Murni: tanpa I/O.
//! Relasi `one_to_many` dan `one_to_one` memakai `parent` dan `child`. `many_to_many` memakai
//! `left` dan `right`, dan keduanya dipetakan ke ujung parent dan child.

use super::model::{ColumnView, RelationView, TableView};
use crate::adapter::http::document_path;
use crate::features::ApiRequest;
use serde_json::{json, Value};

/// Kind dokumen data di server.
pub const DATA_KIND: &str = "data-ir/1";

/// Tabel dan relasi dari dokumen `data-ir/1`.
///
/// # Errors
///
/// Bila `tables` tidak ada, atau ada field wajib yang hilang atau bukan teks.
pub fn tables_from_document(doc: &Value) -> Result<(Vec<TableView>, Vec<RelationView>), String> {
    let tables = doc
        .get("tables")
        .and_then(Value::as_array)
        .ok_or_else(|| "dokumen tidak punya daftar `tables`".to_owned())?
        .iter()
        .map(table_view)
        .collect::<Result<Vec<_>, String>>()?;
    let relations = match doc.get("relations").and_then(Value::as_array) {
        Some(items) => items
            .iter()
            .map(relation_view)
            .collect::<Result<Vec<_>, String>>()?,
        None => Vec::new(),
    };
    Ok((tables, relations))
}

/// Nama dokumen (`name`) dan id-nya, bila ada. Dipakai daftar dokumen data.
#[must_use]
pub fn document_name(doc: &Value) -> Option<(String, String)> {
    let id = doc.get("id").and_then(Value::as_str)?.to_owned();
    let name = doc
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or(&id)
        .to_owned();
    Some((id, name))
}

/// Menulis ulang `tables` dari `original` sesuai tabel hasil edit. Objek kolom yang sudah ada
/// dipertahankan apa adanya (termasuk tipe `enum` atau `decimal`). Kolom baru bertipe `text`.
/// Field lain di setiap tabel (mis. `indexes`) dan seluruh dokumen dipertahankan.
#[must_use]
pub fn patch_tables(original: &Value, tables: &[TableView]) -> Value {
    let mut doc = original.clone();
    let existing: Vec<Value> = original
        .get("tables")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let patched: Vec<Value> = tables
        .iter()
        .map(|table| {
            let previous = existing
                .iter()
                .find(|t| t.get("name").and_then(Value::as_str) == Some(table.name.as_str()))
                .and_then(Value::as_object)
                .cloned();
            let old_columns: Vec<Value> = previous
                .as_ref()
                .and_then(|object| object.get("columns"))
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            let columns: Vec<Value> = table
                .columns
                .iter()
                .map(|column| {
                    old_columns
                        .iter()
                        .find(|c| c.get("name").and_then(Value::as_str) == Some(column.name.as_str()))
                        .cloned()
                        .unwrap_or_else(|| {
                            json!({ "name": column.name, "type": "text", "nullable": column.nullable })
                        })
                })
                .collect();
            let mut object = previous.unwrap_or_default();
            object.insert("name".to_owned(), json!(table.name));
            object.insert("columns".to_owned(), Value::Array(columns));
            object.insert("primary_key".to_owned(), json!(table.primary_key));
            Value::Object(object)
        })
        .collect();
    if let Some(fields) = doc.as_object_mut() {
        fields.insert("tables".to_owned(), Value::Array(patched));
    }
    doc
}

/// Permintaan untuk memuat dokumen data dengan id tertentu.
#[must_use]
pub fn load_request(doc_id: &str) -> ApiRequest {
    ApiRequest {
        method: "GET",
        path: document_path(DATA_KIND, doc_id),
        body: None,
    }
}

/// Permintaan untuk menyimpan dokumen data dengan id tertentu.
#[must_use]
pub fn save_request(doc_id: &str, body: &str) -> ApiRequest {
    ApiRequest {
        method: "PUT",
        path: document_path(DATA_KIND, doc_id),
        body: Some(body.to_owned()),
    }
}

fn table_view(value: &Value) -> Result<TableView, String> {
    let name = str_field(value, "name")?;
    let columns = value
        .get("columns")
        .and_then(Value::as_array)
        .ok_or_else(|| format!("tabel {name} tidak punya `columns`"))?
        .iter()
        .map(column_view)
        .collect::<Result<Vec<_>, String>>()?;
    Ok(TableView {
        name,
        columns,
        primary_key: str_list(value.get("primary_key")),
    })
}

fn column_view(value: &Value) -> Result<ColumnView, String> {
    Ok(ColumnView {
        name: str_field(value, "name")?,
        data_type: type_label(value.get("type")),
        nullable: value
            .get("nullable")
            .and_then(Value::as_bool)
            .unwrap_or(false),
    })
}

/// Teks tipe kolom: `text`, `enum(a|b)`, atau `decimal(p,s)`. Tipe lain ditampilkan `tidak dikenal`.
fn type_label(value: Option<&Value>) -> String {
    match value {
        Some(Value::String(text)) => text.clone(),
        Some(Value::Object(obj)) => {
            if let Some(values) = obj.get("enum").and_then(Value::as_array) {
                let items: Vec<&str> = values.iter().filter_map(Value::as_str).collect();
                format!("enum({})", items.join("|"))
            } else if let Some(decimal) = obj.get("decimal") {
                let precision = decimal
                    .get("precision")
                    .and_then(Value::as_u64)
                    .unwrap_or(0);
                let scale = decimal.get("scale").and_then(Value::as_u64).unwrap_or(0);
                format!("decimal({precision},{scale})")
            } else {
                "tidak dikenal".to_owned()
            }
        }
        _ => "tidak dikenal".to_owned(),
    }
}

fn relation_view(value: &Value) -> Result<RelationView, String> {
    let id = str_field(value, "id")?;
    let parent = endpoint(value.get("parent").or_else(|| value.get("left")), &id)?;
    let child = endpoint(value.get("child").or_else(|| value.get("right")), &id)?;
    Ok(RelationView { id, parent, child })
}

fn endpoint(value: Option<&Value>, relation: &str) -> Result<(String, Vec<String>), String> {
    let value = value.ok_or_else(|| format!("relasi {relation} tidak punya ujung"))?;
    Ok((str_field(value, "table")?, str_list(value.get("columns"))))
}

fn str_field(value: &Value, key: &str) -> Result<String, String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| format!("field `{key}` hilang atau bukan teks"))
}

fn str_list(value: Option<&Value>) -> Vec<String> {
    value
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(test)]
#[path = "document_test.rs"]
mod document_test;
