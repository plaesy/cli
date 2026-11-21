//! Semantic rules for `data-ir/1` (ADR 0004, "Validasi semantik"). Port of `validateData`.

use std::collections::{HashMap, HashSet};

use serde_json::Value;

use plaesy_kernel::json::{array, str_of, strings};

/// Column name -> column type (serialised, so types compare by value).
type Columns = HashMap<String, String>;

pub fn validate(doc: &Value) -> Vec<String> {
    let mut errors = Vec::new();

    let mut tables: HashMap<String, Columns> = HashMap::new();
    for table in array(doc, "tables") {
        let name = str_of(table, "name").unwrap_or("?").to_string();
        if tables.contains_key(&name) {
            errors.push(format!("tabel duplikat: {name}"));
        }
        let mut cols: Columns = HashMap::new();
        for column in array(table, "columns") {
            let col_name = str_of(column, "name").unwrap_or("?").to_string();
            let col_type = column.get("type").map(Value::to_string).unwrap_or_default();
            cols.insert(col_name, col_type);
        }
        if cols.len() != array(table, "columns").len() {
            errors.push(format!("tabel {name}: kolom duplikat"));
        }
        for pk in strings(array(table, "primary_key")) {
            if !cols.contains_key(pk) {
                errors.push(format!(
                    "tabel {name}: primary_key merujuk kolom tidak ada: {pk}"
                ));
            }
        }
        tables.insert(name, cols);
    }

    let mut rel_ids: HashSet<&str> = HashSet::new();
    let mut relations: HashMap<&str, &Value> = HashMap::new();
    for rel in array(doc, "relations") {
        let id = str_of(rel, "id").unwrap_or("?");
        if !rel_ids.insert(id) {
            errors.push(format!("relasi duplikat: {id}"));
        }
        relations.insert(id, rel);
        check_relation(&mut errors, &tables, rel, id);
    }

    for query in array(doc, "queries") {
        check_query(&mut errors, &tables, &relations, query);
    }
    errors
}

fn col_type(tables: &HashMap<String, Columns>, table: &str, column: &str) -> Option<String> {
    tables.get(table).and_then(|cols| cols.get(column)).cloned()
}

fn check_relation(
    errors: &mut Vec<String>,
    tables: &HashMap<String, Columns>,
    rel: &Value,
    id: &str,
) {
    let kind = str_of(rel, "kind").unwrap_or("");
    let ends: Vec<(&Value, Option<&Value>)> = if kind == "many_to_many" {
        let junction = rel.get("junction").unwrap_or(&Value::Null);
        if !tables.contains_key(str_of(junction, "table").unwrap_or("")) {
            errors.push(format!(
                "{id}: junction {} tidak ada",
                str_of(junction, "table").unwrap_or("")
            ));
        }
        vec![
            (rel.get("left").unwrap_or(&Value::Null), Some(junction)),
            (rel.get("right").unwrap_or(&Value::Null), Some(junction)),
        ]
    } else {
        vec![
            (rel.get("parent").unwrap_or(&Value::Null), None),
            (rel.get("child").unwrap_or(&Value::Null), None),
        ]
    };

    if kind == "many_to_many" {
        let junction = rel.get("junction").unwrap_or(&Value::Null);
        let jcols = [
            strings(array(junction, "left_columns")),
            strings(array(junction, "right_columns")),
        ];
        for (i, (end, _)) in ends.iter().enumerate() {
            let table = str_of(end, "table").unwrap_or("");
            let columns = strings(array(end, "columns"));
            let jtable = str_of(junction, "table").unwrap_or("");
            if !tables.contains_key(table) {
                errors.push(format!("{id}: tabel {table} tidak ada"));
            }
            let jc = &jcols[i];
            if columns.len() != jc.len() {
                errors.push(format!("{id}: jumlah kolom junction tidak sama"));
            }
            for (k, c) in columns.iter().enumerate() {
                let a = col_type(tables, table, c);
                let b = jc.get(k).and_then(|jcol| col_type(tables, jtable, jcol));
                if a.is_none() {
                    errors.push(format!("{id}: kolom {table}.{c} tidak ada"));
                }
                if b.is_none() {
                    errors.push(format!(
                        "{id}: kolom junction {jtable}.{} tidak ada",
                        jc.get(k).copied().unwrap_or("?")
                    ));
                }
                if let (Some(a), Some(b)) = (&a, &b) {
                    if a != b {
                        errors.push(format!(
                            "{id}: tipe {table}.{c} tidak cocok dengan junction"
                        ));
                    }
                }
            }
        }
    } else {
        let parent = ends[0].0;
        let child = ends[1].0;
        let (pt, ct) = (
            str_of(parent, "table").unwrap_or(""),
            str_of(child, "table").unwrap_or(""),
        );
        if !tables.contains_key(pt) {
            errors.push(format!("{id}: parent {pt} tidak ada"));
        }
        if !tables.contains_key(ct) {
            errors.push(format!("{id}: child {ct} tidak ada"));
        }
        let pcols = strings(array(parent, "columns"));
        let ccols = strings(array(child, "columns"));
        if pcols.len() != ccols.len() {
            errors.push(format!("{id}: jumlah kolom parent dan child tidak sama"));
        }
        for (k, pc) in pcols.iter().enumerate() {
            let a = col_type(tables, pt, pc);
            let b = ccols.get(k).and_then(|c| col_type(tables, ct, c));
            if a.is_none() {
                errors.push(format!("{id}: kolom parent {pt}.{pc} tidak ada"));
            }
            if b.is_none() {
                errors.push(format!(
                    "{id}: kolom child {ct}.{} tidak ada",
                    ccols.get(k).copied().unwrap_or("?")
                ));
            }
            if let (Some(a), Some(b)) = (&a, &b) {
                if a != b {
                    errors.push(format!("{id}: tipe {pt}.{pc} tidak cocok dengan {ct}"));
                }
            }
        }
    }
}

