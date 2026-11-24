//! Codegen `data-ir/1` ke DDL dan query SQL berparameter (ADR 0004, `codegen-sql`).
//!
//! Dialek pertama: SQLite. Keputusan ini default dan bisa dicabut.
//! - Tipe: text->TEXT, integer->INTEGER, boolean->INTEGER (0/1), timestamp->TEXT (ISO-8601),
//!   uuid->TEXT, json->TEXT, binary->BLOB, decimal->NUMERIC (SQLite tidak memeriksa presisi),
//!   enum->TEXT dengan CHECK.
//! - Relasi `one_to_many` dan `one_to_one` menjadi FOREIGN KEY di tabel anak. `one_to_one`
//!   juga memberi UNIQUE pada kolom anak. `on_delete` restrict->NO ACTION, cascade->CASCADE,
//!   set_null->SET NULL.
//! - Relasi `many_to_many` menjadi tabel junction dengan kedua FOREIGN KEY dan PRIMARY KEY gabungan.
//!   Junction tidak punya `on_delete` di ADR, jadi memakai NO ACTION (default SQLite).
//! - Query tidak pernah menyambung nilai ke string SQL. Nilai literal dan parameter diikat
//!   sebagai nama parameter `:nama`. Identifier diquote dengan tanda kutip ganda.
//!
//! Validasi memakai `plaesy_domain_data::validate` lebih dulu.

use plaesy_domain_data::validate as validate_data;
use serde_json::Value;
use std::fmt;

/// Galat codegen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SqlError {
    /// Dokumen melanggar aturan semantik data-ir.
    Invalid(Vec<String>),
    /// Tipe kolom atau bentuk query tidak didukung.
    Unsupported(String),
}

impl fmt::Display for SqlError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Invalid(errors) => write!(f, "data-ir tidak valid: {}", errors.join("; ")),
            Self::Unsupported(what) => write!(f, "tidak didukung: {what}"),
        }
    }
}

impl std::error::Error for SqlError {}

/// Satu query yang sudah dikompilasi.
/// - `params`: nama parameter yang dideklarasikan query (tanpa `:`), diisi pemanggil saat eksekusi.
/// - `literals`: nilai literal dari query, sudah diberi nama `litN`. Pemanggil mengikatnya apa adanya.
#[derive(Debug, Clone, PartialEq)]
pub struct CompiledQuery {
    pub name: String,
    pub sql: String,
    pub params: Vec<String>,
    pub literals: Vec<(String, Value)>,
}

/// Membuat DDL SQLite untuk seluruh skema `doc`.
///
/// # Errors
///
/// [`SqlError::Invalid`] bila dokumen melanggar aturan semantik, atau [`SqlError::Unsupported`].
pub fn ddl(doc: &Value) -> Result<String, SqlError> {
    check(doc)?;
    let tables = tables_index(doc);
    let mut out = String::new();

    for table in array(doc, "tables") {
        let name = str_of(table, "name");
        out.push_str(&format!("CREATE TABLE {} (\n", quote(name)));
        let mut lines = Vec::new();
        for column in array(table, "columns") {
            lines.push(format!("  {}", column_ddl(name, column)?));
        }
        let pk = strings(array(table, "primary_key"));
        if !pk.is_empty() {
            lines.push(format!("  PRIMARY KEY ({})", quote_list(&pk)));
        }
        for rel in array(doc, "relations") {
            if kind(rel) == "many_to_many" {
                continue;
            }
            let child = rel.get("child").unwrap_or(&Value::Null);
            if str_of(child, "table") != name {
                continue;
            }
            lines.push(format!("  {}", foreign_key(rel)?));
            if kind(rel) == "one_to_one" {
                lines.push(format!(
                    "  UNIQUE ({})",
                    quote_list(&strings(array(child, "columns")))
                ));
            }
        }
        // Junction yang dideklarasikan sebagai tabel: FOREIGN KEY-nya ditambahkan di sini.
        for rel in array(doc, "relations") {
            if kind(rel) == "many_to_many"
                && str_of(rel.get("junction").unwrap_or(&Value::Null), "table") == name
            {
                lines.extend(junction_foreign_keys(rel));
            }
        }
        out.push_str(&lines.join(",\n"));
        out.push_str("\n);\n");
        for index in array(table, "indexes") {
            out.push_str(&index_ddl(name, index));
        }
    }

    for rel in array(doc, "relations") {
        if kind(rel) != "many_to_many" {
            continue;
        }
        // Tabel junction yang sudah dideklarasikan di `tables` tidak dibuat dua kali.
        let junction_name = str_of(rel.get("junction").unwrap_or(&Value::Null), "table");
        if tables.contains(junction_name) {
            continue;
        }
        out.push_str(&junction_ddl(rel, &tables)?);
    }
    Ok(out)
}

