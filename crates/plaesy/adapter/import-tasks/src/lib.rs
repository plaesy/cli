//! Impor lokal tugas dari berkas JSON dan CSV (kemampuan #17, impor berkas).
//!
//! Crate ini hanya mengurai teks yang sudah dibaca pemanggil. Tidak ada jaringan dan
//! tidak ada penulisan ke `.plaesy/tasks`. Baris yang tidak valid dilaporkan dengan
//! nomornya dan tidak menggagalkan baris lain. Galat berkas (JSON bukan array, header
//! CSV tanpa `title`, tanda kutip tidak tertutup) menggagalkan seluruh impor.
//!
//! Nilai status memakai nama folder tugas (`backlog`, `todo`, `doing`, `review`, `done`,
//! `blocked`), sama dengan `plaesy-tasks`. Daftar ini disalin agar crate ini tidak
//! menarik adapter memori.
//!
//! Kolom: `title` wajib. `status` kosong atau tidak ada menjadi `backlog`. `labels`
//! kosong atau tidak ada menjadi daftar kosong. Pada JSON, `labels` berupa array teks.
//! Pada CSV, `labels` dipisah dengan `;` karena `,` adalah pemisah kolom.

use serde_json::{Map, Value};
use std::fmt;

/// Status tugas. Nama varian sama dengan nama folder di `.plaesy/tasks/`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskStatus {
    Backlog,
    Todo,
    Doing,
    Review,
    Done,
    Blocked,
}

impl TaskStatus {
    /// Semua status, dalam urutan siklus hidup.
    pub const ALL: [TaskStatus; 6] = [
        TaskStatus::Backlog,
        TaskStatus::Todo,
        TaskStatus::Doing,
        TaskStatus::Review,
        TaskStatus::Done,
        TaskStatus::Blocked,
    ];

    /// Nama folder untuk status ini.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Backlog => "backlog",
            Self::Todo => "todo",
            Self::Doing => "doing",
            Self::Review => "review",
            Self::Done => "done",
            Self::Blocked => "blocked",
        }
    }

    /// Mencocokkan teks tanpa membedakan huruf besar-kecil dan spasi di tepi.
    fn parse(raw: &str) -> Option<Self> {
        let lowered = raw.trim().to_ascii_lowercase();
        Self::ALL.into_iter().find(|s| s.as_str() == lowered)
    }
}

/// Satu tugas yang lolos validasi.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskRecord {
    pub title: String,
    pub status: TaskStatus,
    /// Label tanpa duplikat, urutan sama dengan berkas.
    pub labels: Vec<String>,
}

/// Sebab sebuah baris ditolak.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RowErrorKind {
    /// Elemen array JSON bukan objek.
    NotAnObject,
    /// Field wajib tidak ada (JSON).
    MissingField(&'static str),
    /// Field ada tetapi tipenya salah (JSON).
    WrongType {
        field: &'static str,
        expected: &'static str,
    },
    /// Judul kosong setelah dipangkas.
    EmptyTitle,
    /// Judul mengandung karakter kontrol, termasuk baris baru.
    InvalidTitle,
    /// Status tidak termasuk dalam [`TaskStatus::ALL`].
    InvalidStatus(String),
    /// Label kosong, atau mengandung `,`, `;`, atau karakter kontrol.
    InvalidLabel(String),
    /// Jumlah field CSV tidak sama dengan jumlah kolom header.
    FieldCount { expected: usize, found: usize },
}

impl fmt::Display for RowErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotAnObject => write!(f, "elemen bukan objek"),
            Self::MissingField(field) => write!(f, "field {field:?} tidak ada"),
            Self::WrongType { field, expected } => {
                write!(f, "field {field:?} harus {expected}")
            }
            Self::EmptyTitle => write!(f, "judul kosong"),
            Self::InvalidTitle => write!(f, "judul mengandung karakter kontrol"),
            Self::InvalidStatus(status) => write!(
                f,
                "status {status:?} tidak dikenal (backlog, todo, doing, review, done, blocked)"
            ),
            Self::InvalidLabel(label) => write!(
                f,
                "label {label:?} kosong atau mengandung ',', ';', atau karakter kontrol"
            ),
            Self::FieldCount { expected, found } => {
                write!(f, "jumlah field {found}, header punya {expected} kolom")
            }
        }
    }
}

/// Baris yang ditolak beserta nomornya.
///
/// Untuk JSON, `row` adalah posisi elemen dalam array (mulai 1). Untuk CSV, `row` adalah
/// nomor baris fisik tempat record dimulai (header di baris 1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RowError {
    pub row: usize,
    pub kind: RowErrorKind,
}

impl fmt::Display for RowError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "baris {}: {}", self.row, self.kind)
    }
}

