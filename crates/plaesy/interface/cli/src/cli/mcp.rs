//! `plaesy mcp`: a project-scoped MCP server over stdio for the project's `.plaesy/` folder.

use std::io;
use std::path::PathBuf;

use super::CliResult;
use crate::mcpserver::Server;

/// Stdout carries only protocol messages, so project `.mcp.json` files can start this command.
#[derive(Debug, clap::Args)]
pub struct Args {
    /// Project root that holds `.plaesy/` (default: current directory)
    #[arg(long)]
    pub root: Option<PathBuf>,
}

pub fn run(args: &Args) -> CliResult {
    let root = args.root.clone().unwrap_or_else(|| PathBuf::from("."));
    let server = Server::new(&root, crate::version())?;
    let stdin = io::stdin().lock();
    let stdout = io::stdout().lock();
    server.serve(stdin, stdout)?;
    Ok(())
}
