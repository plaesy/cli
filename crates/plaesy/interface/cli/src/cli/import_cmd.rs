//! `plaesy import --from sql|rust|python <path> --name <nama>`: impor skema atau kode menjadi IR JSON.
//! IR ke stdout, laporan ke stderr.

use std::path::PathBuf;

use super::CliResult;

/// Sumber impor.
#[derive(Debug, Clone, Copy, clap::ValueEnum)]
pub enum From {
    Sql,
    Rust,
    Python,
}

/// Argumen `plaesy import`.
#[derive(Debug, clap::Args)]
pub struct Args {
    /// Jenis sumber
    #[arg(long, value_enum)]
    pub from: From,
    /// Berkas SQLite (sql), atau berkas kode (rust, python)
    pub path: PathBuf,
    /// Nama IR yang dihasilkan
    #[arg(long)]
    pub name: String,
}

/// Menjalankan impor. IR JSON ke stdout.
///
/// # Errors
///
/// Berkas tidak bisa dibaca, kode tidak bisa diparse, atau galat SQLite.
pub fn run(args: &Args) -> CliResult {
    let (ir, report) = match args.from {
        From::Sql => {
            // Baca-saja dan tanpa membuat berkas: path yang salah harus gagal, bukan menghasilkan
            // database kosong.
            let conn = rusqlite::Connection::open_with_flags(
                &args.path,
                rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
            )?;
            let ir =
                plaesy_adapter_import_sql::import(&conn, &args.name).map_err(|e| e.to_string())?;
            (
                ir,
                String::from("sql: tidak ada laporan kehilangan terpisah"),
            )
        }
        From::Rust => {
            let source = std::fs::read_to_string(&args.path)?;
            let done = plaesy_adapter_import_code_rust::import(&source, &args.name)
                .map_err(|e| e.to_string())?;
            (done.ir, format!("{:?}", done.report))
        }
        From::Python => {
            let source = std::fs::read_to_string(&args.path)?;
            let done = plaesy_adapter_import_code_python::import(&source, &args.name)
                .map_err(|e| e.to_string())?;
            (done.ir, format!("{:?}", done.report))
        }
    };
    eprintln!("laporan: {report}");
    println!("{}", serde_json::to_string_pretty(&ir)?);
    Ok(())
}
