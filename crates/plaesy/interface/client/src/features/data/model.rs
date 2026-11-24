//! Model view data: tabel, kolom, dan relasi.

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ColumnView {
    pub name: String,
    pub data_type: String,
    pub nullable: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableView {
    pub name: String,
    pub columns: Vec<ColumnView>,
    pub primary_key: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelationView {
    pub id: String,
    pub parent: (String, Vec<String>),
    pub child: (String, Vec<String>),
}

/// Masalah validasi ringan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LocalIssue {
    DuplicateTable(String),
    DuplicateColumn { table: String, column: String },
    UnknownPrimaryKey { table: String, column: String },
    UnknownRelationTable(String),
}

/// Validasi ringan atas daftar tabel dan relasi.
#[must_use]
pub fn validate(tables: &[TableView], relations: &[RelationView]) -> Vec<LocalIssue> {
    let mut issues = Vec::new();
    let mut names = std::collections::BTreeSet::new();
    for table in tables {
        if !names.insert(table.name.as_str()) {
            issues.push(LocalIssue::DuplicateTable(table.name.clone()));
        }
        let mut columns = std::collections::BTreeSet::new();
        for column in &table.columns {
            if !columns.insert(column.name.as_str()) {
                issues.push(LocalIssue::DuplicateColumn {
                    table: table.name.clone(),
                    column: column.name.clone(),
                });
            }
        }
        for key in &table.primary_key {
            if !table.columns.iter().any(|c| &c.name == key) {
                issues.push(LocalIssue::UnknownPrimaryKey {
                    table: table.name.clone(),
                    column: key.clone(),
                });
            }
        }
    }
    for relation in relations {
        for table in [&relation.parent.0, &relation.child.0] {
            if !names.contains(table.as_str()) {
                issues.push(LocalIssue::UnknownRelationTable(table.clone()));
            }
        }
    }
    issues
}

#[cfg(test)]
mod tests {
    use super::*;

    fn column(name: &str) -> ColumnView {
        ColumnView {
            name: name.to_owned(),
            data_type: "text".to_owned(),
            nullable: false,
        }
    }

    #[test]
    fn validate_reports_duplicates_and_bad_keys() {
        let tables = vec![
            TableView {
                name: "a".into(),
                columns: vec![column("id"), column("id")],
                primary_key: vec!["nope".into()],
            },
            TableView {
                name: "a".into(),
                columns: vec![],
                primary_key: vec![],
            },
        ];
        let issues = validate(&tables, &[]);
        assert!(issues.contains(&LocalIssue::DuplicateTable("a".into())));
        assert!(issues.contains(&LocalIssue::DuplicateColumn {
            table: "a".into(),
            column: "id".into()
        }));
        assert!(issues.contains(&LocalIssue::UnknownPrimaryKey {
            table: "a".into(),
            column: "nope".into()
        }));
    }

    #[test]
    fn validate_flags_relation_to_missing_table() {
        let tables = vec![TableView {
            name: "orders".into(),
            columns: vec![column("id")],
            primary_key: vec!["id".into()],
        }];
        let relations = vec![RelationView {
            id: "rel_1".into(),
            parent: ("customers".into(), vec!["id".into()]),
            child: ("orders".into(), vec!["customer_id".into()]),
        }];
        assert_eq!(
            validate(&tables, &relations),
            vec![LocalIssue::UnknownRelationTable("customers".into())]
        );
    }
}