fn check_query(
    errors: &mut Vec<String>,
    tables: &HashMap<String, Columns>,
    relations: &HashMap<&str, &Value>,
    query: &Value,
) {
    let id = str_of(query, "id").unwrap_or("?");
    let here = format!("query {id}");
    let from = str_of(query, "from").unwrap_or("");
    if !tables.contains_key(from) {
        errors.push(format!("{here}: from {from} tidak ada"));
    }
    let mut known: HashSet<String> = HashSet::from([from.to_string()]);
    for join in array(query, "joins") {
        let rel_id = str_of(join, "relation").unwrap_or("");
        let Some(rel) = relations.get(rel_id) else {
            errors.push(format!("{here}: join merujuk relasi tidak ada: {rel_id}"));
            continue;
        };
        let touch: Vec<String> = if str_of(rel, "kind") == Some("many_to_many") {
            vec![
                str_of(rel.get("left").unwrap_or(&Value::Null), "table")
                    .unwrap_or("")
                    .to_string(),
                str_of(rel.get("right").unwrap_or(&Value::Null), "table")
                    .unwrap_or("")
                    .to_string(),
            ]
        } else {
            vec![
                str_of(rel.get("parent").unwrap_or(&Value::Null), "table")
                    .unwrap_or("")
                    .to_string(),
                str_of(rel.get("child").unwrap_or(&Value::Null), "table")
                    .unwrap_or("")
                    .to_string(),
            ]
        };
        if !touch.iter().any(|t| known.contains(t)) {
            errors.push(format!(
                "{here}: join {rel_id} tidak terhubung ke tabel yang sudah ada"
            ));
        }
        known.extend(touch);
    }

    let select = array(query, "select");
    let mut aliases: HashSet<&str> = HashSet::new();
    for item in select {
        if let Some(alias) = str_of(item, "alias") {
            if !aliases.insert(alias) {
                errors.push(format!("{here}: alias duplikat {alias}"));
            }
        }
        if let Some(table) = str_of(item, "table") {
            if !known.contains(table) {
                errors.push(format!(
                    "{here}: select memakai tabel di luar from/join: {table}"
                ));
            }
            if let (Some(cols), Some(column)) = (tables.get(table), str_of(item, "column")) {
                if !cols.contains_key(column) {
                    errors.push(format!("{here}: kolom {table}.{column} tidak ada"));
                }
            }
        }
    }

    let has_agg = select.iter().any(|s| s.get("agg").is_some());
    if has_agg {
        let grouped: HashSet<String> = array(query, "group_by")
            .iter()
            .map(|g| {
                format!(
                    "{}.{}",
                    str_of(g, "table").unwrap_or(""),
                    str_of(g, "column").unwrap_or("")
                )
            })
            .collect();
        for item in select {
            if item.get("agg").is_none() {
                if let Some(table) = str_of(item, "table") {
                    let key = format!("{table}.{}", str_of(item, "column").unwrap_or(""));
                    if !grouped.contains(&key) {
                        errors.push(format!("{here}: {key} harus ada di group_by"));
                    }
                }
            }
        }
    }

    for order in array(query, "order_by") {
        let alias = str_of(order, "alias").unwrap_or("");
        if !aliases.contains(alias) {
            errors.push(format!("{here}: order_by merujuk alias tidak ada: {alias}"));
        }
    }

    let params: HashSet<&str> = query
        .get("params")
        .and_then(Value::as_object)
        .map(|o| o.keys().map(String::as_str).collect())
        .unwrap_or_default();
    if let Some(filter) = query.get("where") {
        walk_params(errors, &here, &params, filter);
    }
}

fn walk_params(errors: &mut Vec<String>, here: &str, params: &HashSet<&str>, node: &Value) {
    match node {
        Value::Object(object) => {
            if let Some(name) = object.get("param").and_then(Value::as_str) {
                if !params.contains(name) {
                    errors.push(format!("{here}: parameter tidak dideklarasikan: {name}"));
                }
            }
            for v in object.values() {
                walk_params(errors, here, params, v);
            }
        }
        Value::Array(items) => {
            for v in items {
                walk_params(errors, here, params, v);
            }
        }
        _ => {}
    }
}
