//! Command-line interface for plaesy-memory.
//!
//! ```text
//! memory index [DIR]              sync DIR (default: configured watch dir) into the index
//! memory search "query" [-n 10]   hybrid/semantic/keyword search with citations
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
//! memory serve --mcp --transport http --port 8080   MCP over Streamable HTTP
//! memory remember "title" ["body"] [--template NAME] [--folder PATH]  write a note
//! memory fingerprint PATH       print the current note SHA-256 before update
//! memory update PATH --if-match SHA256 --reason correction --yes  replace from stdin
//! memory archive PATH --if-match SHA256 --reason completed --yes  mark a note inactive
//! memory schedule install|uninstall register or remove the per-user OS schedule
//! ```

use crate::memory::config::MemoryConfig;
use crate::memory::curation::CurationReport;
use crate::memory::hygiene::{HygieneOptions, HygieneReport};
use crate::memory::indexer::Watcher;
use crate::memory::notes::WriteLock;
use crate::memory::search::{
    normalize_tier, CitationBuilder, SearchEngine, SearchFilter, SearchMode, Trust, TIERS,
};
use crate::memory::serve::Transport;
use crate::memory::service::SearchHit;
use crate::memory::{self, init_logging_with, onboard, serve, settings, MemoryError, Result};
use clap::{Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "plaesy memory", version, about = "Offline-first memory index")]
struct Cli {
    /// Config file (TOML) layered over defaults instead of the standard lookup.
    #[arg(long, global = true)]
    config: Option<PathBuf>,

    /// SQLite database path (overrides config).
    #[arg(long, global = true)]
    db: Option<PathBuf>,

    #[command(subcommand)]
    command: Command,
}

fn hygiene_strict_failure(report: &HygieneReport, strict: bool, strict_facts: bool) -> bool {
    if strict {
        !report.findings.is_empty()
    } else if strict_facts {
        report
            .findings
            .iter()
            .any(|finding| finding.label == "FACT")
    } else {
        false
    }
}

#[derive(Clone, Copy, ValueEnum)]
enum Mode {
    Hybrid,
    Semantic,
    Keyword,
}

/// Output format for search results.
#[derive(Clone, Copy, Default, ValueEnum)]
enum OutputFormat {
    /// Plain text listing.
    #[default]
    Text,
    /// JSON array of hits.
    Json,
    /// Markdown list of hits.
    Md,
}

/// Structural gaps shown per kind by `memory missing` unless `--all` is given.
const SAMPLE_PER_KIND: usize = 5;

