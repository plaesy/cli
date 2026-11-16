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
    let root = find_root()?;
    match command {
        Command::List { all } => {
            let list = crate::sessions::list(&root, *all)?;
            if list.is_empty() {
                println!("No {}session records.", if *all { "" } else { "active " });
            }
            for (s, stale) in list {
                println!(
                    "{} [{}] agent={} state={} task={} files={} updated={} expires={}",
                    s.id,
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
            let s = crate::sessions::report(&root, id, agent, state, task.as_deref(), files, *ttl)?;
            println!(
                "Reported session {}: {} (expires {})",
                s.id, s.state, s.expires_at
            );
        }
        Command::Heartbeat { id, ttl } => {
            let s = crate::sessions::heartbeat(&root, id, *ttl)?;
            println!(
                "Heartbeat refreshed for {} (expires {})",
                s.id, s.expires_at
            );
        }
        Command::Stop { id } => {
            crate::sessions::stop(&root, id)?;
            println!("Stopped session record {id}");
        }
    }
    Ok(())
}
