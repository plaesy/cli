//! `plaesy sessions`: project-local self-reported session status.
use super::{common::find_root, CliResult};
use clap::{Args as ClapArgs, Subcommand};

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
            let mut list: Vec<_> = list
                .into_iter()
                .map(|(project, session, stale)| (project.id, session, stale))
                .collect();
            if project.is_none() {
                if let Ok(root) = find_root() {
                    if !crate::projects::list()?.iter().any(|p| p.path == root) {
                        list.extend(
                            crate::sessions::list(&root, *all)?
                                .into_iter()
                                .map(|(session, stale)| ("current".to_owned(), session, stale)),
                        );
                    }
                }
            }
            list.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.id.cmp(&b.1.id)));
            if list.is_empty() {
                println!("No {}session records.", if *all { "" } else { "active " });
            }
            for (project_id, s, stale) in list {
                println!(
                    "{} project={} [{}] agent={} state={} task={} files={} updated={} expires={}",
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
