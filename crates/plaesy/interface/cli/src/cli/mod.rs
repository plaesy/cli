//! The `plaesy` command tree. Port of `scripts/cmd/plaesy` (Go, cobra). Each command group is
//! one module; the logic lives in the library modules, and these files parse arguments and print.

pub mod acp;
pub mod advisor;
pub mod analyze;
pub mod approvals;
pub mod audit;
pub mod chat;
pub mod chat_agents;
pub mod chat_history;
pub mod chat_hooks;
pub mod chat_learn;
pub mod chat_lsp;
pub mod chat_mcp;
pub mod chat_model;
pub mod chat_pending;
pub mod chat_plugins;
pub mod chat_roles;
pub mod chat_schedule;
pub mod chat_subagent;
pub mod chat_tasks;
pub mod chat_text;
pub mod chat_todo;
pub mod chat_tools;
pub mod chat_tui;
pub mod codegen_cmd;
pub mod common;
pub mod dashboard;
pub mod decisions;
pub mod delegate;
pub mod doctor;
pub mod export;
pub mod files;
pub mod images;
pub mod import_cmd;
pub mod init;
pub mod install;
pub mod loop_cmd;
pub mod agent_task;
pub mod gateway;
pub mod mcp;
pub mod memory;
pub mod openapi_cmd;
pub mod output;
pub mod projects;
pub mod reach;
pub mod relay;
pub mod remote;
pub mod sessions;
pub mod skills;
pub mod status;
pub mod tasks;
pub mod tasks_board;
pub mod tasks_comment;
pub mod tasks_create;
pub mod tasks_issue;
pub mod tasks_label;
pub mod tasks_run;
pub mod tasks_validate;
pub mod telemetry;
pub mod trim;
pub mod validate;
pub mod voice;
pub mod workspace;

use clap::{Parser, Subcommand};

/// Errors a command reports. The message is printed as `Error: <message>` and the exit code is 1.
pub type CliResult = Result<(), Box<dyn std::error::Error>>;