#[derive(Subcommand)]
enum Command {
    /// Sync a directory into the index (new/changed files in, deleted files out).
    Index { dir: Option<PathBuf> },
    /// Print the SHA-256 of a note for a conflict-checked update.
    Fingerprint { path: PathBuf },
    /// Search the index.
    Search {
        query: String,
        #[arg(short = 'n', long, default_value_t = 10)]
        limit: usize,
        #[arg(long, value_enum, default_value_t = Mode::Hybrid)]
        mode: Mode,
        /// Output format (text, json, md).
        #[arg(long, value_enum, default_value_t = OutputFormat::Text)]
        format: OutputFormat,
        /// Only these tiers (repeatable): core, episodic, semantic, procedural, archive.
        #[arg(long = "tier")]
        tiers: Vec<String>,
        /// Only notes whose path starts with this prefix.
        #[arg(long)]
        path: Option<String>,
        /// Only notes declaring at least this trust (high, medium, low).
        #[arg(long)]
        min_trust: Option<String>,
        /// Also return superseded / deprecated notes.
        #[arg(long)]
        include_inactive: bool,
        /// Only notes whose declared valid_from..valid_to period contains this date (YYYY-MM-DD).
        #[arg(long)]
        as_of: Option<String>,
        /// Also add the best passage of notes linked to or from the top hits (one hop).
        #[arg(long)]
        expand_links: bool,
        /// Also print per-leg score breakdown (fts, vector, fused, staleness, rerank).
        #[arg(long)]
        explain: bool,
    },
    /// Wikilinks: notes linking to NOTE (default) or linked from it.
    Links {
        note: String,
        /// List the links going out of NOTE instead.
        #[arg(long)]
        outgoing: bool,
    },
    /// Find an existing API to reuse (read-only).
    CodeFind {
        dir: PathBuf,
        #[arg(long)]
        symbol: String,
        #[arg(long)]
        purpose: Option<String>,
        #[arg(long)]
        include_tests: bool,
        #[arg(long)]
        json: bool,
    },
    /// Secondary diagnostic: audit project source APIs for duplicates.
    CodeAudit {
        dir: PathBuf,
        #[arg(long)]
        json: bool,
        #[arg(long, value_parser = ["dart", "auto"], default_value = "auto")]
        language: String,
    },
    /// Audit notes for stale metadata, broken links, heuristic dead paths, and possible secrets.
    Hygiene {
        /// Directory the notes live in (enables dead-path checks; default: configured watch dir).
        dir: Option<PathBuf>,
        /// Local source repository for a vault project, in SLUG=PATH form (repeatable).
        /// Repeat a slug to map a project to multiple repositories.
        #[arg(long = "project-root", value_name = "SLUG=PATH", action = clap::ArgAction::Append)]
        project_roots: Vec<String>,
        /// Emit JSON.
        #[arg(long)]
        json: bool,
        /// Exit with status 1 when any finding is reported (for CI).
        #[arg(long)]
        strict: bool,
        /// Exit with status 1 only for confirmed (FACT) findings; still report inferred findings.
        #[arg(long, conflicts_with = "strict")]
        strict_facts: bool,
    },
    /// Prepare a read-only review of memory quality and maintenance candidates.
    Curate {
        /// Directory the notes live in (default: configured watch directory).
        dir: Option<PathBuf>,
        /// Local source repository for a vault project, in SLUG=PATH form (repeatable).
        #[arg(long = "project-root", value_name = "SLUG=PATH", action = clap::ArgAction::Append)]
        project_roots: Vec<String>,
        /// Maximum examples per candidate category.
        #[arg(short = 'n', long, default_value_t = 20)]
        limit: usize,
        /// Emit machine-readable JSON.
        #[arg(long)]
        json: bool,
    },
    /// What the memory is missing: queries it could not answer, and broken or stale notes.
    Missing {
        /// Close the query gap with this id (it reopens if the query misses again).
        #[arg(long)]
        resolve: Option<i64>,
        /// Maximum number of query gaps to list.
        #[arg(short = 'n', long, default_value_t = 20)]
        limit: usize,
        /// Emit JSON.
        #[arg(long)]
        json: bool,
        /// List every structural gap, not a sample per kind.
        #[arg(long)]
        all: bool,
        /// Directory the notes live in (default: configured watch dir).
        dir: Option<PathBuf>,
    },
    /// Show index statistics.
    Stats,
    /// Watch a directory and keep the index current.
    Watch { dir: Option<PathBuf> },
    /// Print the effective configuration.
    Config {
        /// Subcommand: `mcp` prints integration snippets for MCP clients.
        #[command(subcommand)]
        sub: Option<ConfigSubcommand>,
    },
    /// Run the long-running service: JSON on stdin/stdout (Hermes-compatible), or MCP with --mcp.
    Serve {
        /// Speak the Model Context Protocol instead of the line-delimited JSON service.
        #[arg(long)]
        mcp: bool,
        /// Transport: `stdio` (default) or `http` (Streamable HTTP for n8n, Dify, Kilo Code, …).
        #[arg(long, value_enum, default_value_t = Transport::Stdio)]
        transport: Transport,
        /// TCP port to listen on when `--transport http` is set (default: 8080).
        #[arg(long, default_value_t = 8080)]
        port: u16,
    },
    /// First-run setup: vault folder, settings.json, model download, first index.
    Onboard {
        /// Accept every default (needs --vault unless settings already exist).
        #[arg(short = 'y', long)]
        yes: bool,
        /// Vault: the folder with the notes to index.
        #[arg(long)]
        vault: Option<PathBuf>,
        /// Folder that holds Plaesy/settings.json (default: your local app data folder).
        #[arg(long)]
        root: Option<PathBuf>,
        /// Do not download the embedding model.
        #[arg(long)]
        no_model: bool,
        /// Enable reranking and download the reranker model.
        #[arg(long)]
        rerank: bool,
        /// Do not index the notes at the end.
        #[arg(long)]
        no_index: bool,
    },
    /// Run scheduled jobs from the config: `once` runs what is due and exits (for Task Scheduler);
    /// `run` keeps going until Ctrl-C.
    Schedule {
        /// `once`, `run`, `install`, or `uninstall`.
        #[arg(value_enum)]
        mode: ScheduleMode,
    },
    /// List or inspect `type: project` notes, and (with a subcommand) their decisions/contexts.
    Projects {
        /// Project id, id-prefix, or title (omit to list every project). `*` is only valid
        /// immediately before `decisions`/`contexts`, meaning "every project".
        id: Option<String>,
        #[command(subcommand)]
        sub: Option<ProjectsSubcommand>,
        /// Also list paused/completed/archived projects (ignored when `id` is given).
        #[arg(long)]
        include_inactive: bool,
        /// Output format (text, json).
        #[arg(long, value_enum, default_value_t = GraphFormat::Text)]
        format: GraphFormat,
    },
    /// List `type: decision` notes, across every project or filtered to some.
    Decisions {
        /// Only decisions linked (via `project` or `scope`) to one of these ids (repeatable).
        /// An id that matches no project yields an empty result, not an error.
        #[arg(long = "project")]
        projects: Vec<String>,
        /// Only decisions with one of these statuses (repeatable): proposed, active,
        /// superseded, revoked. Overrides the default hiding of superseded/revoked.
        #[arg(long = "status")]
        status: Vec<String>,
        /// Also show superseded/revoked decisions.
        #[arg(long)]
        include_inactive: bool,
        #[arg(short = 'n', long, default_value_t = 50)]
        limit: usize,
        #[arg(long, value_enum, default_value_t = GraphFormat::Text)]
        format: GraphFormat,
    },
    /// List `type: context` notes, across every project or filtered to some.
    Contexts {
        /// Only contexts linked (via `project` or `scope`) to one of these ids (repeatable).
        #[arg(long = "project")]
        projects: Vec<String>,
        #[arg(long = "status")]
        status: Vec<String>,
        /// Also show archived contexts.
        #[arg(long)]
        include_inactive: bool,
        #[arg(short = 'n', long, default_value_t = 50)]
        limit: usize,
        #[arg(long, value_enum, default_value_t = GraphFormat::Text)]
        format: GraphFormat,
    },
    /// Write a new note (optionally tagged into the projects/decisions/contexts graph).
    ///
    /// This is the same `remember` write path MCP clients use (`memory_remember`), exposed on
    /// the command line so any script, cron job, or agent that only shells out can write memory
    /// without speaking MCP. Creates a file only; an existing note is never overwritten.
    Remember {
        /// Note title; also the basis of the file name (project templates default to README.md).
        title: String,
        /// Markdown body. Without --template, omission reads stdin; with --template, it is optional.
        body: Option<String>,
        /// Folder relative to the notes root (default: inbox for standard notes, type-specific for some templates).
        #[arg(long)]
        folder: Option<String>,
        /// Start from a Markdown template in <vault>/templates/<name>.md (nested paths are allowed).
        #[arg(long)]
        template: Option<String>,
        /// Replace a template placeholder (repeatable), in KEY=VALUE form.
        #[arg(long = "set", value_name = "KEY=VALUE")]
        values: Vec<String>,
        /// core, episodic, semantic, procedural, archive.
        #[arg(long, default_value = "semantic")]
        tier: String,
        /// high, medium, low. A `high` claim from an agent is stored as `medium`.
        #[arg(long, default_value = "medium")]
        confidence: String,
        /// Tag this note into the graph: project, decision, context, knowledge, capture, preference.
        #[arg(long = "type")]
        note_type: Option<String>,
        /// Stable graph id; required with --type and must be a UUID with --template (otherwise generated).
        #[arg(long)]
        id: Option<String>,
        /// Graph status (e.g. active, proposed, superseded — see `memory` graph module docs).
        #[arg(long)]
        status: Option<String>,
        /// decision/context: the project this note mainly belongs to.
        #[arg(long)]
        project: Option<String>,
        /// Also relevant to these projects (repeatable).
        #[arg(long = "scope")]
        scope: Vec<String>,
        /// decision: ids of decisions this one replaces (repeatable).
        #[arg(long = "supersedes")]
        supersedes: Vec<String>,
        /// Print the created file's path as JSON instead of plain text.
        #[arg(long)]
        json: bool,
    },
    /// Replace a note from stdin after confirming its reviewed SHA-256.
    Update {
        /// File path relative to the notes root.
        path: PathBuf,
        /// SHA-256 printed by `memory fingerprint`.
        #[arg(long = "if-match")]
        expected_sha256: String,
        /// Structured reason category; free-form text is not persisted.
        #[arg(long, value_parser = ["correction", "new-evidence", "clarification", "other"])]
        reason: String,
        /// Required confirmation for replacing the source file.
        #[arg(long, required = true)]
        yes: bool,
    },
    /// Mark a note archived; preserve the source file and hide it from default retrieval.
    Archive {
        /// File path relative to the notes root.
        path: PathBuf,
        /// SHA-256 printed by `memory fingerprint`.
        #[arg(long = "if-match")]
        expected_sha256: String,
        /// Structured reason category; free-form reason text is not persisted.
        #[arg(
            long,
            value_parser = ["completed", "obsolete", "superseded", "consolidated", "other"]
        )]
        reason: String,
        /// Required confirmation for changing the note's status.
        #[arg(long, required = true)]
        yes: bool,
    },
    /// Permanently remove a note from this vault and its index, leaving a synced tombstone.
    Forget {
        /// File path relative to the notes root.
        path: PathBuf,
        /// Structured reason category; free-form text is not persisted.
        #[arg(long, value_parser = ["owner-request", "duplicate", "obsolete", "privacy"])]
        reason: String,
        /// Required confirmation for permanent source-file removal.
        #[arg(long, required = true)]
        yes: bool,
    },
}

