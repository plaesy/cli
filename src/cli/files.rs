//! `plaesy files`: local advisory file-ownership claims.
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
    /// Declare that an agent is working on a project-relative file (advisory only)
    Claim {
        path: String,
        #[arg(long)]
        agent: String,
        #[arg(long)]
        task: Option<String>,
        #[arg(long, default_value_t = 1800)]
        ttl: u64,
    },
    /// Show declared active holders of a file (not OS processes)
    Who { path: String },
    /// List declared active file claims
    List,
    /// Release your matching claim
    Release {
        path: String,
        #[arg(long)]
        agent: String,
        #[arg(long)]
        lease: String,
    },
}

pub fn run(args: &Args) -> CliResult {
    let Some(command) = &args.command else {
        return super::print_group_help("files");
    };
    let root: PathBuf = find_root()?;
    match command {
        Command::Claim {
            path,
            agent,
            task,
            ttl,
        } => {
            let c = crate::files::claim(&root, path, agent, task.as_deref(), *ttl)?;
            println!(
                "Advisory claim: {} — agent={} lease={} expires={}{}",
                c.path,
                c.agent,
                c.lease_id,
                c.expires_at,
                c.task
                    .as_ref()
                    .map(|t| format!(" task={t}"))
                    .unwrap_or_default()
            );
            println!(
                "This is a coordination notice only; it does not prevent other processes from editing the file."
            );
        }
        Command::Who { path } => {
            let claims = crate::files::list(&root, Some(path))?;
            if claims.is_empty() {
                println!("No active advisory claim for {path}");
            }
            for c in claims {
                println!(
                    "{} — agent={} lease={} expires={}{}",
                    c.path,
                    c.agent,
                    c.lease_id,
                    c.expires_at,
                    c.task.map(|t| format!(" task={t}")).unwrap_or_default()
                );
            }
            println!("Claims are self-declared labels, not OS process or session identity.");
        }
        Command::List => {
            for c in crate::files::list(&root, None)? {
                println!(
                    "{} — agent={} lease={} expires={}{}",
                    c.path,
                    c.agent,
                    c.lease_id,
                    c.expires_at,
                    c.task
                        .map(|task| format!(" task={task}"))
                        .unwrap_or_default()
                );
            }
            println!("Claims are advisory declarations; writes are not fenced.");
        }
        Command::Release { path, agent, lease } => {
            crate::files::release_lease(&root, path, agent, lease)?;
            println!("Released advisory claim: {path} — agent={agent} lease={lease}");
        }
    }
    Ok(())
}