/// Plaesy CLI
#[derive(Debug, Parser)]
#[command(name = "plaesy", version = crate::version(), about = "Plaesy CLI for memory and agent workflows", arg_required_else_help = true)]
pub struct Cli {
    /// No color and no decorative glyphs (NO_COLOR is also honored). Place it before the subcommand.
    #[arg(long, global = true)]
    pub plain: bool,
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Read-only model advice for comparing options and triaging tasks
    Advisor(advisor::Args),
    /// Approve, revoke, or check persisted approvals for external actions
    Approvals(approvals::Args),
    /// Typed, structured decisions using locally managed decision models
    Decisions(decisions::Args),
    /// Show Plaesy CLI version and environment status
    Status,
    /// Write a static offline HTML dashboard of task counts and delegate usage
    Dashboard(dashboard::Args),
    /// Initialize project-local Plaesy metadata safely
    Init(init::Args),
    /// Serve scoped list, read, and write tools for this project's .plaesy/ folder over MCP
    Mcp(mcp::Args),
    /// Read-only HTTP status and task API (/health, /tasks, /events), loopback by default
    Remote(remote::Args),
    /// Local message gateway: poll the relay for one agent and print each new message.
    Gateway(gateway::Args),
    /// Install the plaesy CLI to a well-known bin directory
    Install,
    /// Remove the installed plaesy binary and PATH entry while preserving user data
    Uninstall,
    /// Restore the installed binary from the running build
    Repair,
    /// Replace the installed binary with the running (newer) build
    Upgrade,
    /// Print the plaesy version and the running and installed binary paths
    Version,
    /// Compare the installed version with a local manifest (--check); no network access
    Update(install::UpdateArgs),
    /// Analyze a project: documentation, dependency graph, and symbols
    Analyze(analyze::Args),
    /// Run iterations until a Stop or Completion condition holds
    Loop(loop_cmd::Args),
    /// Access external data sources across multiple platforms
    Reach(reach::Args),
    /// Send a prompt to a configured model provider or chain
    Delegate(delegate::Args),
    /// Generate code from a flow-ir/1 document (js or go)
    Codegen(codegen_cmd::Args),
    /// Import a SQLite schema or Rust/Python source as an IR document
    Import(import_cmd::Args),
    /// Generate an OpenAPI 3.1 document from an api-ir/1 document
    Openapi(openapi_cmd::Args),
    /// Serve the ACP v1 agent over stdio, answering prompts with a configured provider
    Acp(acp::Args),
    /// Check health of reach platforms and delegate providers
    Doctor(doctor::Args),
    /// Token/context compression for command output, memory and instruction files
    Trim(trim::Args),
    /// List and copy skills between agent providers (claude, codex, agents, hermes)
    Skills(skills::Args),
    /// Interactive multi-turn chat with a configured provider (/provider, /clear, /exit)
    Chat(chat::Args),
    /// Install and operate OS timers for persisted chat goals
    ChatSchedule(chat_schedule::Args),
    /// Generated image assets through a provider API
    Images(images::Args),
    /// Durable, acknowledged direct messaging between agents
    Relay(relay::Args),
    /// Read the audit trail of authorization decisions
    Audit(audit::Args),
    /// Manage local advisory claims on project files
    Files(files::Args),
    /// Register and discover local Plaesy projects
    Projects(projects::Args),
    /// Group registered projects into named workspaces
    Workspace(workspace::Args),
    /// Report and inspect self-declared agent session state
    Sessions(sessions::Args),
    /// Manage the local Markdown memory index and notes
    Memory(memory::Args),
    /// First-run setup: vault folder, settings.json, model download, first index
    Onboard(memory::OnboardArgs),
    /// Task lifecycle under .plaesy/tasks/ (backlog/todo/doing/done/blocked)
    Tasks(tasks::Args),
    /// Export the task list or the delegate run summary as Markdown or CSV
    Export(export::Args),
    /// Validate a constitution, memory, Markdown, local links, or an OOXML document
    Validate(validate::Args),
    /// Local, opt-in telemetry summary (stored in .plaesy/telemetry.jsonl, never sent)
    Telemetry(telemetry::Args),
    /// Voice commands: parse transcribed text into a task command without running it
    Voice(voice::Args),
}

impl Command {
    /// The name a run is recorded under in local telemetry, or `None` when the run is not
    /// recorded. The name is the command group only, never its arguments.
    #[must_use]
    pub fn telemetry_name(&self) -> Option<&'static str> {
        Some(match self {
            Self::Advisor(_) => "advisor",
            Self::Decisions(_) => "decisions",
            Self::Status => "status",
            Self::Init(_) => "init",
            Self::Mcp(_) => "mcp",
            Self::Install => "install",
            Self::Uninstall => "uninstall",
            Self::Repair => "repair",
            Self::Upgrade => "upgrade",
            Self::Analyze(_) => "analyze",
            Self::Loop(_) => "loop",
            Self::Reach(_) => "reach",
            Self::Delegate(_) => "delegate",
            Self::Doctor(_) => "doctor",
            Self::Trim(_) => "trim",
            Self::Skills(_) => "skills",
            Self::Chat(_) => "chat",
            Self::ChatSchedule(_) => "chat-schedule",
            Self::Images(_) => "images",
            Self::Relay(_) => "relay",
            Self::Files(_) => "files",
            Self::Projects(_) => "projects",
            Self::Sessions(_) => "sessions",
            Self::Memory(_) => "memory",
            Self::Tasks(_) => "tasks",
            Self::Validate(_) => "validate",
            Self::Approvals(_) => "approvals",
            Self::Dashboard(_) => "dashboard",
            Self::Version => "version",
            Self::Update(_) => "update",
            Self::Codegen(_) => "codegen",
            Self::Import(_) => "import",
            Self::Openapi(_) => "openapi",
            Self::Acp(_) => "acp",
            Self::Audit(_) => "audit",
            Self::Workspace(_) => "workspace",
            Self::Onboard(_) => "onboard",
            Self::Export(_) => "export",
            Self::Voice(_) => "voice",
            Self::Remote(_) => "remote",
            Self::Gateway(_) => "gateway",
            // Reading the telemetry summary is not itself recorded.
            Self::Telemetry(_) => return None,
        })
    }
}