/// Output format for the projects/decisions/contexts commands.
#[derive(Clone, Copy, Default, ValueEnum)]
enum GraphFormat {
    #[default]
    Text,
    Json,
}

/// Subcommands of `memory projects <id|*>`.
#[derive(Subcommand, Clone)]
enum ProjectsSubcommand {
    /// Decisions belonging to this project (or every project, with `*`).
    Decisions {
        #[arg(long)]
        include_inactive: bool,
        #[arg(long = "status")]
        status: Vec<String>,
        #[arg(short = 'n', long, default_value_t = 50)]
        limit: usize,
        #[arg(long, value_enum, default_value_t = GraphFormat::Text)]
        format: GraphFormat,
    },
    /// Contexts belonging to this project (or every project, with `*`).
    Contexts {
        #[arg(long)]
        include_inactive: bool,
        #[arg(long = "status")]
        status: Vec<String>,
        #[arg(short = 'n', long, default_value_t = 50)]
        limit: usize,
        #[arg(long, value_enum, default_value_t = GraphFormat::Text)]
        format: GraphFormat,
    },
}

/// Subcommands for `memory config`.
#[derive(Subcommand, Debug, Clone)]
enum ConfigSubcommand {
    /// Print integration snippets for MCP clients (Claude Code, CrewAI, n8n, Dify, Kilo Code, Paperclip).
    ///
    /// Use `--app` to print only one client's snippet, or `--list` to print every app's snippet.
    Mcp {
        /// Print snippets for only this app (e.g. `claude-code`, `crewai`, `n8n`, `dify`, `kilocode`, `paperclip`).
        #[arg(long)]
        app: Option<String>,
        /// Print every app's snippet.
        #[arg(long)]
        list: bool,
        /// Path or command name for the `plaesy` executable to embed in the snippets.
        #[arg(long, default_value = "plaesy")]
        binary: String,
        /// Notes directory to embed in the snippets.
        #[arg(long, default_value = "./notes")]
        notes_dir: String,
        /// HTTP URL of the MCP server (when using `--transport http`).
        #[arg(long)]
        http_url: Option<String>,
    },
}