/// Membuat semua query dari `doc` sebagai SQL berparameter.
///
/// # Errors
///
/// [`SqlError::Invalid`] bila dokumen melanggar aturan semantik, atau [`SqlError::Unsupported`].
pub fn queries(doc: &Value) -> Result<Vec<CompiledQuery>, SqlError> {
    check(doc)?;
    let mut out = Vec::new();
    for query in array(doc, "queries") {
        out.push(compile_query(doc, query)?);
    }
    Ok(out)
}

fn check(doc: &Value) -> Result<(), SqlError> {
    let errors: Vec<String> = validate_data(doc).iter().map(ToString::to_string).collect();
    if errors.is_empty() {
        Ok(())
    } else {
        Err(SqlError::Invalid(errors))
    }
}

fn column_ddl(table: &str, column: &Value) -> Result<String, SqlError> {
    let name = str_of(column, "name");
    let ty = column.get("type").unwrap_or(&Value::Null);
    let (sql_type, check) = sql_type(table, name, ty)?;
    let mut parts = vec![quote(name), sql_type.to_owned()];
    if !column
        .get("nullable")
        .and_then(Value::as_bool)
        .unwrap_or(true)
    {
        parts.push("NOT NULL".to_owned());
    }
    if let Some(default) = column.get("default") {
        parts.push(format!("DEFAULT {}", literal_sql(default)?));
    }
    if column
        .get("unique")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        parts.push("UNIQUE".to_owned());
    }
    if let Some(check) = check {
        parts.push(format!("CHECK ({} IN ({}))", quote(name), check));
    }
    Ok(parts.join(" "))
}

/// Tipe SQLite dan, untuk enum, daftar nilai untuk CHECK.
fn sql_type(
    table: &str,
    column: &str,
    ty: &Value,
) -> Result<(&'static str, Option<String>), SqlError> {
    if let Some(name) = ty.as_str() {
        let sql = match name {
            "text" | "timestamp" | "uuid" | "json" | "date" => "TEXT",
            "integer" | "boolean" => "INTEGER",
            "binary" => "BLOB",
            other => {
                return Err(SqlError::Unsupported(format!(
                    "{table}.{column}: tipe primitif {other:?}"
                )))
            }
        };
        return Ok((sql, None));
    }
    if ty.get("decimal").is_some() {
        return Ok(("NUMERIC", None));
    }
    if let Some(values) = ty.get("enum").and_then(Value::as_array) {
        let list = values
            .iter()
            .filter_map(Value::as_str)
            .map(sql_string)
            .collect::<Vec<_>>()
            .join(", ");
        return Ok(("TEXT", Some(list)));
    }
    Err(SqlError::Unsupported(format!(
        "{table}.{column}: bentuk tipe"
    )))
}

