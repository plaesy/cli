//! Reducer editor data: tambah tabel, kolom, dan relasi secara murni.

use super::model::{ColumnView, RelationView, TableView};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    AddTable(TableView),
    AddColumn { table: String, column: ColumnView },
    AddRelation(RelationView),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActionError {
    NotFound(String),
    AlreadyExists(String),
}

/// Menerapkan `action` dan mengembalikan keadaan baru (tabel, relasi).
///
/// # Errors
///
/// `NotFound` bila tabel tidak ada, `AlreadyExists` bila tabel, kolom, atau relasi sudah ada.
pub fn apply(
    tables: &[TableView],
    relations: &[RelationView],
    action: &Action,
) -> Result<(Vec<TableView>, Vec<RelationView>), ActionError> {
    let mut next_tables = tables.to_vec();
    let mut next_relations = relations.to_vec();
    match action {
        Action::AddTable(table) => {
            if next_tables.iter().any(|t| t.name == table.name) {
                return Err(ActionError::AlreadyExists(table.name.clone()));
            }
            next_tables.push(table.clone());
        }
        Action::AddColumn { table, column } => {
            let target = next_tables
                .iter_mut()
                .find(|t| &t.name == table)
                .ok_or_else(|| ActionError::NotFound(table.clone()))?;
            if target.columns.iter().any(|c| c.name == column.name) {
                return Err(ActionError::AlreadyExists(column.name.clone()));
            }
            target.columns.push(column.clone());
        }
        Action::AddRelation(relation) => {
            if next_relations.iter().any(|r| r.id == relation.id) {
                return Err(ActionError::AlreadyExists(relation.id.clone()));
            }
            next_relations.push(relation.clone());
        }
    }
    Ok((next_tables, next_relations))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn column(name: &str) -> ColumnView {
        ColumnView {
            name: name.into(),
            data_type: "text".into(),
            nullable: false,
        }
    }

    fn table(name: &str) -> TableView {
        TableView {
            name: name.into(),
            columns: vec![],
            primary_key: vec![],
        }
    }

    #[test]
    fn add_column_to_missing_table_is_not_found() {
        assert_eq!(
            apply(
                &[],
                &[],
                &Action::AddColumn {
                    table: "x".into(),
                    column: column("id")
                }
            ),
            Err(ActionError::NotFound("x".into()))
        );
    }

    #[test]
    fn add_duplicate_table_or_column_is_refused() {
        let (tables, _) = apply(&[], &[], &Action::AddTable(table("a"))).unwrap();
        assert_eq!(
            apply(&tables, &[], &Action::AddTable(table("a"))),
            Err(ActionError::AlreadyExists("a".into()))
        );
        let (tables, _) = apply(
            &tables,
            &[],
            &Action::AddColumn {
                table: "a".into(),
                column: column("id"),
            },
        )
        .unwrap();
        assert_eq!(
            apply(
                &tables,
                &[],
                &Action::AddColumn {
                    table: "a".into(),
                    column: column("id")
                }
            ),
            Err(ActionError::AlreadyExists("id".into()))
        );
    }

    #[test]
    fn add_relation_appends_and_refuses_same_id() {
        let relation = RelationView {
            id: "rel_1".into(),
            parent: ("a".into(), vec!["id".into()]),
            child: ("b".into(), vec!["a_id".into()]),
        };
        let (_, relations) = apply(&[], &[], &Action::AddRelation(relation.clone())).unwrap();
        assert_eq!(relations.len(), 1);
        assert_eq!(
            apply(&[], &relations, &Action::AddRelation(relation)),
            Err(ActionError::AlreadyExists("rel_1".into()))
        );
    }
}