#[derive(Clone, Copy, ValueEnum)]
enum ScheduleMode {
    Once,
    Run,
    Install,
    Uninstall,
}

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
            "Tip: run `memory onboard` once to choose your vault folder and download the model."
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
    } = cli.command
    {
        return serve::run(config, mcp, transport, port).await;
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
    match cli.command {
        Command::Config { sub: _ }
        | Command::Serve { .. }
        | Command::Onboard { .. }
        | Command::CodeFind { .. }
        | Command::CodeAudit { .. } => {
            unreachable!("handled above")
        }
        Command::Schedule { mode } => {
            let jobs = crate::memory::schedule::jobs_from_config(&config.schedule)?;
            let root = config.watch_dir.clone();
            require_dir(&root)?;
            let log = crate::memory::schedule::log_path();
            match mode {
                ScheduleMode::Once => {
                    let now = chrono::Utc::now();
                    for o in
                        crate::memory::schedule::run_due(&engine, &root, &jobs, now, log.as_deref())
                            .await?
                    {
                        println!("{}", o.line(now));
                    }
                }
                ScheduleMode::Run => {
                    crate::memory::schedule::run_loop(&engine, &root, &jobs, log.as_deref())
                        .await?;
                }
                ScheduleMode::Uninstall => {
                    unreachable!("schedule uninstall is handled before configuration load")
                }
                ScheduleMode::Install => {
                    unreachable!("schedule registration modes are handled before engine creation")
                }
            }
        }
        Command::Index { dir } => {
            let root = dir.unwrap_or_else(|| config.watch_dir.clone());
            require_dir(&root)?;
            let s = engine.index_directory(&root).await?;
            println!(
                "indexed {} | unchanged {} | removed {} | chunks written {} | failed {}",
                s.indexed,
                s.unchanged,
                s.removed,
                s.chunks,
                s.failed.len()
            );
            for (path, error) in &s.failed {
                eprintln!("  failed: {path}: {error}");
            }
            if !s.failed.is_empty() {
                std::process::exit(2);
            }
        }
        Command::Fingerprint { path } => {
            let hash = engine.fingerprint_note(&path)?;
            println!("{hash}  {}", path.display());
        }
        Command::Search {
            query,
            limit,
            mode,
            format,
            tiers,
            path,
            min_trust,
            include_inactive,
            as_of,
            expand_links,
            explain,
        } => {
            for t in &tiers {
                if normalize_tier(t).is_none() {
                    return Err(MemoryError::ConfigError(format!(
                        "unknown tier '{t}' (expected one of: {})",
                        TIERS.join(", ")
                    )));
                }
            }
            let min_trust = match min_trust {
                None => None,
                Some(t) => Some(
                    Trust::parse(&serde_json::Value::String(t.clone())).ok_or_else(|| {
                        MemoryError::ConfigError(format!(
                            "unknown trust '{t}' (expected high, medium or low)"
                        ))
                    })?,
                ),
            };
            let as_of = as_of
                .map(|s| {
                    crate::memory::search::parse_date(&s).ok_or_else(|| {
                        MemoryError::ConfigError(format!(
                            "--as-of '{s}' is not a date (expected YYYY-MM-DD)"
                        ))
                    })
                })
                .transpose()?;
            let filter = SearchFilter {
                tiers,
                path_prefix: path,
                min_trust,
                include_inactive,
                as_of,
            };
            let mode = match mode {
                Mode::Hybrid => SearchMode::Hybrid,
                Mode::Semantic => SearchMode::Semantic,
                Mode::Keyword => SearchMode::Keyword,
            };
            let results = if expand_links {
                engine.search_expanded(&query, limit, mode, &filter).await?
            } else {
                engine
                    .search_with_explain(&query, limit, mode, &filter, explain)
                    .await?
            };
            engine.note_query(&query, &results)?;
            match format {
                OutputFormat::Json => {
                    let hits: Vec<SearchHit> = results
                        .iter()
                        .map(SearchHit::from_result)
                        .collect::<Result<_>>()?;
                    let out = serde_json::to_string_pretty(&hits)
                        .map_err(|e| MemoryError::SerializationError(e.to_string()))?;
                    println!("{out}");
                }
                OutputFormat::Md => {
                    if results.is_empty() {
                        println!("no results");
                    } else {
                        for r in &results {
                            let c = CitationBuilder::new().build(r)?;
                            println!("- {c}");
                        }
                    }
                }
                OutputFormat::Text => {
                    if results.is_empty() {
                        println!("no results");
                    } else {
                        let b = CitationBuilder::new();
                        for (i, r) in results.iter().enumerate() {
                            println!("{}. score {:.3}  {}", i + 1, r.score, b.build(r)?);
                            if explain {
                                if let Some(ex) = &r.explain {
                                    print!("     explain: fused={:.4}", ex.fused.unwrap_or(0.0));
                                    if let Some(v) = ex.fts {
                                        print!(" fts={:.4}", v);
                                    }
                                    if let Some(v) = ex.vector {
                                        print!(" vector={:.4}", v);
                                    }
                                    if let Some(v) = ex.staleness_factor {
                                        print!(" staleness={:.1}", v);
                                    }
                                    println!();
                                }
                            }
                            for line in r.text.lines().take(4) {
                                println!("     {line}");
                            }
                        }
                    }
                }
            }
        }
        Command::Links { note, outgoing } => {
            let links = engine.links(&note, !outgoing).await?;
            if links.is_empty() {
                println!("no links");
            }
            for l in links {
                println!(
                    "{}:{}  [[{}]] -> {}",
                    l.source_path,
                    l.line,
                    l.target,
                    l.resolved_path.as_deref().unwrap_or("(unresolved)")
                );
            }
        }
        Command::Hygiene {
            dir,
            project_roots,
            json,
            strict,
            strict_facts,
        } => {
            let root = dir.unwrap_or_else(|| config.watch_dir.clone());
            if root.is_dir() {
                engine.index_directory(&root).await?; // audit what is on disk now
            }
            let project_roots = parse_project_roots(&project_roots)?;
            let report = engine
                .hygiene(&HygieneOptions {
                    root: root.is_dir().then_some(root),
                    project_roots,
                    ..HygieneOptions::default()
                })
                .await?;
            if json {
                let out = serde_json::to_string_pretty(&report)
                    .map_err(|e| MemoryError::SerializationError(e.to_string()))?;
                println!("{out}");
            } else {
                println!(
                    "{} notes checked, {} findings",
                    report.checked,
                    report.findings.len()
                );
                for f in &report.findings {
                    let at = f.line.map(|l| format!(":{l}")).unwrap_or_default();
                    println!(
                        "  [{}] {:<30} {}{at}  {}",
                        f.label, f.kind, f.path, f.detail
                    );
                }
                for (kind, n) in &report.summary {
                    println!("  {kind}: {n}");
                }
            }
            if hygiene_strict_failure(&report, strict, strict_facts) {
                std::process::exit(1);
            }
        }
        Command::Curate {
            dir,
            project_roots,
            limit,
            json,
        } => {
            if let Some(root) = &dir {
                require_dir(root)?;
            }
            let root = dir.unwrap_or_else(|| config.watch_dir.clone());
            require_dir(&root)?;
            let project_roots = parse_project_roots(&project_roots)?;
            if WriteLock::daemon_is_active(&root) {
                eprintln!("Using the index maintained by the active memory daemon.");
            } else {
                engine.index_directory(&root).await?;
            }
            let report = engine
                .curate(
                    &HygieneOptions {
                        root: Some(root),
                        project_roots,
                        ..HygieneOptions::default()
                    },
                    limit,
                )
                .await?;
            if json {
                let out = serde_json::to_string_pretty(&report)
                    .map_err(|e| MemoryError::SerializationError(e.to_string()))?;
                println!("{out}");
            } else {
                print_curation_report(&report);
            }
        }
        Command::Missing {
            resolve,
            limit,
            json,
            all,
            dir,
        } => {
            if let Some(id) = resolve {
                if engine.resolve_gap(id)? {
                    println!("resolved gap {id}");
                } else {
                    println!("no open gap with id {id}");
                }
                return Ok(());
            }
            if let Some(d) = &dir {
                if !d.is_dir() {
                    return Err(MemoryError::ConfigError(format!(
                        "not a directory: {}",
                        d.display()
                    )));
                }
            }
            let root = dir.unwrap_or_else(|| config.watch_dir.clone());
            if root.is_dir() {
                engine.index_directory(&root).await?;
            }
            let report = engine.missing(root.is_dir().then_some(root), limit).await?;
            if json {
                let out = serde_json::to_string_pretty(&report)
                    .map_err(|e| MemoryError::SerializationError(e.to_string()))?;
                println!("{out}");
            } else if report.query_gaps.is_empty() && report.structural.is_empty() {
                println!("no gaps");
            }
            if !json {
                for g in &report.query_gaps {
                    let score = g
                        .last_top_score
                        .map_or("no hit".to_string(), |s| format!("top {s:.2}"));
                    // One line per gap, whatever the query held.
                    let flat = g.query.split_whitespace().collect::<Vec<_>>().join(" ");
                    let shown: String = flat.chars().take(80).collect();
                    let ellipsis = if flat.chars().count() > 80 { "…" } else { "" };
                    println!(
                        "  #{:<4} pri {:>5.2}  x{:<3} {score:<10} {shown}{ellipsis}",
                        g.id, g.priority, g.misses
                    );
                }
                // Structural gaps can run to hundreds of lines: show a count per kind and a
                // sample of each, unless --all asks for everything.
                let mut shown: std::collections::BTreeMap<&str, usize> = Default::default();
                for f in &report.structural {
                    let n = shown.entry(f.kind.as_str()).or_insert(0);
                    *n += 1;
                    if !all && *n > SAMPLE_PER_KIND {
                        continue;
                    }
                    let at = f.line.map(|l| format!(":{l}")).unwrap_or_default();
                    println!(
                        "  [{}] {:<14} {}{at}  {}",
                        f.label, f.kind, f.path, f.detail
                    );
                }
                let total: usize = report.structural.len();
                if !report.structural.is_empty() {
                    println!("structural gaps: {total}");
                    for (kind, n) in &shown {
                        if !all && *n > SAMPLE_PER_KIND {
                            println!("  {kind}: {n} (showing {SAMPLE_PER_KIND}, use --all)");
                        } else {
                            println!("  {kind}: {n}");
                        }
                    }
                }
            }
        }
        Command::Stats => {
            let s = engine.statistics().await?;
            println!("documents:  {}", s.documents_total);
            println!("chunks:     {}", s.chunks_total);
            println!("db size:    {} bytes", s.db_size_bytes);
            println!("fts size:   {} bytes", s.fts_index_size_bytes);
            println!("hnsw size:  {} bytes", s.hnsw_size_bytes);
            println!(
                "vector search: {}",
                if engine.has_vector_search() {
                    "on"
                } else {
                    "off (keyword-only)"
                }
            );
        }
        Command::Watch { dir } => {
            let root = dir.unwrap_or_else(|| config.watch_dir.clone());
            require_dir(&root)?;
            let s = engine.index_directory(&root).await?;
            println!(
                "initial sync: indexed {} unchanged {} removed {}",
                s.indexed, s.unchanged, s.removed
            );
            let mut watcher = Watcher::new(&root.to_string_lossy()).await?;
            println!("watching {} (Ctrl-C to stop)", root.display());
            loop {
                tokio::select! {
                    _ = tokio::signal::ctrl_c() => break,
                    changes = watcher.changes() => {
                        let events = changes?;
                        if events.is_empty() { break; }
                        let n = engine.apply_events(&root, &events).await;
                        println!("applied {n}/{} file events", events.len());
                    }
                }
            }
        }
        Command::Projects {
            id,
            sub,
            include_inactive,
            format,
        } => {
            let graph = crate::memory::graph::load_graph(&engine.database()).await?;
            match (id, sub) {
                (None, None) => print_projects(&graph, include_inactive, format),
                (None, Some(_)) => {
                    return Err(MemoryError::ConfigError(
                        "a subcommand (decisions/contexts) needs a project id or '*' first, e.g. `memory projects * decisions`".into(),
                    ));
                }
                (Some(id), None) => print_project_detail(&graph, &id, format)?,
                (Some(id), Some(sub)) => print_project_scoped(&graph, &id, sub, format)?,
            }
        }
        Command::Decisions {
            projects,
            status,
            include_inactive,
            limit,
            format,
        } => {
            let graph = crate::memory::graph::load_graph(&engine.database()).await?;
            validate_projects_filter(&graph, &projects)?;
            let filter = crate::memory::graph::ListFilter {
                projects,
                status,
                include_inactive,
                limit,
            };
            print_decision_list(&graph, &filter, format);
        }
        Command::Contexts {
            projects,
            status,
            include_inactive,
            limit,
            format,
        } => {
            let graph = crate::memory::graph::load_graph(&engine.database()).await?;
            validate_projects_filter(&graph, &projects)?;
            let filter = crate::memory::graph::ListFilter {
                projects,
                status,
                include_inactive,
                limit,
            };
            print_context_list(&graph, &filter, format);
        }
        Command::Remember {
            title,
            body,
            folder,
            template,
            values,
            tier,
            confidence,
            note_type,
            id,
            status,
            project,
            scope,
            supersedes,
            json,
        } => {
            let body = match (body, template.is_some()) {
                (Some(body), _) => body,
                (None, true) => String::new(),
                (None, false) => {
                    use std::io::Read;
                    let mut buf = String::new();
                    std::io::stdin().read_to_string(&mut buf).map_err(|e| {
                        MemoryError::IoError {
                            path: "<stdin>".into(),
                            source: e,
                        }
                    })?;
                    buf
                }
            };
            let has_graph_fields = id.is_some()
                || status.is_some()
                || project.is_some()
                || !scope.is_empty()
                || !supersedes.is_empty();
            if template.is_none() && note_type.is_none() && has_graph_fields {
                return Err(MemoryError::ConfigError(
                    "--id/--status/--project/--scope/--supersedes require --type".into(),
                ));
            }
            if let Some(template_name) = template {
                let explicit_folder = folder.is_some();
                let root = engine.watch_root().ok_or_else(|| {
                    MemoryError::ConfigError("no notes root is configured".into())
                })?;
                let (template_folder, markdown, graph) = crate::memory::notes::render_template(
                    root,
                    &template_name,
                    &title,
                    &body,
                    crate::memory::notes::TemplateOverrides {
                        folder: folder.as_deref(),
                        values: &values,
                        id: id.as_deref(),
                        status: status.as_deref(),
                        project: project.as_deref(),
                        scope: &scope,
                        supersedes: &supersedes,
                    },
                )?;
                if let Some(requested_type) = note_type {
                    if graph.as_ref().map(|g| g.note_type.as_str()) != Some(requested_type.as_str())
                    {
                        return Err(MemoryError::ConfigError(format!(
                            "--type '{requested_type}' does not match template '{template_name}'"
                        )));
                    }
                }
                if graph.is_none() && has_graph_fields {
                    return Err(MemoryError::ConfigError(format!(
                        "template '{template_name}' is not a graph type; remove --id/--status/--project/--scope/--supersedes"
                    )));
                }
                let note = crate::memory::notes::NewNote {
                    folder: template_folder,
                    title: if !explicit_folder
                        && graph.as_ref().is_some_and(|g| g.note_type == "project")
                    {
                        "README".into()
                    } else {
                        title
                    },
                    body,
                    tier,
                    confidence,
                    graph,
                };
                let path = engine.remember_rendered(&note, &markdown).await?;
                if json {
                    println!(
                        "{}",
                        serde_json::json!({ "path": path.display().to_string() })
                    );
                } else {
                    println!("saved: {}", path.display());
                }
                return Ok(());
            }
            if !values.is_empty() {
                return Err(MemoryError::ConfigError("--set requires --template".into()));
            }
            let graph = note_type
                .as_ref()
                .map(|note_type| crate::memory::notes::GraphFields {
                    note_type: note_type.clone(),
                    id: id.clone().unwrap_or_default(),
                    status,
                    project,
                    scope,
                    supersedes,
                });
            let note = crate::memory::notes::NewNote {
                folder: folder.unwrap_or_default(),
                title,
                body,
                tier,
                confidence,
                graph,
            };
            let path = engine.remember(&note).await?;
            if json {
                println!(
                    "{}",
                    serde_json::json!({ "path": path.display().to_string() })
                );
            } else {
                println!("saved: {}", path.display());
            }
        }
        Command::Forget { path, reason, yes } => {
            if !yes {
                return Err(MemoryError::ConfigError(
                    "forget permanently removes the source file; rerun with --yes after reviewing the path".into(),
                ));
            }
            let shown = path.display().to_string();
            engine.forget_note(&path, &reason).await?;
            println!("forgotten: {shown}");
        }
        Command::Update {
            path,
            expected_sha256,
            reason,
            yes: _,
        } => {
            use std::io::Read;
            let mut updated = String::new();
            std::io::stdin()
                .read_to_string(&mut updated)
                .map_err(|source| MemoryError::IoError {
                    path: "<stdin>".into(),
                    source,
                })?;
            let hash = engine
                .update_note(&path, &expected_sha256, &updated, &reason)
                .await?;
            println!("updated: {} (sha256 {hash})", path.display());
        }
        Command::Archive {
            path,
            expected_sha256,
            reason,
            yes: _,
        } => {
            let hash = engine
                .archive_note(&path, &expected_sha256, &reason)
                .await?;
            println!("archived: {} (sha256 {hash})", path.display());
        }
    }
    if engine.has_vector_search() {
        engine.save_snapshot().await;
    }
    Ok(())
}