/// Hasil impor: tugas yang valid dan baris yang gagal, keduanya dalam urutan berkas.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ImportReport {
    pub tasks: Vec<TaskRecord>,
    pub rejected: Vec<RowError>,
}

impl ImportReport {
    /// Jumlah baris data yang diperiksa, valid maupun gagal.
    #[must_use]
    pub fn total_rows(&self) -> usize {
        self.tasks.len() + self.rejected.len()
    }
}

/// Galat yang menggagalkan seluruh berkas.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImportError {
    /// Teks bukan JSON yang valid.
    InvalidJson(String),
    /// Akar JSON bukan array.
    NotAnArray,
    /// Kolom wajib tidak ada di header CSV.
    MissingColumn { column: &'static str },
    /// Nama kolom muncul lebih dari sekali di header CSV.
    DuplicateColumn(String),
    /// Tanda kutip tidak tertutup sampai akhir berkas. `line` adalah awal record itu.
    UnterminatedQuote { line: usize },
}

impl fmt::Display for ImportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidJson(message) => write!(f, "JSON tidak valid: {message}"),
            Self::NotAnArray => write!(f, "JSON harus berupa array objek"),
            Self::MissingColumn { column } => {
                write!(f, "kolom wajib {column:?} tidak ada di header CSV")
            }
            Self::DuplicateColumn(column) => {
                write!(f, "kolom {column:?} muncul lebih dari sekali di header CSV")
            }
            Self::UnterminatedQuote { line } => {
                write!(f, "tanda kutip di baris {line} tidak ditutup")
            }
        }
    }
}

impl std::error::Error for ImportError {}

/// Mengimpor array objek JSON. Setiap objek punya `title` (teks, wajib), `status`
/// (teks, opsional), dan `labels` (array teks, opsional). Field lain diabaikan.
///
/// # Errors
///
/// [`ImportError::InvalidJson`] jika teks bukan JSON, [`ImportError::NotAnArray`] jika
/// akarnya bukan array. Baris yang salah tidak menghasilkan galat, melainkan masuk ke
/// [`ImportReport::rejected`].
pub fn import_json(text: &str) -> Result<ImportReport, ImportError> {
    let value: Value =
        serde_json::from_str(text).map_err(|e| ImportError::InvalidJson(e.to_string()))?;
    let Value::Array(items) = value else {
        return Err(ImportError::NotAnArray);
    };
    let mut report = ImportReport::default();
    for (index, item) in items.iter().enumerate() {
        match json_row(item) {
            Ok(task) => report.tasks.push(task),
            Err(kind) => report.rejected.push(RowError {
                row: index + 1,
                kind,
            }),
        }
    }
    Ok(report)
}

fn json_row(item: &Value) -> Result<TaskRecord, RowErrorKind> {
    let Value::Object(map) = item else {
        return Err(RowErrorKind::NotAnObject);
    };
    let title = match json_text(map, "title")? {
        Some(title) => title,
        None => return Err(RowErrorKind::MissingField("title")),
    };
    let status = json_text(map, "status")?.unwrap_or_default();
    let labels = json_labels(map)?;
    finish(&title, &status, labels)
}

fn json_text(
    map: &Map<String, Value>,
    field: &'static str,
) -> Result<Option<String>, RowErrorKind> {
    match map.get(field) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(text)) => Ok(Some(text.clone())),
        Some(_) => Err(RowErrorKind::WrongType {
            field,
            expected: "teks",
        }),
    }
}

fn json_labels(map: &Map<String, Value>) -> Result<Vec<String>, RowErrorKind> {
    const WRONG: RowErrorKind = RowErrorKind::WrongType {
        field: "labels",
        expected: "array teks",
    };
    match map.get("labels") {
        None | Some(Value::Null) => Ok(Vec::new()),
        Some(Value::Array(items)) => items
            .iter()
            .map(|item| match item {
                Value::String(label) => Ok(label.clone()),
                _ => Err(WRONG),
            })
            .collect(),
        Some(_) => Err(WRONG),
    }
}

