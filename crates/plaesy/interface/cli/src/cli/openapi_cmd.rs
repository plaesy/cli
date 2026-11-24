//! `plaesy openapi <file.json>`: membuat OpenAPI 3.1 dari api-ir/1. JSON ke stdout; galat ke stderr dengan exit code 1.

use std::path::PathBuf;

use super::CliResult;

/// Argumen `plaesy openapi`.
#[derive(Debug, clap::Args)]
pub struct Args {
    /// Dokumen api-ir/1 (JSON)
    pub file: PathBuf,
}

/// Menjalankan generator OpenAPI. Galat dicetak ke stderr dan proses keluar dengan kode 1.
///
/// # Errors
///
/// Berkas tidak bisa dibaca, atau dokumen tidak lolos aturan semantik.
pub fn run(args: &Args) -> CliResult {
    let text = std::fs::read_to_string(&args.file)?;
    let doc: serde_json::Value = serde_json::from_str(&text)?;
    match plaesy_domain_api::openapi::to_openapi(&doc) {
        Ok(openapi) => {
            println!("{}", serde_json::to_string_pretty(&openapi)?);
            Ok(())
        }
        Err(errors) => {
            for error in &errors {
                eprintln!("{error}");
            }
            std::process::exit(1);
        }
    }
}
