//! `memory serve`: the long-running entry point for Hermes-compatible JSON service on
//! stdin/stdout) and by MCP clients (`memory serve --mcp`).
//!
//! The service answers requests immediately, while a background task syncs the configured watch
//! directory into the index and then keeps it current with a file watcher. Logging must already be
//! initialised by the caller; stdout is reserved for the protocol.

use crate::memory::config::MemoryConfig;
use crate::memory::error::{MemoryError, Result};
use crate::memory::indexer::Watcher;
use crate::memory::mcp::{self, tokens};
use crate::memory::notes::WriteLock;
use crate::memory::search::SearchEngine;
use crate::memory::service;
use clap::ValueEnum;
use std::net::SocketAddr;
use std::sync::Arc;

/// Transport selection for `memory serve`.
#[derive(Clone, Copy, ValueEnum, Default, PartialEq, Eq)]
pub enum Transport {
    /// Stdio: newline-delimited JSON-RPC 2.0 (default).
    #[default]
    Stdio,
    /// HTTP Streamable: POST /mcp for requests, SSE for notifications.
    Http,
}

impl std::str::FromStr for Transport {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "stdio" => Ok(Transport::Stdio),
            "http" => Ok(Transport::Http),
            other => Err(format!(
                "unknown transport '{other}' (expected stdio or http)"
            )),
        }
    }
}

/// Run the service until stdin closes or the HTTP server stops. `mcp` selects the Model Context
/// Protocol transport; `transport` selects stdio vs HTTP.
pub async fn run(
    config: MemoryConfig,
    mcp: bool,
    transport: Transport,
    port: u16,
    auth_token: Option<String>,
    read_only: bool,
) -> Result<()> {
    if let Some(token) = auth_token.as_deref() {
        if !mcp || transport != Transport::Http {
            return Err(MemoryError::ConfigError(
                "--auth-token requires --mcp --transport http".into(),
            ));
        }
        if token.len() < 32 || token.trim() != token {
            return Err(MemoryError::ConfigError(
                "MCP bearer token must be at least 32 bytes and have no surrounding whitespace"
                    .into(),
            ));
        }
    }

    let token_file_exists = tokens::default_path().is_some_and(|path| path.is_file());
    if mcp && transport == Transport::Http && auth_token.is_none() && !token_file_exists {
        tracing::warn!(
            "MCP HTTP server has no bearer authentication; create a token with `plaesy memory token add` or set PLAESY_MCP_TOKEN before exposing it"
        );
    }

    // Read-only tidak mengambil lock daemon, sehingga vault yang sudah dimiliki daemon lain tetap bisa dibaca.
    let daemon_lock = if config.watch_dir.is_dir() && !read_only {
        Some(WriteLock::acquire_daemon(&config.watch_dir)?)
    } else {
        None
    };
    let engine = Arc::new(SearchEngine::from_config(&config).await?);
    engine.set_watch_root(config.watch_dir.clone());
    engine.set_daemon_mode(daemon_lock.is_some());
    engine.set_read_only(read_only);
    // Keep the ownership lock alive until the protocol server exits.
    let _daemon_lock = daemon_lock;

    let root = config.watch_dir.clone();
    if read_only {
        tracing::info!("read-only: serving the existing index without sync or watching");
    } else if root.is_dir() {
        // The watcher starts first so edits made during the initial sync are not lost.
        // Sync + watching run in the background: the service answers requests immediately
        // (searches see whatever is indexed so far) instead of blocking on a long first index.
        let mut watcher = Watcher::new(&root.to_string_lossy()).await?;
        let bg_engine = Arc::clone(&engine);
        tokio::spawn(async move {
            match bg_engine.index_directory(&root).await {
                Ok(s) => {
                    tracing::info!(
                        indexed = s.indexed,
                        unchanged = s.unchanged,
                        removed = s.removed,
                        failed = s.failed.len(),
                        "initial sync of {} done",
                        root.display()
                    );
                    for (path, error) in &s.failed {
                        tracing::warn!("could not index {path}: {error}");
                    }
                }
                Err(e) => tracing::warn!("initial sync failed: {e}"),
            }
            loop {
                match watcher.changes().await {
                    Ok(events) if events.is_empty() => break, // watcher shut down
                    Ok(events) => {
                        let n = bg_engine.apply_events(&root, &events).await;
                        tracing::info!("applied {n}/{} file events", events.len());
                    }
                    Err(e) => tracing::warn!("watcher error: {e}"),
                }
            }
        });
    } else {
        tracing::warn!(
            "watch dir {} not found; serving existing index only",
            root.display()
        );
    }

    if !mcp {
        if transport == Transport::Http {
            return Err(MemoryError::ConfigError(
                "--transport http requires --mcp; the plain JSON service is stdio-only".into(),
            ));
        }
        // JSON service (Hermes provider) is stdio-only.
        return service::serve(&engine, tokio::io::stdin(), tokio::io::stdout()).await;
    }

    match transport {
        Transport::Stdio => mcp::serve_stdio(engine).await,
        Transport::Http => {
            let addr: SocketAddr = format!("127.0.0.1:{port}")
                .parse()
                .map_err(|e| MemoryError::ConfigError(format!("bad port {port}: {e}")))?;
            let auth =
                tokens::TokenAuth::new(auth_token, tokens::default_path(), &config.watch_dir)?;
            mcp::serve_http(engine, addr, auth).await
        }
    }
}
