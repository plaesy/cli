//! Command-line interface for plaesy-memory.
//!
//! ```text
//! memory index [DIR]              sync DIR (default: configured watch dir) into the index
//! memory search "query" [-n 10]   hybrid/semantic/keyword search with citations
//! memory list [--offset 0] [-n 50]  browse indexed note metadata without bodies
//! memory links NOTE [--outgoing]  notes linking to NOTE (or linked from it)
//! memory hygiene [DIR] [--project-root SLUG=PATH] [--json] stale notes, links, local paths
//! memory curate [DIR] [--limit 20] [--project-root SLUG=PATH] [--json] read-only maintenance review
//! memory code-find DIR --symbol NAME [--purpose TEXT] [--json]  find reusable Dart APIs
//! memory code-audit DIR [--json]  secondary diagnostic for duplicate Dart APIs in a project tree
//! memory missing [--resolve ID]   unanswered queries and broken or stale notes
//! memory stats                    index statistics
//! memory watch [DIR]              keep the index current until Ctrl-C
//! memory config                   print the effective configuration
//! memory config mcp [--app X]     integration snippets for MCP clients (Claude Code, n8n, Dify, Kilo Code, …)
//! memory serve [--mcp]            JSON service on stdin/stdout, or MCP server with --mcp
//! memory serve --mcp --transport http --port 9123   MCP over Streamable HTTP
//! memory remember "title" ["body"] [--template NAME] [--folder PATH]  write a note
//! memory fingerprint PATH       print the current note SHA-256 before update
//! memory update PATH --if-match SHA256 --reason correction --yes  replace from stdin
//! memory archive PATH --if-match SHA256 --reason completed --yes  mark a note inactive
//! memory schedule install|uninstall register or remove the per-user OS schedule
//! ```

use crate::memory::config::MemoryConfig;
use crate::memory::hygiene::HygieneOptions;
#[cfg(test)]
use crate::memory::hygiene::HygieneReport;
use crate::memory::indexer::Watcher;
use crate::memory::notes::WriteLock;
use crate::memory::search::{
    normalize_tier, CitationBuilder, SearchEngine, SearchFilter, SearchMode, Trust, TIERS,
};
use crate::memory::service::SearchHit;
use crate::memory::{self, init_logging_with, onboard, serve, settings, MemoryError, Result};
use clap::Parser;

mod definitions;
mod dispatch;
mod output;
use definitions::*;
use dispatch::dispatch;
use output::*;

pub async fn run_args(args: Vec<String>) -> Result<()> {
    use clap::error::ErrorKind;
    use std::ffi::OsString;

    let argv = std::iter::once(OsString::from("plaesy memory"))
        .chain(args.into_iter().map(OsString::from));
    let cli = match Cli::try_parse_from(argv) {
        Ok(cli) => cli,
        Err(error)
            if matches!(
                error.kind(),
                ErrorKind::DisplayHelp | ErrorKind::DisplayVersion
            ) =>
        {
            error.print().map_err(|source| MemoryError::IoError {
                path: "stdout".into(),
                source,
            })?;
            return Ok(());
        }
        Err(error) => return Err(MemoryError::ConfigError(error.to_string())),
    };
    run(cli).await
}

