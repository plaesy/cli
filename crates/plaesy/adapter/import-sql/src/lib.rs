//! Impor skema SQLite menjadi dokumen `data-ir/1` (ADR 0004, arah kode ke visual).
//!
//! Dialek pertama adalah SQLite. Keputusan ini didelegasikan dan bisa dicabut. Tipe yang
//! tidak punya padanan di `data-ir/1` (misalnya `REAL`) menghasilkan [`ImportError`], tidak
//! diterjemahkan diam-diam. Hasil impor sebaiknya dicek dengan `plaesy_domain_data::validate`.

use rusqlite::Connection;
use serde_json::{json, Map, Value};
use std::fmt;

/// Galat impor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImportError {
    /// Tipe SQLite tidak punya padanan di data-ir/1.
    UnsupportedType {
        table: String,
        column: String,
        declared: String,
    },
    /// Galat dari SQLite.
    Sqlite(String),
}

impl fmt::Display for ImportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedType {
                table,
                column,
                declared,
            } => write!(
                f,
                "{table}.{column}: tipe {declared:?} tidak punya padanan di data-ir/1"
            ),
            Self::Sqlite(message) => write!(f, "sqlite: {message}"),
        }
    }
}

impl std::error::Error for ImportError {}

fn sql(error: rusqlite::Error) -> ImportError {
    ImportError::Sqlite(error.to_string())
}

/// Membaca skema `conn` dan membangun dokumen `data-ir/1` bernama `name`.
///
/// # Errors
///
/// Tipe yang tidak didukung, atau galat SQLite.
pub fn import(conn: &Connection, name: &str) -> Result<Value, ImportError> {
    let tables = table_names(conn)?;
    let mut table_docs = Vec::new();
    let mut relations = Vec::new();

    for table in &tables {
        let (columns, pk_columns) = columns_of(conn, table)?;
        let mut column_docs = Vec::new();
        let mut unique_columns = Vec::new();
        let mut indexes = Vec::new();

        for index in index_list(conn, table)? {
            let index_columns = index_columns(conn, &index.name)?;
            // Indeks otomatis dari PRIMARY KEY tidak dicatat sebagai indeks terpisah.
            if index.origin == "pk" {
                continue;
            }
            if index.origin == "u" && index.unique && index_columns.len() == 1 {
                unique_columns.push(index_columns[0].clone());
                continue;
            }
            if index.origin == "u" {
                continue; // UNIQUE multi-kolom ditangani sebagai indeks unik di bawah.
            }
            indexes.push(json!({
                "name": index.name,
                "columns": index_columns,
                "unique": index.unique
            }));
        }
        for index in index_list(conn, table)?
            .into_iter()
            .filter(|i| i.origin == "u" && i.unique)
        {
            let cols = index_columns(conn, &index.name)?;
            if cols.len() > 1 {
                indexes.push(json!({ "name": index.name, "columns": cols, "unique": true }));
            }
        }

        for column in &columns {
            let mut doc = Map::new();
            doc.insert("name".to_owned(), json!(column.name));
            doc.insert("type".to_owned(), column_type(table, column)?);
            doc.insert("nullable".to_owned(), json!(!column.not_null));
            if let Some(default) = &column.default {
                doc.insert("default".to_owned(), default.clone());
            }
            if unique_columns.contains(&column.name) {
                doc.insert("unique".to_owned(), json!(true));
            }
            column_docs.push(Value::Object(doc));
        }

        table_docs.push(json!({
            "name": table,
            "columns": column_docs,
            "primary_key": pk_columns,
            "indexes": indexes
        }));

        relations.extend(foreign_keys(conn, table)?);
    }

    Ok(json!({
        "ir": "data-ir/1",
        "id": format!("dat_{}", uuid::Uuid::new_v4().simple()),
        "version": 1,
        "name": name,
        "tables": table_docs,
        "relations": relations,
        "queries": []
    }))
}

