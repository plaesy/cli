//! `plaesy acp`: melayani agen ACP v1 lewat stdio. Setiap `session/prompt` dijawab oleh satu
//! penyedia dari `delegate.providers` di `.plaesy/config.json`. Penyedia dipilih dengan
//! `--provider`, sehingga model bisa diganti lewat konfigurasi tanpa mengubah kode.

use std::io;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use super::CliResult;
use crate::delegate::providers::Getenv;
use crate::delegate::{self, Request};
use plaesy_adapter_sqlite::SqliteStore;
use plaesy_interface_acp::{serve, Agent, ModelBackend, ToolHost};

/// Melayani ACP lewat stdio. Stdout hanya berisi JSON-RPC.
#[derive(Debug, clap::Args)]
pub struct Args {
    /// Proyek yang `.plaesy/config.json`-nya dibaca, dan tempat `documents.sqlite` dibuat
    #[arg(long, default_value = ".")]
    pub path: PathBuf,
    /// Nama penyedia dari delegate.providers yang menjawab prompt
    #[arg(long)]
    pub provider: String,
    /// Izinkan penyedia yang ditandai billable. Tanpa ini, penyedia billable ditolak
    #[arg(long)]
    pub confirm: bool,
}

/// Model yang memanggil satu penyedia delegasi untuk setiap prompt.
struct ProviderModel {
    provider: Box<dyn delegate::Provider>,
}

impl ModelBackend for ProviderModel {
    fn complete(&self, prompt: &str, _tools: &mut dyn ToolHost) -> Result<String, String> {
        let request = Request {
            prompt: prompt.to_owned(),
            max_output_tokens: 0,
            timeout: Duration::from_secs(120),
            dir: String::new(),
        };
        delegate::run(self.provider.as_ref(), &request)
            .map(|response| response.text)
            .map_err(|error| error.to_string())
    }
}

/// Menjalankan agen sampai stdin ditutup.
///
/// # Errors
///
/// Konfigurasi tidak bisa dibaca, penyedia tidak dikenal atau tidak boleh dipakai, atau penyimpanan gagal.
pub fn run(args: &Args) -> CliResult {
    let cfg = super::delegate::load_delegate_config(&args.path)?;
    let entry = cfg
        .providers
        .iter()
        .find(|p| p.name == args.provider)
        .ok_or_else(|| {
            format!(
                "penyedia {:?} tidak ada di delegate.providers",
                args.provider
            )
        })?;
    if !entry.enabled {
        return Err(format!("penyedia {:?} nonaktif", args.provider).into());
    }
    if entry.billable && !args.confirm {
        return Err(format!(
            "penyedia {:?} billable; jalankan dengan --confirm untuk mengizinkannya",
            args.provider
        )
        .into());
    }
    let getenv: Getenv = Arc::new(|name: &str| std::env::var(name).ok());
    // Ollama lokal memakai loop alat (api/chat). Penyedia lain tetap teks saja.
    let model: Box<dyn ModelBackend + '_> = if entry.kind == "ollama" {
        let endpoint = if entry.endpoint.is_empty() {
            "http://127.0.0.1:11434".to_owned()
        } else {
            entry.endpoint.clone()
        };
        Box::new(plaesy_adapter_model_ollama::OllamaModel {
            endpoint,
            model: entry.model.clone(),
            timeout: Duration::from_secs(120),
        })
    } else {
        let provider = delegate::build(entry, getenv).map_err(|error| error.to_string())?;
        Box::new(ProviderModel { provider })
    };

    let plaesy_dir = args.path.join(".plaesy");
    std::fs::create_dir_all(&plaesy_dir)?;
    let store = SqliteStore::open(plaesy_dir.join("documents.sqlite"))?;
    let agent = Agent::with_model(&store, model);

    let stdin = io::stdin();
    let stdout = io::stdout();
    serve(&agent, stdin.lock(), stdout.lock())?;
    Ok(())
}