fn foreign_key(rel: &Value) -> Result<String, SqlError> {
    let child = rel.get("child").unwrap_or(&Value::Null);
    let parent = rel.get("parent").unwrap_or(&Value::Null);
    let action = match str_of(rel, "on_delete") {
        "cascade" => "CASCADE",
        "set_null" => "SET NULL",
        "restrict" => "NO ACTION",
        other => return Err(SqlError::Unsupported(format!("on_delete {other:?}"))),
    };
    Ok(format!(
        "FOREIGN KEY ({}) REFERENCES {} ({}) ON DELETE {}",
        quote_list(&strings(array(child, "columns"))),
        quote(str_of(parent, "table")),
        quote_list(&strings(array(parent, "columns"))),
        action
    ))
}

fn index_ddl(table: &str, index: &Value) -> String {
    let unique = if index
        .get("unique")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        "UNIQUE "
    } else {
        ""
    };
    format!(
        "CREATE {unique}INDEX {} ON {} ({});\n",
        quote(str_of(index, "name")),
        quote(table),
        quote_list(&strings(array(index, "columns")))
    )
}

/// Dua FOREIGN KEY dari tabel junction ke sisi kiri dan kanan relasi many_to_many.
fn junction_foreign_keys(rel: &Value) -> Vec<String> {
    let junction = rel.get("junction").unwrap_or(&Value::Null);
    let mut keys = Vec::new();
    for (side, jcols) in [
        (
            rel.get("left").unwrap_or(&Value::Null),
            strings(array(junction, "left_columns")),
        ),
        (
            rel.get("right").unwrap_or(&Value::Null),
            strings(array(junction, "right_columns")),
        ),
    ] {
        keys.push(format!(
            "  FOREIGN KEY ({}) REFERENCES {} ({})",
            quote_list(&jcols),
            quote(str_of(side, "table")),
            quote_list(&strings(array(side, "columns")))
        ));
    }
    keys
}

fn junction_ddl(rel: &Value, tables: &TableIndex) -> Result<String, SqlError> {
    let junction = rel.get("junction").unwrap_or(&Value::Null);
    let left = rel.get("left").unwrap_or(&Value::Null);
    let right = rel.get("right").unwrap_or(&Value::Null);
    let jname = str_of(junction, "table");
    let lcols = strings(array(junction, "left_columns"));
    let rcols = strings(array(junction, "right_columns"));
    let mut lines = Vec::new();
    for (cols, side, side_cols) in [
        (&lcols, left, strings(array(left, "columns"))),
        (&rcols, right, strings(array(right, "columns"))),
    ] {
        let side_table = str_of(side, "table");
        for (jcol, scol) in cols.iter().zip(side_cols.iter()) {
            let ty = tables.column_type(side_table, scol).ok_or_else(|| {
                SqlError::Unsupported(format!("kolom {side_table}.{scol} tidak ditemukan"))
            })?;
            let (sql, _) = sql_type(side_table, scol, ty)?;
            lines.push(format!("  {} {} NOT NULL", quote(jcol), sql));
        }
    }
    lines.push(format!(
        "  PRIMARY KEY ({})",
        quote_list(&[lcols.clone(), rcols.clone()].concat())
    ));
    lines.push(format!(
        "  FOREIGN KEY ({}) REFERENCES {} ({})",
        quote_list(&lcols),
        quote(str_of(left, "table")),
        quote_list(&strings(array(left, "columns")))
    ));
    lines.push(format!(
        "  FOREIGN KEY ({}) REFERENCES {} ({})",
        quote_list(&rcols),
        quote(str_of(right, "table")),
        quote_list(&strings(array(right, "columns")))
    ));
    Ok(format!(
        "CREATE TABLE {} (\n{}\n);\n",
        quote(jname),
        lines.join(",\n")
    ))
}

