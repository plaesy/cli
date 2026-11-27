//! `plaesy server`: the document API over HTTP, assembled here (ADR 0003: `cli` is the only adapter assembler).
//!
//! Config layers, lowest to highest: built-in defaults, the `--config` TOML file, `PLAESY_SERVER_*`
//! environment variables, then command-line flags. The bind address is loopback by default. A
//! non-loopback bind is refused until authentication exists (backend plan, slice 4).

use std::future::Future;
use std::io;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use plaesy_adapter_sqlite::SqliteStore;
use plaesy_interface_http::{router, SharedStore};
use serde::Deserialize;
use thiserror::Error;
use tokio::net::TcpListener;

use super::CliResult;

/// Default listen address (decisions.md, backend server). Port 8787 is used by another service.
pub const DEFAULT_BIND: &str = "127.0.0.1:8790";
/// Default document database, relative to the working directory.
pub const DEFAULT_DB: &str = ".plaesy/server.sqlite";
/// Prefix for environment overrides, for example `PLAESY_SERVER_BIND`.
const ENV_PREFIX: &str = "PLAESY_SERVER_";
/// Authentication is not implemented yet (slice 3 and 4). Until it is, non-loopback binds stay refused.
const AUTH_ENABLED: bool = false;

/// Command-line arguments. Security-sensitive switches are file or environment only, on purpose.
#[derive(Debug, clap::Args)]
pub struct Args {
    /// TOML config file, applied below environment variables and flags
    #[arg(long)]
    pub config: Option<PathBuf>,
    /// Address to listen on, for example 127.0.0.1:8790
    #[arg(long)]
    pub bind: Option<String>,
    /// SQLite database file that holds the documents
    #[arg(long)]
    pub db: Option<PathBuf>,
}

impl Args {
    fn layer(&self) -> Layer {
        Layer {
            bind: self.bind.clone(),
            db: self.db.clone(),
            ..Layer::default()
        }
    }
}

/// One configuration layer. `None` means this layer does not set the key.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Layer {
    pub bind: Option<String>,
    pub db: Option<PathBuf>,
    pub allow_non_loopback: Option<bool>,
    pub tls_terminated_upstream: Option<bool>,
    pub cors_origins: Option<Vec<String>>,
}

impl Layer {
    /// Keeps the keys of `self` and replaces each one that `higher` sets.
    #[must_use]
    pub fn overlay(self, higher: Layer) -> Layer {
        Layer {
            bind: higher.bind.or(self.bind),
            db: higher.db.or(self.db),
            allow_non_loopback: higher.allow_non_loopback.or(self.allow_non_loopback),
            tls_terminated_upstream: higher
                .tls_terminated_upstream
                .or(self.tls_terminated_upstream),
            cors_origins: higher.cors_origins.or(self.cors_origins),
        }
    }
}

/// Galat konfigurasi yang bisa langsung ditampilkan ke operator.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ConfigError {
    #[error("alamat bind tidak valid: {0}")]
    BadBind(String),
    #[error("nilai {name} tidak valid: {value} (pakai true atau false)")]
    BadFlag { name: String, value: String },
    #[error("origin CORS tidak valid: {0} (pakai http:// atau https:// tanpa path dan tanpa *)")]
    BadOrigin(String),
    #[error("bind non-loopback ditolak: butuh allow_non_loopback dan tls_terminated_upstream")]
    NonLoopbackRefused,
    #[error("bind non-loopback belum diizinkan: autentikasi perangkat belum aktif")]
    AuthRequired,
}

/// Configuration after all layers are merged and checked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerConfig {
    pub bind: SocketAddr,
    pub db: PathBuf,
    pub allow_non_loopback: bool,
    pub tls_terminated_upstream: bool,
    pub cors_origins: Vec<String>,
}

/// Builds the `Layer` for `PLAESY_SERVER_*` variables. `get` returns the raw value of a variable.
/// Empty values count as unset.
///
/// # Errors
///
/// [`ConfigError::BadFlag`] when a boolean variable is not `true`, `false`, `1`, or `0`.
pub fn layer_from_env(get: impl Fn(&str) -> Option<String>) -> Result<Layer, ConfigError> {
    let read = |suffix: &str| {
        let name = format!("{ENV_PREFIX}{suffix}");
        get(&name).filter(|value| !value.is_empty()).map(|value| (name, value))
    };
    Ok(Layer {
        bind: read("BIND").map(|(_, value)| value),
        db: read("DB").map(|(_, value)| PathBuf::from(value)),
        allow_non_loopback: flag(read("ALLOW_NON_LOOPBACK"))?,
        tls_terminated_upstream: flag(read("TLS_TERMINATED_UPSTREAM"))?,
        cors_origins: read("CORS_ORIGINS").map(|(_, value)| {
            value
                .split(',')
                .map(|origin| origin.trim().to_owned())
                .filter(|origin| !origin.is_empty())
                .collect()
        }),
    })
}

