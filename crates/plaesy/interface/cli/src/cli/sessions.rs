//! `plaesy sessions`: project-local self-reported session status.
use super::{common::find_root, CliResult};
use clap::{Args as ClapArgs, Subcommand};
use std::{collections::HashMap, path::PathBuf};

#[derive(Debug, ClapArgs)]
pub struct Args {
    #[command(subcommand)]
    pub command: Option<Command>,
}
#[derive(Debug, Subcommand)]
pub enum Command {
    /// List live sessions (use --all to include stale records)
    List {
        #[arg(long)]
        all: bool,
        /// Limit discovery to a registered project ID or path.
        #[arg(long)]
        project: Option<String>,
    },
    /// Publish or update this session's declared status
    Status {
        #[arg(long)]
        id: String,
        #[arg(long)]
        agent: String,
        #[arg(long, default_value = "working")]
        state: String,
        #[arg(long)]
        task: Option<String>,
        #[arg(long = "file")]
        files: Vec<String>,
        #[arg(long, default_value_t = 1800)]
        ttl: u64,
    },
    /// Refresh liveness for a previously reported session
    Heartbeat {
        #[arg(long)]
        id: String,
        #[arg(long, default_value_t = 1800)]
        ttl: u64,
    },
    /// Remove this session's status record
    Stop {
        #[arg(long)]
        id: String,
    },
}
pub fn run(args: &Args) -> CliResult {
    let Some(command) = &args.command else {
        return super::print_group_help("sessions");
    };
    match command {
        Command::List { all, project } => {
            let list = crate::sessions::list_global(project.as_deref(), *all)?;
            // (project id, project root, session, stale)
            let mut list: Vec<(String, PathBuf, crate::sessions::Session, bool)> = list
                .into_iter()
                .map(|(project, session, stale)| (project.id, project.path, session, stale))
                .collect();
            if project.is_none() {
                if let Ok(root) = find_root() {
                    if !crate::projects::list()?.iter().any(|p| p.path == root) {
                        list.extend(crate::sessions::list(&root, *all)?.into_iter().map(
                            |(session, stale)| ("current".to_owned(), root.clone(), session, stale),
                        ));
                    }
                }
            }
            list.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.2.id.cmp(&b.2.id)));
            if list.is_empty() {
                println!("No {}session records.", if *all { "" } else { "active " });
            }
            // Each project root is asked for its branch once, however many sessions it has.
            let mut branches: HashMap<PathBuf, Option<String>> = HashMap::new();
            for (project_id, path, s, stale) in list {
                let branch = branches
                    .entry(path.clone())
                    .or_insert_with(|| crate::sessions::branch(&path))
                    .clone();
                let age = crate::sessions::age_secs(&s)
                    .map_or_else(|| "-".to_owned(), crate::sessions::format_age);
                println!(
                    "{} project={} [{}] agent={} state={} task={} files={} branch={} age={} updated={} expires={}",
                    s.id,
                    project_id,
                    if stale { "stale" } else { "active" },
                    s.agent,
                    s.state,
                    s.task.as_deref().unwrap_or("-"),
                    if s.files.is_empty() {
                        "-".into()
                    } else {
                        s.files.join(",")
                    },
                    branch.as_deref().unwrap_or("-"),
                    age,
                    s.updated_at,
                    s.expires_at
                );
            }
            println!("Session state is self-reported; stale means heartbeat TTL expired. It does not prove activity or fence writes.");
        }
        Command::Status {
            id,
            agent,
            state,
            task,
            files,
            ttl,
        } => {
            let root = find_root()?;
            let s = crate::sessions::report(&root, id, agent, state, task.as_deref(), files, *ttl)?;
            println!(
                "Reported session {}: {} (expires {})",
                s.id, s.state, s.expires_at
            );
        }
        Command::Heartbeat { id, ttl } => {
            let root = find_root()?;
            let s = crate::sessions::heartbeat(&root, id, *ttl)?;
            println!(
                "Heartbeat refreshed for {} (expires {})",
                s.id, s.expires_at
            );
        }
        Command::Stop { id } => {
            let root = find_root()?;
            crate::sessions::stop(&root, id)?;
            println!("Stopped session record {id}");
        }
    }
    Ok(())
}