fn parse_project_roots(
    values: &[String],
) -> Result<std::collections::BTreeMap<String, Vec<PathBuf>>> {
    let mut roots = std::collections::BTreeMap::new();
    for value in values {
        let Some((slug, path)) = value.split_once('=') else {
            return Err(MemoryError::ConfigError(format!(
                "invalid --project-root {value:?}; expected SLUG=PATH"
            )));
        };
        let slug = slug.trim();
        let path = path.trim();
        if slug.is_empty()
            || slug == "."
            || slug == ".."
            || slug.contains(['/', '\\'])
            || path.is_empty()
        {
            return Err(MemoryError::ConfigError(format!(
                "invalid --project-root {value:?}; use a project slug and non-empty path"
            )));
        }
        let path = PathBuf::from(path);
        if !path.is_dir() {
            return Err(MemoryError::ConfigError(format!(
                "project root for {slug:?} is not a directory: {}",
                path.display()
            )));
        }
        roots
            .entry(slug.to_owned())
            .or_insert_with(Vec::new)
            .push(path);
    }
    Ok(roots)
}

/// `--project` is a *filter*: an id matching no project yields an empty result (see graph
/// module docs), except `*` which always means "every project" and bypasses this check.
fn validate_projects_filter(
    graph: &crate::memory::graph::Graph,
    projects: &[String],
) -> Result<()> {
    for p in projects {
        if p == "*" {
            continue;
        }
        if !graph.project_ids().contains(&p.as_str()) {
            eprintln!("note: no project matches '{p}'; it will filter out everything for that id");
        }
    }
    Ok(())
}