/// Mengimpor CSV dengan header. Kolom `title` wajib. `status` dan `labels` opsional.
/// Kolom lain diabaikan. Teks boleh memakai tanda kutip RFC 4180 (`""` untuk kutip di
/// dalam), dan BOM di awal berkas dilewati. Baris kosong diabaikan.
///
/// # Errors
///
/// [`ImportError::MissingColumn`] jika `title` tidak ada di header,
/// [`ImportError::DuplicateColumn`] jika nama kolom berulang,
/// [`ImportError::UnterminatedQuote`] jika tanda kutip tidak tertutup.
/// Baris yang salah masuk ke [`ImportReport::rejected`].
pub fn import_csv(text: &str) -> Result<ImportReport, ImportError> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let records = parse_csv_records(text)?;
    let mut records = records.into_iter();
    let Some(header) = records.next() else {
        return Err(ImportError::MissingColumn { column: "title" });
    };

    let columns: Vec<String> = header
        .fields
        .iter()
        .map(|name| name.trim().to_ascii_lowercase())
        .collect();
    for (index, name) in columns.iter().enumerate() {
        if columns[..index].contains(name) {
            return Err(ImportError::DuplicateColumn(name.clone()));
        }
    }
    let position = |name: &str| columns.iter().position(|column| column == name);
    let title_at = position("title").ok_or(ImportError::MissingColumn { column: "title" })?;
    let status_at = position("status");
    let labels_at = position("labels");

    let mut report = ImportReport::default();
    for record in records {
        if record.fields.len() != columns.len() {
            report.rejected.push(RowError {
                row: record.line,
                kind: RowErrorKind::FieldCount {
                    expected: columns.len(),
                    found: record.fields.len(),
                },
            });
            continue;
        }
        let cell = |at: Option<usize>| {
            at.and_then(|i| record.fields.get(i))
                .map_or("", String::as_str)
        };
        let labels: Vec<String> = cell(labels_at)
            .split(';')
            .map(str::trim)
            .filter(|label| !label.is_empty())
            .map(str::to_owned)
            .collect();
        match finish(cell(Some(title_at)), cell(status_at), labels) {
            Ok(task) => report.tasks.push(task),
            Err(kind) => report.rejected.push(RowError {
                row: record.line,
                kind,
            }),
        }
    }
    Ok(report)
}

/// Validasi bersama untuk kedua format. Judul dipangkas. Status kosong menjadi `backlog`.
/// Label dipangkas, dibuang bila kosong, dan duplikatnya dibuang.
fn finish(title: &str, status: &str, labels: Vec<String>) -> Result<TaskRecord, RowErrorKind> {
    let title = title.trim();
    if title.is_empty() {
        return Err(RowErrorKind::EmptyTitle);
    }
    if title.chars().any(char::is_control) {
        return Err(RowErrorKind::InvalidTitle);
    }
    let status = if status.trim().is_empty() {
        TaskStatus::Backlog
    } else {
        TaskStatus::parse(status)
            .ok_or_else(|| RowErrorKind::InvalidStatus(status.trim().to_owned()))?
    };
    let mut clean: Vec<String> = Vec::with_capacity(labels.len());
    for label in labels {
        let trimmed = label.trim();
        if trimmed.is_empty()
            || trimmed.contains([',', ';'])
            || trimmed.chars().any(char::is_control)
        {
            return Err(RowErrorKind::InvalidLabel(label));
        }
        if !clean.iter().any(|existing| existing == trimmed) {
            clean.push(trimmed.to_owned());
        }
    }
    Ok(TaskRecord {
        title: title.to_owned(),
        status,
        labels: clean,
    })
}

/// Satu record CSV: nomor baris fisik awalnya dan field-fieldnya.
#[derive(Debug)]
struct CsvRecord {
    line: usize,
    fields: Vec<String>,
}

/// Memisah teks CSV menjadi record. Baris kosong (tanpa field) dilewati.
fn parse_csv_records(text: &str) -> Result<Vec<CsvRecord>, ImportError> {
    let mut records = Vec::new();
    let mut fields: Vec<String> = Vec::new();
    let mut field = String::new();
    let mut in_quotes = false;
    let mut line = 1usize;
    let mut record_line = 1usize;
    let mut chars = text.chars().peekable();

    while let Some(ch) = chars.next() {
        if in_quotes {
            match ch {
                '"' if chars.peek() == Some(&'"') => {
                    chars.next();
                    field.push('"');
                }
                '"' => in_quotes = false,
                '\n' => {
                    line += 1;
                    field.push('\n');
                }
                _ => field.push(ch),
            }
            continue;
        }
        match ch {
            '"' => in_quotes = true,
            ',' => fields.push(std::mem::take(&mut field)),
            '\r' => {}
            '\n' => {
                push_csv_record(&mut records, &mut fields, &mut field, record_line);
                line += 1;
                record_line = line;
            }
            _ => field.push(ch),
        }
    }
    if in_quotes {
        return Err(ImportError::UnterminatedQuote { line: record_line });
    }
    push_csv_record(&mut records, &mut fields, &mut field, record_line);
    Ok(records)
}

fn push_csv_record(
    records: &mut Vec<CsvRecord>,
    fields: &mut Vec<String>,
    field: &mut String,
    line: usize,
) {
    if fields.is_empty() && field.is_empty() {
        return;
    }
    fields.push(std::mem::take(field));
    records.push(CsvRecord {
        line,
        fields: std::mem::take(fields),
    });
}

#[cfg(test)]
#[path = "import_tasks_test.rs"]
mod import_tasks_test;