struct Column {
    name: String,
    declared: String,
    not_null: bool,
    default: Option<Value>,
}

fn table_names(conn: &Connection) -> Result<Vec<String>, ImportError> {
    let mut statement = conn
        .prepare("SELECT name FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%' ORDER BY name")
        .map_err(sql)?;
    let rows = statement
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(sql)?;
    rows.collect::<Result<_, _>>().map_err(sql)
}

/// Kolom dan daftar primary key (dalam urutan kunci).
fn columns_of(conn: &Connection, table: &str) -> Result<(Vec<Column>, Vec<String>), ImportError> {
    let mut statement = conn
        .prepare(&format!(
            "PRAGMA table_info(\"{}\")",
            table.replace('"', "\"\"")
        ))
        .map_err(sql)?;
    let rows = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, bool>(3)?,
                row.get::<_, Option<String>>(4)?,
                row.get::<_, i64>(5)?,
            ))
        })
        .map_err(sql)?;
    let mut columns = Vec::new();
    let mut pk: Vec<(i64, String)> = Vec::new();
    for row in rows {
        let (name, declared, not_null, default, pk_position) = row.map_err(sql)?;
        if pk_position > 0 {
            pk.push((pk_position, name.clone()));
        }
        columns.push(Column {
            name,
            declared,
            not_null,
            default: default.map(|text| default_value(&text)),
        });
    }
    pk.sort();
    Ok((columns, pk.into_iter().map(|(_, name)| name).collect()))
}

/// Mengubah teks default SQLite ke nilai JSON. Ekspresi yang tidak dikenal dilewati.
fn default_value(text: &str) -> Value {
    let trimmed = text.trim();
    if trimmed.eq_ignore_ascii_case("null") {
        return Value::Null;
    }
    if let Some(inner) = trimmed
        .strip_prefix('\'')
        .and_then(|s| s.strip_suffix('\''))
    {
        return json!(inner.replace("''", "'"));
    }
    if trimmed.eq_ignore_ascii_case("true") {
        return json!(true);
    }
    if trimmed.eq_ignore_ascii_case("false") {
        return json!(false);
    }
    if let Ok(number) = trimmed.parse::<i64>() {
        return json!(number);
    }
    Value::Null
}

/// Padanan tipe data-ir/1 untuk satu kolom SQLite.
fn column_type(table: &str, column: &Column) -> Result<Value, ImportError> {
    let declared = column.declared.trim();
    let upper = declared.to_ascii_uppercase();
    let base = upper.split('(').next().unwrap_or("").trim().to_owned();
    let unsupported = || ImportError::UnsupportedType {
        table: table.to_owned(),
        column: column.name.clone(),
        declared: declared.to_owned(),
    };
    let primitive = match base.as_str() {
        "" => return Ok(json!("text")),
        "BOOLEAN" | "BOOL" => "boolean",
        "DATE" => "date",
        "DATETIME" | "TIMESTAMP" => "timestamp",
        "UUID" => "uuid",
        "JSON" => "json",
        "BLOB" => "binary",
        "NUMERIC" | "DECIMAL" => {
            let (precision, scale) = decimal_args(&upper).ok_or_else(unsupported)?;
            return Ok(json!({ "decimal": { "precision": precision, "scale": scale } }));
        }
        other if other.contains("INT") => "integer",
        other if other.contains("CHAR") || other.contains("CLOB") || other.contains("TEXT") => {
            "text"
        }
        _ => return Err(unsupported()),
    };
    Ok(json!(primitive))
}

/// `NUMERIC(p, s)` atau `DECIMAL(p, s)` menjadi (p, s).
fn decimal_args(upper: &str) -> Option<(u32, u32)> {
    let inside = upper.split_once('(')?.1.strip_suffix(')')?;
    let mut parts = inside.split(',').map(|p| p.trim().parse::<u32>());
    let precision = parts.next()?.ok()?;
    let scale = parts.next().unwrap_or(Ok(0)).ok()?;
    if (1..=38).contains(&precision) && scale <= precision {
        Some((precision, scale))
    } else {
        None
    }
}