fn print_projects(
    graph: &crate::memory::graph::Graph,
    include_inactive: bool,
    format: GraphFormat,
) {
    let filter = crate::memory::graph::ListFilter {
        include_inactive,
        limit: usize::MAX,
        ..Default::default()
    };
    let items = crate::memory::graph::list(graph, "project", &filter);
    match format {
        GraphFormat::Json => print_json(&items),
        GraphFormat::Text => {
            if items.is_empty() {
                println!("No projects found. Create one with `type: project` frontmatter.");
                return;
            }
            for p in items {
                let decisions = graph
                    .of_type("decision")
                    .iter()
                    .filter(|d| d.belongs_to(&p.id))
                    .count();
                let contexts = graph
                    .of_type("context")
                    .iter()
                    .filter(|c| c.belongs_to(&p.id))
                    .count();
                println!(
                    "{:<24} {:<10} {:<28} decisions={decisions} contexts={contexts}",
                    p.id,
                    p.effective_status(),
                    p.title
                );
            }
        }
    }
}

fn print_project_detail(
    graph: &crate::memory::graph::Graph,
    id: &str,
    format: GraphFormat,
) -> Result<()> {
    if id == "*" {
        return Err(MemoryError::ConfigError(
            "\"*\" is only valid before a subcommand, e.g. `projects * decisions`. Use `projects` to list all.".into(),
        ));
    }
    use crate::memory::graph::Resolved;
    match crate::memory::graph::resolve_project(graph, id) {
        Resolved::Found(p) => {
            let decisions: Vec<_> = graph
                .of_type("decision")
                .into_iter()
                .filter(|d| d.belongs_to(&p.id))
                .collect();
            let contexts: Vec<_> = graph
                .of_type("context")
                .into_iter()
                .filter(|c| c.belongs_to(&p.id))
                .collect();
            match format {
                GraphFormat::Json => print_json(&serde_json::json!({
                    "id": p.id, "title": p.title, "status": p.effective_status(),
                    "decisions_count": decisions.len(), "contexts_count": contexts.len(),
                })),
                GraphFormat::Text => {
                    println!("{}  ({})", p.title, p.id);
                    println!("  status:    {}", p.effective_status());
                    println!("  decisions: {}", decisions.len());
                    println!("  contexts:  {}", contexts.len());
                }
            }
            Ok(())
        }
        Resolved::Ambiguous { query, candidates } => Err(MemoryError::ConfigError(format!(
            "\"{query}\" matches {} projects: {}. Use the full id.",
            candidates.len(),
            candidates.join(", ")
        ))),
        Resolved::NotFound { query, suggestions } => {
            if suggestions.is_empty() {
                Err(MemoryError::ConfigError(format!(
                    "no project found with id \"{query}\". Run `memory projects` to list all."
                )))
            } else {
                Err(MemoryError::ConfigError(format!(
                    "no project found with id \"{query}\". Did you mean: {}? Run `memory projects` to list all.",
                    suggestions.join(", ")
                )))
            }
        }
    }
}