async fn run(cli: Cli) -> Result<()> {
    if matches!(
        &cli.command,
        Command::Schedule {
            mode: ScheduleMode::Uninstall
        }
    ) {
        crate::memory::schedule::registration::uninstall()?;
        return Ok(());
    }

    let mut config = match &cli.config {
        Some(p) => MemoryConfig::load_from_file(p).await?,
        None => MemoryConfig::load().await?,
    };
    if let Some(db) = cli.db {
        config.database.db_path = db;
    }
    init_logging_with(&config.logging)?;

    // A keyword-only query does not use either model. Keep the selected mode cheap by avoiding
    // vector-model and reranker initialization in `SearchEngine::from_config`.
    if matches!(
        &cli.command,
        Command::Search {
            mode: Mode::Keyword,
            ..
        }
    ) {
        config.search.enable_vector_search = false;
        config.search.rerank = false;
    }

    if let Command::Onboard {
        yes,
        vault,
        root,
        no_model,
        rerank,
        no_index,
    } = cli.command
    {
        return onboard::run(onboard::OnboardOptions {
            yes,
            vault,
            root,
            no_model,
            rerank,
            no_index,
        })
        .await;
    }
    if !matches!(
        cli.command,
        Command::Config { .. } | Command::Serve { .. } | Command::Schedule { .. }
    ) && settings::default_path().is_some_and(|p| !p.is_file())
    {
        eprintln!(
            "Tip: run `plaesy onboard` once to choose your vault folder and download the model."
        );
    }

    if let Command::Config { sub } = cli.command {
        if let Some(ConfigSubcommand::Mcp {
            app,
            list,
            binary,
            notes_dir,
            http_url,
        }) = sub
        {
            return memory::config::mcp::run(app, list, binary, notes_dir, http_url).await;
        }
        println!("{}", config.to_diagnostic_string());
        return Ok(());
    }
    if let Command::Serve {
        mcp,
        transport,
        port,
        auth_token,
        read_only,
    } = cli.command
    {
        let transport = transport.unwrap_or_else(|| {
            config
                .mcp
                .transport
                .parse()
                .expect("validated MCP transport")
        });
        let port = port.unwrap_or(config.mcp.port);
        let auth_token = match auth_token {
            Some(token) => Some(token),
            None if config.mcp.auth_token_env.is_empty() => None,
            None => crate::memory::settings::env_file::value(&config.mcp.auth_token_env)?,
        };
        let read_only = read_only || config.mcp.read_only;
        return serve::run(config, mcp, transport, port, auth_token, read_only).await;
    }

    if let Command::Schedule {
        mode: ScheduleMode::Install,
    } = &cli.command
    {
        require_dir(&config.watch_dir)?;
        crate::memory::schedule::jobs_from_config(&config.schedule)?;
        crate::memory::schedule::registration::install(&config)?;
        return Ok(());
    }

    if let Command::CodeFind {
        dir,
        symbol,
        purpose,
        include_tests,
        json,
    } = &cli.command
    {
        require_dir(dir)?;
        let report = crate::memory::code_audit::audit_with_options(
            dir,
            crate::memory::code_audit::Language::Dart,
            *include_tests,
        )
        .map_err(|e| MemoryError::IoError {
            path: dir.display().to_string(),
            source: e,
        })?;
        let db = crate::memory::storage::Database::new(config.database.clone()).await?;
        let graph = crate::memory::graph::load_graph(&db).await?;
        let project_scope = dir
            .file_name()
            .and_then(|name| name.to_str())
            .map(str::to_ascii_lowercase);
        let records = graph
            .notes
            .into_iter()
            .filter(|n| {
                project_scope.as_deref().is_none_or(|scope| {
                    n.project
                        .as_deref()
                        .is_some_and(|p| p.eq_ignore_ascii_case(scope))
                        || n.scope.iter().any(|p| p.eq_ignore_ascii_case(scope))
                })
            })
            .map(|n| {
                let status = n.effective_status().to_string();
                crate::memory::code_audit::CanonicalRecord {
                    id: n.id,
                    title: n.title,
                    path: n.path,
                    status,
                    project: n.project,
                    scope: n.scope,
                }
            })
            .collect();
        let result = crate::memory::code_audit::find(
            &report.declarations,
            symbol,
            purpose.as_deref(),
            records,
        );
        if *json {
            print_json(&result);
        } else {
            println!("{}: {}", result.decision, result.query);
            for c in &result.candidates {
                println!(
                    "{} {} ({}) {}:{}",
                    c.match_kind,
                    c.signature,
                    c.container.as_deref().unwrap_or(""),
                    c.file,
                    c.line
                );
            }
        }
        return Ok(());
    }

    if let Command::CodeAudit {
        dir,
        json,
        language: _,
    } = &cli.command
    {
        require_dir(dir)?;
        let mut report =
            crate::memory::code_audit::audit(dir, crate::memory::code_audit::Language::Dart)
                .map_err(|e| MemoryError::IoError {
                    path: dir.display().to_string(),
                    source: e,
                })?;
        let db = crate::memory::storage::Database::new(config.database.clone()).await?;
        let graph = crate::memory::graph::load_graph(&db).await?;
        let project_scope = dir
            .file_name()
            .and_then(|name| name.to_str())
            .map(str::to_ascii_lowercase);
        report.canonical_records = graph
            .notes
            .into_iter()
            .filter(|n| {
                project_scope.as_deref().is_none_or(|scope| {
                    n.project
                        .as_deref()
                        .is_some_and(|p| p.eq_ignore_ascii_case(scope))
                        || n.scope.iter().any(|p| p.eq_ignore_ascii_case(scope))
                })
            })
            .map(|n| {
                let status = n.effective_status().to_string();
                crate::memory::code_audit::CanonicalRecord {
                    id: n.id,
                    title: n.title,
                    path: n.path,
                    status,
                    project: n.project,
                    scope: n.scope,
                }
            })
            .collect();
        drop(db);
        if *json {
            println!(
                "{}",
                serde_json::to_string_pretty(&report)
                    .map_err(|e| MemoryError::ConfigError(e.to_string()))?
            );
        } else {
            println!(
                "scanned {} Dart files; {} declarations",
                report.files_scanned,
                report.declarations.len()
            );
            for g in &report.exact_duplicates {
                println!(
                    "ERROR exact duplicate {} ({} declarations)",
                    g.key,
                    g.declarations.len()
                );
            }
            for g in &report.semantic_duplicates {
                println!(
                    "WARNING likely semantic duplicate ({} declarations)",
                    g.declarations.len()
                );
            }
        }
        return Ok(());
    }
    if let Command::Fingerprint { path } = &cli.command {
        let hash = crate::memory::search::fingerprint_note_in_root(&config.watch_dir, path)?;
        println!("{hash}  {}", path.display());
        return Ok(());
    }
    let engine = SearchEngine::from_config(&config).await?;
    engine.set_watch_root(config.watch_dir.clone());
    dispatch(engine, config, cli.command).await
}

#[cfg(test)]
#[path = "cli_test.rs"]
mod cli_test;
