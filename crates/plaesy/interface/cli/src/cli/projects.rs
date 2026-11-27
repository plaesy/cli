//! `plaesy projects`: explicit registration for local project discovery, and per-project settings.
use super::{common::find_root, CliResult};
use clap::{Args as ClapArgs, Subcommand};
use std::{error::Error, fs, path::PathBuf};

/// Environment variable that selects the project when `--project` is absent.
const PROJECT_ENV: &str = "PLAESY_PROJECT";

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
    /// Show one registered project and its settings.
    Get {
        /// Project ID or registered path. Defaults to PLAESY_PROJECT, then the project above the working directory.
        #[arg(long, short)]
        project: Option<String>,
    },
    /// Set one setting of a project; omit VALUE to remove the setting.
    SetConfig {
        /// Project ID or registered path. Defaults to PLAESY_PROJECT, then the project above the working directory.
        #[arg(long, short)]
        project: Option<String>,
        /// Setting name: letters, digits, '.', '-' or '_'.
        key: String,
        /// New value; omit to remove the setting.
        value: Option<String>,
    },
    /// Unregister a project and drop its settings. Its files are not touched.
    Rm {
        /// Project ID or registered path. Defaults to PLAESY_PROJECT, then the project above the working directory.
        #[arg(long, short)]
        project: Option<String>,
    },
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
        Command::Get { project } => {
            let selector = project_selector(project.as_deref())?;
            let (project, settings) = crate::projects::settings(&selector)?;
            println!("Project {}", project.id);
            println!("Path {}", project.path.display());
            if settings.is_empty() {
                println!("Settings: none");
            }
            for (key, value) in settings {
                println!("{key}={value}");
            }
        }
        Command::SetConfig {
            project,
            key,
            value,
        } => {
            let selector = project_selector(project.as_deref())?;
            let project = crate::projects::set_setting(&selector, key, value.as_deref())?;
            match value {
                Some(_) => println!("Set {key} on project {}.", project.id),
                None => println!("Removed {key} from project {}.", project.id),
            }
        }
        Command::Rm { project } => {
            let selector = project_selector(project.as_deref())?;
            let project = crate::projects::remove(&selector)?;
            println!(
                "Removed project {} from the registry. Files at {} are unchanged.",
                project.id,
                project.path.display()
            );
        }
    }
    Ok(())
}

/// The selector a command acts on: `--project`, then `PLAESY_PROJECT`, then the project above
/// the working directory (as its canonical path).
fn project_selector(flag: Option<&str>) -> Result<String, Box<dyn Error>> {
    let env = std::env::var(PROJECT_ENV).ok();
    if let Some(selector) = pick_selector(flag, env.as_deref()) {
        return Ok(selector);
    }
    let root = fs::canonicalize(find_root()?)?;
    Ok(root.to_string_lossy().into_owned())
}

/// The first non-blank of `flag` and `env`, trimmed. Blank values count as absent.
fn pick_selector(flag: Option<&str>, env: Option<&str>) -> Option<String> {
    [flag, env]
        .into_iter()
        .flatten()
        .map(str::trim)
        .find(|value| !value.is_empty())
        .map(str::to_owned)
}

#[cfg(test)]
#[path = "projects_test.rs"]
mod projects_test;