/// Kompilasi satu query menjadi SELECT berparameter.
fn compile_query(doc: &Value, query: &Value) -> Result<CompiledQuery, SqlError> {
    let name = str_of(query, "name").to_owned();
    let from = str_of(query, "from");
    let mut binder = Binder::default();
    let relations = relations_index(doc);

    let mut select = Vec::new();
    for item in array(query, "select") {
        match item.get("agg").and_then(Value::as_str) {
            Some(agg) => {
                let func = match agg {
                    "count" => "COUNT",
                    "sum" => "SUM",
                    "avg" => "AVG",
                    "min" => "MIN",
                    "max" => "MAX",
                    other => return Err(SqlError::Unsupported(format!("agregat {other:?}"))),
                };
                let col = column_ref(item);
                select.push(format!("{func}({col}) AS {}", quote(str_of(item, "alias"))));
            }
            None => {
                let col = column_ref(item);
                match item.get("alias").and_then(Value::as_str) {
                    Some(alias) => select.push(format!("{col} AS {}", quote(alias))),
                    None => select.push(col),
                }
            }
        }
    }

    let mut sql = format!("SELECT {}\nFROM {}", select.join(", "), quote(from));
    for join in array(query, "joins") {
        let rel_id = str_of(join, "relation");
        let rel = relations
            .get(rel_id)
            .ok_or_else(|| SqlError::Unsupported(format!("relasi {rel_id:?} tidak ditemukan")))?;
        let keyword = if str_of(join, "type") == "left" {
            "LEFT JOIN"
        } else {
            "JOIN"
        };
        sql.push_str(&join_sql(keyword, from, rel)?);
    }

    if let Some(where_clause) = query.get("where") {
        sql.push_str(&format!(
            "\nWHERE {}",
            predicate(where_clause, &mut binder)?
        ));
    }
    let group = array(query, "group_by");
    if !group.is_empty() {
        let cols: Vec<String> = group.iter().map(column_ref).collect();
        sql.push_str(&format!("\nGROUP BY {}", cols.join(", ")));
    }
    let order = array(query, "order_by");
    if !order.is_empty() {
        let items: Vec<String> = order
            .iter()
            .map(|o| {
                let dir = if str_of(o, "dir") == "desc" {
                    "DESC"
                } else {
                    "ASC"
                };
                format!("{} {dir}", quote(str_of(o, "alias")))
            })
            .collect();
        sql.push_str(&format!("\nORDER BY {}", items.join(", ")));
    }
    if let Some(limit) = query.get("limit").and_then(Value::as_u64) {
        sql.push_str(&format!("\nLIMIT {limit}"));
    }
    sql.push(';');

    let mut params: Vec<String> = query
        .get("params")
        .and_then(Value::as_object)
        .map(|o| o.keys().cloned().collect())
        .unwrap_or_default();
    params.sort();
    Ok(CompiledQuery {
        name,
        sql,
        params,
        literals: binder.literals,
    })
}

fn join_sql(keyword: &str, from: &str, rel: &Value) -> Result<String, SqlError> {
    if kind(rel) == "many_to_many" {
        let junction = rel.get("junction").unwrap_or(&Value::Null);
        let left = rel.get("left").unwrap_or(&Value::Null);
        let right = rel.get("right").unwrap_or(&Value::Null);
        let jname = str_of(junction, "table");
        if str_of(left, "table") == from {
            return Ok(format!(
                "\n{keyword} {} ON {}\n{keyword} {} ON {}",
                quote(jname),
                on_pairs(
                    left,
                    jname,
                    &strings(array(left, "columns")),
                    &strings(array(junction, "left_columns"))
                ),
                quote(str_of(right, "table")),
                on_pairs(
                    right,
                    jname,
                    &strings(array(right, "columns")),
                    &strings(array(junction, "right_columns"))
                ),
            ));
        }
        return Ok(format!(
            "\n{keyword} {} ON {}\n{keyword} {} ON {}",
            quote(jname),
            on_pairs(
                right,
                jname,
                &strings(array(right, "columns")),
                &strings(array(junction, "right_columns"))
            ),
            quote(str_of(left, "table")),
            on_pairs(
                left,
                jname,
                &strings(array(left, "columns")),
                &strings(array(junction, "left_columns"))
            ),
        ));
    }
    let parent = rel.get("parent").unwrap_or(&Value::Null);
    let child = rel.get("child").unwrap_or(&Value::Null);
    // Tabel yang di-join adalah sisi yang bukan `from`.
    let target = if str_of(parent, "table") == from {
        child
    } else {
        parent
    };
    // Kondisi ON selalu ditulis induk = anak, apa pun arah join-nya.
    let target_table = str_of(target, "table");
    let parent_cols = strings(array(parent, "columns"));
    let child_cols = strings(array(child, "columns"));
    let pairs: Vec<String> = parent_cols
        .iter()
        .zip(child_cols.iter())
        .map(|(p, c)| {
            format!(
                "{}.{} = {}.{}",
                quote(str_of(parent, "table")),
                quote(p),
                quote(str_of(child, "table")),
                quote(c)
            )
        })
        .collect();
    Ok(format!(
        "\n{keyword} {} ON {}",
        quote(target_table),
        pairs.join(" AND ")
    ))
}

