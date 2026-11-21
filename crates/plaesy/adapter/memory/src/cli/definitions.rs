//! Clap argument and subcommand definitions for `plaesy memory`.

use crate::memory::hygiene::HygieneReport;
use crate::memory::serve::Transport;
use clap::{Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "plaesy memory", version, about = "Offline-first memory index")]
pub(super) struct Cli {
    /// Config file (TOML) layered over defaults instead of the standard lookup.
    #[arg(long, global = true)]
    pub(super) config: Option<PathBuf>,

    /// SQLite database path (overrides config).
    #[arg(long, global = true)]
    pub(super) db: Option<PathBuf>,

    #[command(subcommand)]
    pub(super) command: Command,
}

pub(super) fn hygiene_strict_failure(
    report: &HygieneReport,
    strict: bool,
    strict_facts: bool,
) -> bool {
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
pub(super) enum Mode {
    Hybrid,
    Semantic,
    Keyword,
}

/// Output format for search results.
#[derive(Clone, Copy, Default, ValueEnum)]
pub(super) enum OutputFormat {
    /// Plain text listing.
    #[default]
    Text,
    /// JSON array of hits.
    Json,
    /// Markdown list of hits.
    Md,
}

/// Structural gaps shown per kind by `memory missing` unless `--all` is given.
pub(super) const SAMPLE_PER_KIND: usize = 5;

#[derive(Subcommand)]
pub(super) enum Command {
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
    /// Show a paginated inventory of indexed notes (without note bodies).
    List {
        /// Skip this many matching notes.
        #[arg(long, default_value_t = 0)]
        offset: usize,
        /// Maximum notes to return.
        #[arg(short = 'n', long, default_value_t = 50)]
        limit: usize,
        /// Match declared note type (e.g. decision, context, project).
        #[arg(long = "type")]
        note_type: Option<String>,
        /// Match declared lifecycle status (e.g. active, archived, superseded).
        #[arg(long)]
        status: Option<String>,
        /// Case-insensitive substring filter over note path and body (for browsing).
        #[arg(long)]
        query: Option<String>,
        /// Only notes whose path starts with this prefix.
        #[arg(long)]
        path: Option<String>,
        /// Output format.
        #[arg(long, value_enum, default_value_t = GraphFormat::Text)]
        format: GraphFormat,
    },
    /// Show recent hash-only lifecycle events (update, archive, forget).
    History {
        /// Maximum events to return (1–1000).
        #[arg(short = 'n', long, default_value_t = 50)]
        limit: usize,
        /// Filter by lifecycle operation.
        #[arg(long, value_parser = ["update", "archive", "forget"])]
        operation: Option<String>,
        /// Emit JSON.
        #[arg(long)]
        json: bool,
    },
    /// Show index statistics.
    Stats,
    /// Watch a directory and keep the index current until Ctrl-C.
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
        /// Require this bearer token for MCP HTTP requests. Alternatively set PLAESY_MCP_TOKEN.
        #[arg(long)]
        auth_token: Option<String>,
        /// Serve the existing index without taking the daemon lock or syncing/watching the vault.
        /// Lets a client read a vault that another daemon already owns.
        #[arg(long)]
        read_only: bool,
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
pub(super) enum GraphFormat {
    #[default]
    Text,
    Json,
}

/// Subcommands of `memory projects <id|*>`.
#[derive(Subcommand, Clone)]
pub(super) enum ProjectsSubcommand {
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
pub(super) enum ConfigSubcommand {
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
pub(super) enum ScheduleMode {
    Once,
    Run,
    Install,
    Uninstall,
}