fn flag(entry: Option<(String, String)>) -> Result<Option<bool>, ConfigError> {
    match entry {
        None => Ok(None),
        Some((_, value)) if value == "true" || value == "1" => Ok(Some(true)),
        Some((_, value)) if value == "false" || value == "0" => Ok(Some(false)),
        Some((name, value)) => Err(ConfigError::BadFlag { name, value }),
    }
}

/// Merges the three layers (file, environment, flags) over the defaults and checks them.
///
/// # Errors
///
/// Returns the first invalid value.
pub fn resolve(file: Layer, env: Layer, flags: Layer) -> Result<ServerConfig, ConfigError> {
    let merged = file.overlay(env).overlay(flags);
    let bind_text = merged.bind.unwrap_or_else(|| DEFAULT_BIND.to_owned());
    let bind = bind_text
        .parse::<SocketAddr>()
        .map_err(|_| ConfigError::BadBind(bind_text.clone()))?;
    let cors_origins = merged.cors_origins.unwrap_or_default();
    for origin in &cors_origins {
        validate_origin(origin)?;
    }
    Ok(ServerConfig {
        bind,
        db: merged.db.unwrap_or_else(|| PathBuf::from(DEFAULT_DB)),
        allow_non_loopback: merged.allow_non_loopback.unwrap_or(false),
        tls_terminated_upstream: merged.tls_terminated_upstream.unwrap_or(false),
        cors_origins,
    })
}

fn validate_origin(origin: &str) -> Result<(), ConfigError> {
    let host = origin
        .strip_prefix("https://")
        .or_else(|| origin.strip_prefix("http://"));
    match host {
        Some(host) if !host.is_empty() && !host.contains(['/', '*', ' ']) => Ok(()),
        _ => Err(ConfigError::BadOrigin(origin.to_owned())),
    }
}

/// Gerbang bind. Loopback always passes. A non-loopback bind needs both opt-ins and authentication.
///
/// # Errors
///
/// [`ConfigError::NonLoopbackRefused`] or [`ConfigError::AuthRequired`].
pub fn check_bind(config: &ServerConfig, auth_enabled: bool) -> Result<(), ConfigError> {
    if config.bind.ip().is_loopback() {
        return Ok(());
    }
    if !(config.allow_non_loopback && config.tls_terminated_upstream) {
        return Err(ConfigError::NonLoopbackRefused);
    }
    if !auth_enabled {
        return Err(ConfigError::AuthRequired);
    }
    Ok(())
}

/// Entry point for `plaesy server`. Diagnostics go to stderr; stdout stays empty.
pub fn run(args: &Args) -> CliResult {
    let file = match &args.config {
        Some(path) => toml::from_str::<Layer>(&std::fs::read_to_string(path)?)?,
        None => Layer::default(),
    };
    let env = layer_from_env(|name| std::env::var(name).ok())?;
    let config = resolve(file, env, args.layer())?;
    check_bind(&config, AUTH_ENABLED)?;
    let runtime = tokio::runtime::Runtime::new()?;
    runtime.block_on(run_until_shutdown(config))
}

async fn run_until_shutdown(config: ServerConfig) -> CliResult {
    let listener = TcpListener::bind(config.bind).await?;
    let store = open_store(&config.db)?;
    log_event("info", "listening", Some(listener.local_addr()?));
    serve_on(listener, store, shutdown_signal()).await?;
    log_event("info", "stopped", None);
    Ok(())
}

/// Opens the SQLite document store and migrates it on open.
///
/// # Errors
///
/// Storage errors, or a schema newer than this build supports.
pub fn open_store(db: &Path) -> Result<SharedStore, Box<dyn std::error::Error>> {
    let store = SqliteStore::open(db)?;
    let shared: SharedStore = Arc::new(store);
    Ok(shared)
}

/// Serves the document router on `listener` until `shutdown` completes, then drains connections.
///
/// # Errors
///
/// Accept or serve errors from the listener.
pub async fn serve_on(
    listener: TcpListener,
    store: SharedStore,
    shutdown: impl Future<Output = ()> + Send + 'static,
) -> io::Result<()> {
    axum::serve(listener, router(store))
        .with_graceful_shutdown(shutdown)
        .await
}

/// Completes on Ctrl+C, or on SIGTERM where the platform has it.
async fn shutdown_signal() {
    let ctrl_c = async {
        // A failed handler leaves the server running; there is nothing better to do here.
        let _ = tokio::signal::ctrl_c().await;
    };
    #[cfg(unix)]
    let terminate = async {
        if let Ok(mut signal) =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        {
            signal.recv().await;
        } else {
            std::future::pending::<()>().await;
        }
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();
    tokio::select! {
        () = ctrl_c => {}
        () = terminate => {}
    }
}

fn log_event(level: &str, event: &str, bind: Option<SocketAddr>) {
    let line = serde_json::json!({
        "level": level,
        "event": event,
        "bind": bind.map(|address| address.to_string()),
    });
    eprintln!("{line}");
}

#[cfg(test)]
#[path = "server_test.rs"]
mod server_test;
