//! `plaesy codegen --lang js|go <file.json>`: membuat kode dari flow-ir/1. Kode ke stdout, laporan ke stderr.

use std::path::PathBuf;

use super::CliResult;

/// Bahasa target codegen.
#[derive(Debug, Clone, Copy, clap::ValueEnum)]
pub enum Lang {
    Js,
    Go,
}

/// Argumen `plaesy codegen`.
#[derive(Debug, clap::Args)]
pub struct Args {
    /// Bahasa target
    #[arg(long, value_enum)]
    pub lang: Lang,
    /// Dokumen flow-ir/1 (JSON)
    pub file: PathBuf,
}

/// Menjalankan codegen. Hasil ke stdout, laporan kehilangan ke stderr.
///
/// # Errors
///
/// Berkas tidak bisa dibaca, JSON tidak valid, atau flow tidak valid.
pub fn run(args: &Args) -> CliResult {
    let text = std::fs::read_to_string(&args.file)?;
    let doc: serde_json::Value = serde_json::from_str(&text)?;
    let generated = match args.lang {
        Lang::Js => plaesy_adapter_codegen_js::generate(&doc)
            .map(|g| (g.source, format!("{:?}", g.report)))
            .map_err(|error| error.to_string()),
        Lang::Go => plaesy_adapter_codegen_go::generate(&doc)
            .map(|g| (g.source, format!("{:?}", g.report)))
            .map_err(|error| error.to_string()),
    }?;
    eprintln!("laporan: {}", generated.1);
    print!("{}", generated.0);
    Ok(())
}