fn print_project_scoped(
    graph: &crate::memory::graph::Graph,
    id: &str,
    sub: ProjectsSubcommand,
    _format: GraphFormat,
) -> Result<()> {
    use crate::memory::graph::{ListFilter, Resolved};
    let (note_type, include_inactive, status, limit, format) = match sub {
        ProjectsSubcommand::Decisions {
            include_inactive,
            status,
            limit,
            format,
        } => ("decision", include_inactive, status, limit, format),
        ProjectsSubcommand::Contexts {
            include_inactive,
            status,
            limit,
            format,
        } => ("context", include_inactive, status, limit, format),
    };
    let projects = if id == "*" {
        Vec::new() // no restriction: same as the unfiltered global command
    } else {
        match crate::memory::graph::resolve_project(graph, id) {
            Resolved::Found(p) => vec![p.id.clone()],
            Resolved::Ambiguous { query, candidates } => {
                return Err(MemoryError::ConfigError(format!(
                    "\"{query}\" matches {} projects: {}. Use the full id.",
                    candidates.len(),
                    candidates.join(", ")
                )));
            }
            Resolved::NotFound { query, suggestions } => {
                return Err(MemoryError::ConfigError(if suggestions.is_empty() {
                    format!(
                        "no project found with id \"{query}\". Run `memory projects` to list all."
                    )
                } else {
                    format!(
                        "no project found with id \"{query}\". Did you mean: {}? Run `memory projects` to list all.",
                        suggestions.join(", ")
                    )
                }));
            }
        }
    };
    let filter = ListFilter {
        projects,
        status,
        include_inactive,
        limit,
    };
    match note_type {
        "decision" => print_decision_list(graph, &filter, format),
        _ => print_context_list(graph, &filter, format),
    }
    Ok(())
}

fn print_decision_list(
    graph: &crate::memory::graph::Graph,
    filter: &crate::memory::graph::ListFilter,
    format: GraphFormat,
) {
    let items = crate::memory::graph::list(graph, "decision", filter);
    let (conflicts, superseded_by) = crate::memory::graph::supersession_map(graph);
    match format {
        GraphFormat::Json => print_json(&items),
        GraphFormat::Text => {
            if items.is_empty() {
                let scope =
                    if filter.projects.is_empty() || filter.projects.iter().any(|p| p == "*") {
                        "".to_string()
                    } else {
                        format!(" for project \"{}\"", filter.projects.join(", "))
                    };
                println!("No decisions{scope}.");
                return;
            }
            for d in &items {
                let owner = d.project.as_deref().unwrap_or("[global]");
                let shared = if filter.projects.iter().any(|p| d.is_shared_only(p)) {
                    " (shared)"
                } else {
                    ""
                };
                print!("[{owner}]{shared}  {}  {}", d.title, d.effective_status());
                if let Some(by) = superseded_by.get(&d.id) {
                    print!("  superseded by: {}", by.join(", "));
                }
                println!();
            }
            for c in &conflicts {
                eprintln!(
                    "⚠ conflicting supersession: \"{}\" is claimed superseded by {} — run `memory hygiene --strict`",
                    c.target_id,
                    c.claimants.join(", ")
                );
            }
        }
    }
}

fn print_context_list(
    graph: &crate::memory::graph::Graph,
    filter: &crate::memory::graph::ListFilter,
    format: GraphFormat,
) {
    let items = crate::memory::graph::list(graph, "context", filter);
    match format {
        GraphFormat::Json => print_json(&items),
        GraphFormat::Text => {
            if items.is_empty() {
                println!("No contexts.");
                return;
            }
            for c in &items {
                let owner = c.project.as_deref().unwrap_or("[global]");
                println!("[{owner}]  {}  {}", c.title, c.effective_status());
            }
        }
    }
}

fn print_curation_report(report: &CurationReport) {
    println!(
        "Memory curation review: {} notes checked (read-only)",
        report.checked
    );
    println!(
        "Hygiene findings: {} total ({} confirmed, {} inferred)",
        report.hygiene_findings_total, report.confirmed_findings, report.inferred_findings
    );
    for (kind, count) in &report.hygiene.summary {
        println!("  {kind}: {count}");
        for finding in report
            .hygiene
            .findings
            .iter()
            .filter(|finding| &finding.kind == kind)
            .take(3)
        {
            println!("    {} — {}", finding.path, finding.detail);
        }
    }
    if report.hygiene_has_more {
        println!("  more hygiene findings omitted; use `memory hygiene` or increase --limit");
    }

    println!("Unanswered searches: {}", report.query_gaps.items.len());
    for gap in &report.query_gaps.items {
        println!("  #{} ({} misses) {}", gap.id, gap.misses, gap.query);
    }
    if report.query_gaps.has_more {
        println!("  more query gaps omitted; increase --limit to show more");
    }

    println!(
        "Exact duplicate groups: {}",
        report.exact_duplicates.items.len()
    );
    for group in &report.exact_duplicates.items {
        println!("  {}", group.files.items.join(" = "));
        if group.files.has_more {
            println!("    additional duplicate paths omitted");
        }
    }
    if report.exact_duplicates.has_more {
        println!("  more duplicate groups omitted; increase --limit to show more");
    }

    println!(
        "Unlinked note candidates: {}",
        report.unlinked_notes.items.len()
    );
    for path in &report.unlinked_notes.items {
        println!("  {path}");
    }
    if report.unlinked_notes.has_more {
        println!("  more unlinked notes omitted; increase --limit to show more");
    }

    println!(
        "Knowledge folders that may need an index ({} direct notes threshold): {}",
        report.topic_index_threshold,
        report.missing_topic_indexes.items.len()
    );
    for candidate in &report.missing_topic_indexes.items {
        println!("  {} ({} notes)", candidate.folder, candidate.notes);
    }
    if report.missing_topic_indexes.has_more {
        println!("  more folders omitted; increase --limit to show more");
    }
    println!("Review candidates manually; this command does not move, edit, or delete notes.");
}

