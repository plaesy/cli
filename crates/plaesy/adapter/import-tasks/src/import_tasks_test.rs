use super::*;

fn task(title: &str, status: TaskStatus, labels: &[&str]) -> TaskRecord {
    TaskRecord {
        title: title.to_owned(),
        status,
        labels: labels.iter().map(|l| (*l).to_owned()).collect(),
    }
}

#[test]
fn json_valid_rows_are_imported_with_defaults() {
    let report = import_json(
        r#"[
            {"title": "  Siapkan rilis  ", "status": "DOING", "labels": ["rilis", " urgent ", "rilis"]},
            {"title": "Tulis catatan"},
            {"title": "Tinjau", "status": null, "labels": null, "extra": 1}
        ]"#,
    )
    .expect("array json is accepted");

    assert!(report.rejected.is_empty());
    assert_eq!(report.total_rows(), 3);
    assert_eq!(
        report.tasks,
        vec![
            task("Siapkan rilis", TaskStatus::Doing, &["rilis", "urgent"]),
            task("Tulis catatan", TaskStatus::Backlog, &[]),
            task("Tinjau", TaskStatus::Backlog, &[]),
        ]
    );
}

#[test]
fn json_broken_rows_are_reported_by_position_and_valid_rows_still_import() {
    let report = import_json(
        r#"[
            {"title": "Baik", "status": "done"},
            42,
            {"title": "   "},
            {"title": "Status salah", "status": "selesai"},
            {"title": 7},
            {"title": "Label salah", "labels": ["a,b"]},
            {"title": "Label tipe", "labels": "a"},
            {"title": "Baik lagi", "status": "blocked", "labels": ["x"]}
        ]"#,
    )
    .expect("array json is accepted");

    assert_eq!(
        report.tasks,
        vec![
            task("Baik", TaskStatus::Done, &[]),
            task("Baik lagi", TaskStatus::Blocked, &["x"]),
        ]
    );
    assert_eq!(
        report.rejected,
        vec![
            RowError {
                row: 2,
                kind: RowErrorKind::NotAnObject
            },
            RowError {
                row: 3,
                kind: RowErrorKind::EmptyTitle
            },
            RowError {
                row: 4,
                kind: RowErrorKind::InvalidStatus("selesai".to_owned())
            },
            RowError {
                row: 5,
                kind: RowErrorKind::WrongType {
                    field: "title",
                    expected: "teks"
                }
            },
            RowError {
                row: 6,
                kind: RowErrorKind::InvalidLabel("a,b".to_owned())
            },
            RowError {
                row: 7,
                kind: RowErrorKind::WrongType {
                    field: "labels",
                    expected: "array teks"
                }
            },
        ]
    );
}

#[test]
fn json_missing_title_is_a_row_error_not_a_file_error() {
    let report =
        import_json(r#"[{"status": "todo"}, {"title": "Ada"}]"#).expect("array json is accepted");

    assert_eq!(report.tasks, vec![task("Ada", TaskStatus::Backlog, &[])]);
    assert_eq!(
        report.rejected,
        vec![RowError {
            row: 1,
            kind: RowErrorKind::MissingField("title"),
        }]
    );
}

#[test]
fn json_file_level_errors_fail_the_whole_import() {
    assert!(matches!(
        import_json(r#"{"title": "bukan array"}"#),
        Err(ImportError::NotAnArray)
    ));
    assert!(matches!(
        import_json("[{\"title\": "),
        Err(ImportError::InvalidJson(_))
    ));
}

#[test]
fn csv_valid_rows_handle_quotes_crlf_labels_and_blank_lines() {
    let text = "\u{feff}Title,Status,Labels\r\n\
                \"Rapat, tinjauan\",todo,\"bug; ui\"\r\n\
                \r\n\
                \"Kutip \"\"ganda\"\"\",,\r\n\
                Tanpa label,doing,\r\n";
    let report = import_csv(text).expect("header has title");

    assert!(report.rejected.is_empty());
    assert_eq!(
        report.tasks,
        vec![
            task("Rapat, tinjauan", TaskStatus::Todo, &["bug", "ui"]),
            task("Kutip \"ganda\"", TaskStatus::Backlog, &[]),
            task("Tanpa label", TaskStatus::Doing, &[]),
        ]
    );
}

#[test]
fn csv_broken_rows_are_reported_with_physical_line_numbers() {
    let text = "title,status\n\
                \"satu\ndua\",todo\n\
                ,doing\n\
                Ada,selesai\n\
                Kurang\n\
                Lebih,done,x,y\n\
                Baik,review\n";
    let report = import_csv(text).expect("header has title");

    assert_eq!(report.tasks, vec![task("Baik", TaskStatus::Review, &[])]);
    assert_eq!(
        report.rejected,
        vec![
            RowError {
                row: 2,
                kind: RowErrorKind::InvalidTitle,
            },
            RowError {
                row: 4,
                kind: RowErrorKind::EmptyTitle,
            },
            RowError {
                row: 5,
                kind: RowErrorKind::InvalidStatus("selesai".to_owned()),
            },
            RowError {
                row: 6,
                kind: RowErrorKind::FieldCount {
                    expected: 2,
                    found: 1,
                },
            },
            RowError {
                row: 7,
                kind: RowErrorKind::FieldCount {
                    expected: 2,
                    found: 4,
                },
            },
        ]
    );
}

#[test]
fn csv_missing_title_column_is_a_file_error() {
    assert_eq!(
        import_csv("status,labels\ntodo,x\n"),
        Err(ImportError::MissingColumn { column: "title" })
    );
    assert_eq!(
        import_csv(""),
        Err(ImportError::MissingColumn { column: "title" })
    );
}

#[test]
fn csv_missing_optional_columns_fall_back_to_defaults() {
    let report = import_csv("title\nSatu\nDua\n").expect("header has title");

    assert!(report.rejected.is_empty());
    assert_eq!(
        report.tasks,
        vec![
            task("Satu", TaskStatus::Backlog, &[]),
            task("Dua", TaskStatus::Backlog, &[]),
        ]
    );
}

#[test]
fn csv_duplicate_and_unterminated_input_are_file_errors() {
    assert_eq!(
        import_csv("title,Title\na,b\n"),
        Err(ImportError::DuplicateColumn("title".to_owned()))
    );
    assert_eq!(
        import_csv("title\n\"tidak ditutup\n"),
        Err(ImportError::UnterminatedQuote { line: 2 })
    );
}

#[test]
fn rejected_row_display_names_the_row_and_reason() {
    let error = RowError {
        row: 4,
        kind: RowErrorKind::EmptyTitle,
    };
    assert_eq!(error.to_string(), "baris 4: judul kosong");
}