fn on_pairs(
    side: &Value,
    junction: &str,
    side_cols: &[String],
    junction_cols: &[String],
) -> String {
    side_cols
        .iter()
        .zip(junction_cols.iter())
        .map(|(s, j)| {
            format!(
                "{}.{} = {}.{}",
                quote(str_of(side, "table")),
                quote(s),
                quote(junction),
                quote(j)
            )
        })
        .collect::<Vec<_>>()
        .join(" AND ")
}

/// Mengikat operand dan predikat menjadi SQL. Literal diberi nama `litN` dan tidak pernah disisipkan.
#[derive(Default)]
struct Binder {
    literals: Vec<(String, Value)>,
}

impl Binder {
    fn literal(&mut self, value: &Value) -> String {
        let name = format!("lit{}", self.literals.len() + 1);
        self.literals.push((name.clone(), value.clone()));
        format!(":{name}")
    }
}

fn predicate(node: &Value, binder: &mut Binder) -> Result<String, SqlError> {
    if let Some(items) = node.get("and").and_then(Value::as_array) {
        let parts: Result<Vec<String>, SqlError> =
            items.iter().map(|i| predicate(i, binder)).collect();
        return Ok(format!("({})", parts?.join(" AND ")));
    }
    if let Some(items) = node.get("or").and_then(Value::as_array) {
        let parts: Result<Vec<String>, SqlError> =
            items.iter().map(|i| predicate(i, binder)).collect();
        return Ok(format!("({})", parts?.join(" OR ")));
    }
    if let Some(inner) = node.get("not") {
        return Ok(format!("NOT ({})", predicate(inner, binder)?));
    }
    let Some(cmp) = node.get("cmp") else {
        return Err(SqlError::Unsupported(
            "predikat tanpa and, or, not, atau cmp".to_owned(),
        ));
    };
    let left = operand(cmp.get("left").unwrap_or(&Value::Null), binder)?;
    let op = str_of(cmp, "op");
    match op {
        "is_null" => return Ok(format!("{left} IS NULL")),
        "is_not_null" => return Ok(format!("{left} IS NOT NULL")),
        _ => {}
    }
    let right_node = cmp.get("right").unwrap_or(&Value::Null);
    if op == "in" {
        // Daftar literal diikat satu per satu. Parameter daftar diterima sebagai teks JSON
        // dan dibuka dengan json_each (SQLite tidak bisa mengikat array sebagai satu nilai).
        if let Some(items) = right_node.as_array() {
            let mut parts = Vec::new();
            for item in items {
                parts.push(operand(item, binder)?);
            }
            return Ok(format!("{left} IN ({})", parts.join(", ")));
        }
        let list = operand(right_node, binder)?;
        return Ok(format!("{left} IN (SELECT value FROM json_each({list}))"));
    }
    let symbol = match op {
        "eq" => "=",
        "ne" => "<>",
        "lt" => "<",
        "lte" => "<=",
        "gt" => ">",
        "gte" => ">=",
        "like" => "LIKE",
        other => return Err(SqlError::Unsupported(format!("operator {other:?}"))),
    };
    let right = operand(right_node, binder)?;
    Ok(format!("{left} {symbol} {right}"))
}