/// Prints the help of one command group, for a group run with no subcommand.
///
/// # Errors
///
/// Printing the help fails.
pub fn print_group_help(name: &str) -> CliResult {
    use clap::CommandFactory;
    let mut cmd = Cli::command();
    let group = cmd
        .find_subcommand_mut(name)
        .ok_or_else(|| format!("{name} command missing"))?;
    group.print_help()?;
    println!();
    Ok(())
}

/// Runs the parsed command.
///
/// # Errors
///
/// The command fails. The caller prints the error and exits with status 1.
pub fn run(cli: Cli) -> CliResult {
    let name = cli.command.telemetry_name();
    let no_color = std::env::var_os("NO_COLOR");
    let style = output::Style::resolve(cli.plain, no_color.as_deref());
    crate::common::log::set_color_enabled(style.color);
    let started = std::time::Instant::now();
    let result = dispatch(cli.command);
    if let Some(name) = name {
        record_telemetry(name, result.is_ok(), started.elapsed());
    }
    result
}

/// Records one run when telemetry is opted in. Telemetry is best effort: a failure is
/// reported on stderr and never changes the command's result or its stdout.
fn record_telemetry(name: &str, ok: bool, elapsed: std::time::Duration) {
    let Ok(root) = common::find_root() else {
        return;
    };
    if let Err(e) = crate::telemetry::record(&root, name, ok, elapsed) {
        eprintln!("Warning: telemetry not recorded: {e}");
    }
}

fn dispatch(command: Command) -> CliResult {
    match command {
        Command::Advisor(args) => advisor::run(&args),
        Command::Decisions(args) => decisions::run(&args),
        Command::Status => status::run(),
        Command::Dashboard(args) => dashboard::run(&args),
        Command::Init(args) => init::run(&args),
        Command::Mcp(args) => mcp::run(&args),
        Command::Remote(args) => remote::run(&args),
        Command::Gateway(args) => gateway::run(&args),
        Command::Install => install::install(),
        Command::Uninstall => install::uninstall(),
        Command::Repair => install::repair(),
        Command::Upgrade => install::upgrade(),
        Command::Version => install::version(),
        Command::Update(args) => install::update(&args),
        Command::Analyze(args) => analyze::run(&args),
        Command::Loop(args) => loop_cmd::run(&args),
        Command::Reach(args) => reach::run(&args),
        Command::Delegate(args) => delegate::run(&args),
        Command::Codegen(args) => codegen_cmd::run(&args),
        Command::Import(args) => import_cmd::run(&args),
        Command::Openapi(args) => openapi_cmd::run(&args),
        Command::Acp(args) => acp::run(&args),
        Command::Doctor(args) => doctor::run(&args),
        Command::Trim(args) => trim::run(&args),
        Command::Skills(args) => skills::run(&args),
        Command::Chat(args) => chat::run(&args),
        Command::ChatSchedule(args) => chat_schedule::run(&args),
        Command::Images(args) => images::run(&args),
        Command::Relay(args) => relay::run(&args),
        Command::Audit(args) => audit::run(&args),
        Command::Files(args) => files::run(&args),
        Command::Projects(args) => projects::run(&args),
        Command::Approvals(args) => approvals::run(&args),
        Command::Workspace(args) => workspace::run(&args),
        Command::Sessions(args) => sessions::run(&args),
        Command::Memory(args) => memory::run(&args),
        Command::Onboard(args) => memory::run_onboard(&args),
        Command::Tasks(args) => tasks::run(&args),
        Command::Export(args) => export::run(&args),
        Command::Validate(args) => validate::run(&args),
        Command::Telemetry(args) => telemetry::run(&args),
        Command::Voice(args) => voice::run(&args),
    }
}
