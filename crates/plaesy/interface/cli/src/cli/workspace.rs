//! `plaesy workspace`: named groups of registered Plaesy projects.
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
    /// Create an empty workspace group.
    Create { name: String },
    /// Delete a workspace group; its projects stay registered.
    Delete { name: String },
    /// Add a Plaesy project root to a group, registering it first (default: current project).
    Add { name: String, path: Option<PathBuf> },
    /// Remove a project from a group by its ID or path.
    Remove { name: String, selector: String },
    /// List groups, or the projects of one group. Missing roots are marked `missing`.
    List { name: Option<String> },
}
pub fn run(args: &Args) -> CliResult {
    let Some(command) = &args.command else {
        return super::print_group_help("workspace");
    };
    match command {
        Command::Create { name } => {
            crate::workspaces::create(name)?;
            println!("Created workspace {name}");
        }
        Command::Delete { name } => {
            crate::workspaces::delete(name)?;
            println!("Deleted workspace {name}");
        }
        Command::Add { name, path } => {
            let cwd;
            let path = if let Some(path) = path {
                path.as_path()
            } else {
                cwd = find_root()?;
                cwd.as_path()
            };
            let (project, added) = crate::workspaces::add(name, path)?;
            if added {
                println!("Added {} to workspace {name}", project.id);
            } else {
                println!("{} is already in workspace {name}", project.id);
            }
        }
        Command::Remove { name, selector } => {
            let project = crate::workspaces::remove(name, selector)?;
            println!("Removed {} from workspace {name}", project.id);
        }
        Command::List { name: None } => {
            let groups = crate::workspaces::list()?;
            if groups.is_empty() {
                println!("No workspaces.");
            }
            for group in groups {
                println!("{} {}", group.name, group.members.len());
            }
        }
        Command::List { name: Some(name) } => {
            let members = crate::workspaces::members(name)?;
            if members.is_empty() {
                println!("Workspace {name} has no projects.");
            }
            for member in members {
                let state = if member.available { "ok" } else { "missing" };
                println!(
                    "{} {state} {}",
                    member.project.id,
                    member.project.path.display()
                );
            }
        }
    }
    Ok(())
}