fn operand(node: &Value, binder: &mut Binder) -> Result<String, SqlError> {
    if node.get("table").is_some() && node.get("column").is_some() {
        return Ok(column_ref(node));
    }
    if let Some(param) = node.get("param").and_then(Value::as_str) {
        return Ok(format!(":{param}"));
    }
    if node.is_null() || node.is_boolean() || node.is_number() || node.is_string() {
        return Ok(binder.literal(node));
    }
    Err(SqlError::Unsupported(format!("operand {node}")))
}

fn column_ref(node: &Value) -> String {
    format!(
        "{}.{}",
        quote(str_of(node, "table")),
        quote(str_of(node, "column"))
    )
}

/// Indeks tabel dan kolomnya, untuk menemukan tipe kolom junction.
struct TableIndex<'a> {
    tables: Vec<(&'a str, &'a Value)>,
}

impl<'a> TableIndex<'a> {
    fn contains(&self, table: &str) -> bool {
        self.tables.iter().any(|(n, _)| *n == table)
    }

    fn column_type(&self, table: &str, column: &str) -> Option<&'a Value> {
        let entry = self.tables.iter().find(|(n, _)| *n == table)?;
        let table_value: &'a Value = entry.1;
        array(table_value, "columns")
            .iter()
            .find(|c| str_of(c, "name") == column)
            .and_then(|c| c.get("type"))
    }
}

fn tables_index(doc: &Value) -> TableIndex<'_> {
    TableIndex {
        tables: array(doc, "tables")
            .iter()
            .map(|t| (str_of(t, "name"), t))
            .collect(),
    }
}

fn relations_index(doc: &Value) -> std::collections::BTreeMap<String, Value> {
    array(doc, "relations")
        .iter()
        .map(|r| (str_of(r, "id").to_owned(), r.clone()))
        .collect()
}

fn kind(rel: &Value) -> &str {
    str_of(rel, "kind")
}

fn array<'a>(value: &'a Value, key: &str) -> &'a [Value] {
    value
        .get(key)
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[])
}

fn str_of<'a>(value: &'a Value, key: &str) -> &'a str {
    value.get(key).and_then(Value::as_str).unwrap_or("")
}

fn strings(values: &[Value]) -> Vec<String> {
    values
        .iter()
        .filter_map(Value::as_str)
        .map(str::to_owned)
        .collect()
}

fn quote(identifier: &str) -> String {
    format!("\"{}\"", identifier.replace('"', "\"\""))
}

fn quote_list(identifiers: &[String]) -> String {
    identifiers
        .iter()
        .map(|i| quote(i))
        .collect::<Vec<_>>()
        .join(", ")
}

fn sql_string(text: &str) -> String {
    format!("'{}'", text.replace('\'', "''"))
}

/// Nilai default sebagai literal SQL. Boolean menjadi 1 atau 0 karena tipe boolean adalah INTEGER.
fn literal_sql(value: &Value) -> Result<String, SqlError> {
    match value {
        Value::Null => Ok("NULL".to_owned()),
        Value::Bool(b) => Ok(if *b { "1".to_owned() } else { "0".to_owned() }),
        Value::Number(n) => Ok(n.to_string()),
        Value::String(s) => Ok(sql_string(s)),
        other => Err(SqlError::Unsupported(format!("nilai default {other}"))),
    }
}

#[cfg(test)]
#[path = "codegen_sql_test.rs"]
mod codegen_sql_test;
