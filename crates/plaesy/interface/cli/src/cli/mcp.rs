//! Project-scoped MCP over stdio and shared memory MCP lifecycle commands.

use std::io;
use std::path::PathBuf;

use super::{mcp_service, memory, CliResult};
use crate::mcpserver::Server;

/// Without a subcommand, keep the original project-scoped stdio behavior.
#[derive(Debug, clap::Args)]
pub struct Args {
    /// Project root that holds `.plaesy/` (legacy form; default: current directory)
    #[arg(long)]
    pub root: Option<PathBuf>,
    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Debug, clap::Subcommand)]
pub enum Command {
    /// Serve one project's `.plaesy/` folder over stdio
    Project {
        /// Project root (default: current directory)
        #[arg(long)]
        root: Option<PathBuf>,
    },
    /// Serve or manage the shared memory MCP server
    Memory {
        #[command(subcommand)]
        command: MemoryCommand,
    },
}

#[derive(Debug, clap::Subcommand)]
pub enum MemoryCommand {
    /// Run the shared memory MCP HTTP server in the foreground
    Serve {
        /// Shared vault directory
        #[arg(long)]
        vault: PathBuf,
        /// Loopback HTTP port
        #[arg(long, default_value_t = 9123)]
        port: u16,
    },
    /// Register autostart and launch the shared memory MCP server in the background
    Start {
        /// Shared vault directory (required on first start)
        #[arg(long)]
        vault: Option<PathBuf>,
        /// Loopback HTTP port (default: 9123 on first start)
        #[arg(long)]
        port: Option<u16>,
    },
    /// Stop the background server; keep its autostart registration
    Stop,
    /// Show the background server registration and listener state
    Status,
    /// Stop the server and remove its autostart registration
    Uninstall,
}

fn serve_project(root: PathBuf) -> CliResult {
    let server = Server::new(&root, crate::version())?;
    server.serve(io::stdin().lock(), io::stdout().lock())?;
    Ok(())
}

pub fn run(args: &Args) -> CliResult {
    match &args.command {
        None => serve_project(args.root.clone().unwrap_or_else(|| PathBuf::from("."))),
        Some(Command::Project { root }) => {
            if args.root.is_some() {
                return Err("use --root either before or after `project`, not both".into());
            }
            serve_project(root.clone().unwrap_or_else(|| PathBuf::from(".")))
        }
        Some(Command::Memory { command }) => {
            if args.root.is_some() {
                return Err("--root only applies to project MCP".into());
            }
            match command {
                MemoryCommand::Serve { vault, port } => memory::run(&memory::Args {
                    vault: Some(vault.clone()),
                    project: false,
                    args: vec![
                        "serve".into(),
                        "--mcp".into(),
                        "--transport".into(),
                        "http".into(),
                        "--port".into(),
                        port.to_string(),
                    ],
                }),
                MemoryCommand::Start { vault, port } => {
                    mcp_service::start(vault.as_deref(), *port)
                }
                MemoryCommand::Stop => mcp_service::stop(),
                MemoryCommand::Status => mcp_service::status(),
                MemoryCommand::Uninstall => mcp_service::uninstall(),
            }
        }
    }
}
