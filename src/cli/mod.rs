//! The `plaesy` command tree. Port of `scripts/cmd/plaesy` (Go, cobra). Each command group is
//! one module; the logic lives in the library modules, and these files parse arguments and print.

pub mod analyze;
pub mod clean;
pub mod common;
pub mod config;
pub mod delegate;
pub mod doctor;
pub mod images;
pub mod memory;
pub mod init;
pub mod install;
pub mod loop_cmd;
pub mod mcp;
pub mod platforms;
pub mod reach;
pub mod reload;
pub mod stack;
pub mod status;
pub mod tasks;
pub mod tasks_create;
pub mod tasks_run;
pub mod tasks_validate;
pub mod trim;
pub mod validate;

use clap::{Parser, Subcommand};

/// Errors a command reports. The message is printed as `Error: <message>` and the exit code is 1.
pub type CliResult = Result<(), Box<dyn std::error::Error>>;

/// Plaesy Constitution Kit CLI
#[derive(Debug, Parser)]
#[command(name = "plaesy", version = crate::version(), about = "Plaesy Constitution Kit CLI", arg_required_else_help = true)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Show Plaesy CLI version and environment status
    Status,
    /// Scaffold a new Plaesy project (.plaesy/ structure + AI platform files)
    Init(init::Args),
    /// Refresh generated .plaesy/ files, keeping memory and context
    Reload(reload::Args),
    /// Serve this project's .plaesy/ folder over MCP (stdio, read-only)
    Mcp(mcp::Args),
    /// Install the plaesy CLI to a well-known bin directory
    Install,
    /// Remove the installed plaesy binary and its Plaesy home
    Uninstall,
    /// Restore the installed binary and bundled assets from the running build
    Repair,
    /// Replace the installed binary with the running (newer) build
    Upgrade,
    /// Remove Plaesy Constitution Kit framework files and directories
    Clean(clean::Args),
    /// AI platforms plaesy can target (claude, cursor_ai, ...)
    Platforms(platforms::Args),
    /// Inspect a target project's technology stack
    Stack(stack::Args),
    /// Read and validate configs/platform.json
    Config(config::Args),
    /// Analyze a project: documentation, dependency graph, and symbols
    Analyze(analyze::Args),
    /// Run iterations until a Stop or Completion condition holds
    Loop(loop_cmd::Args),
    /// Access external data sources across multiple platforms
    Reach(reach::Args),
    /// Send a prompt to a configured model provider or chain
    Delegate(delegate::Args),
    /// Check health of reach platforms and delegate providers
    Doctor(doctor::Args),
    /// Token/context compression for command output, memory and instruction files
    Trim(trim::Args),
    /// Generated image assets through a provider API
    Images(images::Args),
    /// Offline memory index (runs the `memory` binary from the memory submodule)
    Memory(memory::Args),
    /// Task lifecycle under .plaesy/tasks/ (backlog/todo/doing/done/blocked)
    Tasks(tasks::Args),
    /// Validate a constitution, memory, Markdown, or an OOXML document
    Validate(validate::Args),
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
    match cli.command {
        Command::Status => status::run(),
        Command::Init(args) => init::run(&args),
        Command::Reload(args) => reload::run(&args),
        Command::Mcp(args) => mcp::run(&args),
        Command::Install => install::install(),
        Command::Uninstall => install::uninstall(),
        Command::Repair => install::repair(),
        Command::Upgrade => install::upgrade(),
        Command::Clean(args) => clean::run(&args),
        Command::Platforms(args) => platforms::run(&args),
        Command::Stack(args) => stack::run(&args),
        Command::Config(args) => config::run(&args),
        Command::Analyze(args) => analyze::run(&args),
        Command::Loop(args) => loop_cmd::run(&args),
        Command::Reach(args) => reach::run(&args),
        Command::Delegate(args) => delegate::run(&args),
        Command::Doctor(args) => doctor::run(&args),
        Command::Trim(args) => trim::run(&args),
        Command::Images(args) => images::run(&args),
        Command::Memory(args) => memory::run(&args),
        Command::Tasks(args) => tasks::run(&args),
        Command::Validate(args) => validate::run(&args),
    }
}