fn print_json(v: &impl serde::Serialize) {
    match serde_json::to_string_pretty(v) {
        Ok(s) => println!("{s}"),
        Err(e) => eprintln!("error: failed to serialize: {e}"),
    }
}

fn require_dir(path: &std::path::Path) -> Result<()> {
    if path.is_dir() {
        Ok(())
    } else {
        Err(MemoryError::ConfigError(format!(
            "{} is not a directory",
            path.display()
        )))
    }
}

#[cfg(test)]
mod remember_cli_tests {
    use super::*;

    fn report(labels: &[&str]) -> HygieneReport {
        HygieneReport {
            checked: 0,
            findings: labels
                .iter()
                .map(|label| crate::memory::hygiene::Finding {
                    kind: "sample".into(),
                    path: "note.md".into(),
                    line: None,
                    detail: "sample finding".into(),
                    label: (*label).into(),
                })
                .collect(),
            summary: Default::default(),
        }
    }

    #[test]
    fn hygiene_strict_facts_ignores_inferred_findings_but_strict_does_not() {
        let inferred = report(&["INFERRED"]);
        assert!(!hygiene_strict_failure(&inferred, false, true));
        assert!(hygiene_strict_failure(&inferred, true, false));

        let confirmed = report(&["FACT"]);
        assert!(hygiene_strict_failure(&confirmed, false, true));
        assert!(hygiene_strict_failure(&confirmed, true, false));
    }

    #[test]
    fn hygiene_strict_modes_cannot_be_combined() {
        assert!(Cli::try_parse_from([
            "plaesy memory",
            "hygiene",
            "vault",
            "--strict",
            "--strict-facts",
        ])
        .is_err());
    }

    #[test]
    fn template_remember_options_parse() {
        let cli = Cli::try_parse_from([
            "plaesy memory",
            "remember",
            "A title",
            "--template",
            "knowledge",
            "--folder",
            "knowledges/testing",
            "--set",
            "ONE_SENTENCE_SUMMARY=A core claim",
        ])
        .expect("template remember arguments should parse");
        let Command::Remember {
            title,
            template,
            values,
            body,
            ..
        } = cli.command
        else {
            panic!("expected remember subcommand");
        };
        assert_eq!(title, "A title");
        assert_eq!(template.as_deref(), Some("knowledge"));
        assert_eq!(values, ["ONE_SENTENCE_SUMMARY=A core claim"]);
        assert!(body.is_none());
    }

    #[test]
    fn forget_requires_confirmation_and_a_structured_reason() {
        assert!(Cli::try_parse_from([
            "plaesy memory",
            "forget",
            "notes/a.md",
            "--reason",
            "owner-request",
        ])
        .is_err());

        let with_confirmation = Cli::try_parse_from([
            "plaesy memory",
            "forget",
            "notes/a.md",
            "--reason",
            "privacy",
            "--yes",
        ])
        .expect("structured forget command parses");
        assert!(matches!(
            with_confirmation.command,
            Command::Forget { yes: true, .. }
        ));

        assert!(Cli::try_parse_from([
            "plaesy memory",
            "forget",
            "notes/a.md",
            "--reason",
            "free-form secret reason",
            "--yes",
        ])
        .is_err());
    }

    #[test]
    fn update_requires_expected_hash_reason_and_confirmation() {
        let parsed = Cli::try_parse_from([
            "plaesy memory",
            "update",
            "knowledges/a.md",
            "--if-match",
            &"a".repeat(64),
            "--reason",
            "new-evidence",
            "--yes",
        ])
        .expect("well-formed update parses");
        assert!(matches!(parsed.command, Command::Update { yes: true, .. }));

        assert!(Cli::try_parse_from([
            "plaesy memory",
            "update",
            "knowledges/a.md",
            "--if-match",
            &"a".repeat(64),
            "--reason",
            "free-form",
            "--yes",
        ])
        .is_err());
        assert!(Cli::try_parse_from([
            "plaesy memory",
            "update",
            "knowledges/a.md",
            "--if-match",
            &"a".repeat(64),
            "--reason",
            "new-evidence",
        ])
        .is_err());
    }

    #[test]
    fn fingerprint_path_parses() {
        assert!(matches!(
            Cli::try_parse_from(["plaesy memory", "fingerprint", "knowledges/a.md"])
                .expect("fingerprint parses")
                .command,
            Command::Fingerprint { .. }
        ));
    }

    #[test]
    fn archive_requires_expected_hash_reason_and_confirmation() {
        let parsed = Cli::try_parse_from([
            "plaesy memory",
            "archive",
            "knowledges/a.md",
            "--if-match",
            &"b".repeat(64),
            "--reason",
            "completed",
            "--yes",
        ])
        .expect("well-formed archive parses");
        assert!(matches!(parsed.command, Command::Archive { yes: true, .. }));

        assert!(Cli::try_parse_from([
            "plaesy memory",
            "archive",
            "knowledges/a.md",
            "--if-match",
            &"b".repeat(64),
            "--reason",
            "free-form",
            "--yes",
        ])
        .is_err());
        assert!(Cli::try_parse_from([
            "plaesy memory",
            "archive",
            "knowledges/a.md",
            "--if-match",
            &"b".repeat(64),
            "--reason",
            "completed",
        ])
        .is_err());
    }
}