struct IndexInfo {
    name: String,
    unique: bool,
    origin: String,
}

fn index_list(conn: &Connection, table: &str) -> Result<Vec<IndexInfo>, ImportError> {
    let mut statement = conn
        .prepare(&format!(
            "PRAGMA index_list(\"{}\")",
            table.replace('"', "\"\"")
        ))
        .map_err(sql)?;
    let rows = statement
        .query_map([], |row| {
            Ok(IndexInfo {
                name: row.get::<_, String>(1)?,
                unique: row.get::<_, bool>(2)?,
                origin: row.get::<_, String>(3)?,
            })
        })
        .map_err(sql)?;
    rows.collect::<Result<_, _>>().map_err(sql)
}

fn index_columns(conn: &Connection, index: &str) -> Result<Vec<String>, ImportError> {
    let mut statement = conn
        .prepare(&format!(
            "PRAGMA index_info(\"{}\")",
            index.replace('"', "\"\"")
        ))
        .map_err(sql)?;
    let rows = statement
        .query_map([], |row| row.get::<_, String>(2))
        .map_err(sql)?;
    rows.collect::<Result<_, _>>().map_err(sql)
}

/// Satu relasi `one_to_many` per foreign key. Klausa `ON DELETE` dipetakan ke
/// `restrict`, `cascade`, atau `set_null`. `NO ACTION` (default SQLite) dianggap `restrict`.
fn foreign_keys(conn: &Connection, table: &str) -> Result<Vec<Value>, ImportError> {
    let mut statement = conn
        .prepare(&format!(
            "PRAGMA foreign_key_list(\"{}\")",
            table.replace('"', "\"\"")
        ))
        .map_err(sql)?;
    let rows = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, Option<String>>(4)?,
                row.get::<_, String>(6)?,
            ))
        })
        .map_err(sql)?;
    // Baris dengan id yang sama adalah satu foreign key komposit.
    let mut grouped: Vec<ForeignKey> = Vec::new();
    for row in rows {
        let (id, parent, from, to, on_delete) = row.map_err(sql)?;
        match grouped.iter_mut().find(|g| g.id == id) {
            Some(group) => {
                group.child_columns.push(from);
                group.parent_columns.push(to.unwrap_or_default());
            }
            None => grouped.push(ForeignKey {
                id,
                parent,
                child_columns: vec![from],
                parent_columns: vec![to.unwrap_or_default()],
                on_delete,
            }),
        }
    }
    let mut relations = Vec::new();
    for fk in grouped {
        let ForeignKey {
            id,
            parent,
            child_columns,
            parent_columns,
            on_delete,
        } = fk;
        let parent_columns = if parent_columns.iter().all(String::is_empty) {
            // Foreign key tanpa kolom induk eksplisit merujuk ke primary key induk.
            columns_of(conn, &parent)?.1
        } else {
            parent_columns
        };
        relations.push(json!({
            "id": format!("rel_{table}_{parent}_{id}"),
            "kind": "one_to_many",
            "parent": { "table": parent, "columns": parent_columns },
            "child": { "table": table, "columns": child_columns },
            "on_delete": map_on_delete(&on_delete)
        }));
    }
    Ok(relations)
}

/// Satu foreign key, dengan kolom komposit bila id-nya sama.
struct ForeignKey {
    id: i64,
    parent: String,
    child_columns: Vec<String>,
    parent_columns: Vec<String>,
    on_delete: String,
}

fn map_on_delete(action: &str) -> &'static str {
    match action.to_ascii_uppercase().as_str() {
        "CASCADE" => "cascade",
        "SET NULL" => "set_null",
        _ => "restrict",
    }
}

#[cfg(test)]
#[path = "import_sql_test.rs"]
mod import_sql_test;
