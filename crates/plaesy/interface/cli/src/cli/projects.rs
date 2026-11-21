//! `plaesy projects`: explicit registration for local project discovery.
use super::{common::find_root, CliResult};
use clap::{Args as ClapArgs, Subcommand};
use std::path::PathBuf;

#[derive(Debug, ClapArgs)]
pub struct Args {
    #[command(subcommand)]
    pub command: Option<Command>,
}
#[derive(Debug, Subcommand)]
pub enum Command {
    /// Register a Plaesy project root (must contain .plaesy/).
    Register { path: Option<PathBuf> },
    /// List registered projects that still exist.
    List,
}
pub fn run(args: &Args) -> CliResult {
    let Some(command) = &args.command else {
        return super::print_group_help("projects");
    };
    match command {
        Command::Register { path } => {
            let cwd;
            let path = if let Some(path) = path {
                path.as_path()
            } else {
                cwd = find_root()?;
                cwd.as_path()
            };
            let project = crate::projects::register(path)?;
            println!(
                "Registered project {} at {}",
                project.id,
                project.path.display()
            );
        }
        Command::List => {
            let projects = crate::projects::list()?;
            if projects.is_empty() {
                println!("No registered projects.");
            }
            for project in projects {
                println!("{} {}", project.id, project.path.display());
            }
        }
    }
    Ok(())
}
