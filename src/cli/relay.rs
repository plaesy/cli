//! CLI for durable direct relay messages between registered agent identities.

use clap::{Parser, Subcommand};
use std::path::PathBuf;

use super::CliResult;

#[derive(Debug, Parser)]
#[command(arg_required_else_help = true)]
pub struct Args {
    /// SQLite database file. Protect it as private project data.
    #[arg(long, default_value = ".plaesy/relay.sqlite")]
    db: PathBuf,
    #[command(subcommand)]
    command: Action,
}

#[derive(Debug, Subcommand)]
enum Action {
    /// Send a durable message (repeat the ID for an idempotent retry).
    Send {
        #[arg(long)]
        id: String,
        #[arg(long)]
        from: String,
        #[arg(long)]
        to: String,
        body: String,
    },
    /// List pending messages for one agent; fetching does not acknowledge.
    Inbox {
        #[arg(long)]
        agent: String,
        #[arg(long, default_value_t = 100)]
        limit: usize,
    },
    /// Acknowledge a pending message addressed to one agent.
    Ack {
        #[arg(long)]
        id: String,
        #[arg(long)]
        agent: String,
    },
}

/// Execute the direct-message CLI action.
pub fn run(args: &Args) -> CliResult {
    if let Some(parent) = args.db.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent)?;
    }
    let mut store = crate::relay::MessageStore::open(&args.db)?;
    match &args.command {
        Action::Send { id, from, to, body } => println!(
            "{}",
            serde_json::to_string(&store.send(id, from, to, body)?)?
        ),
        Action::Inbox { agent, limit } => println!(
            "{}",
            serde_json::to_string_pretty(&store.inbox(agent, *limit)?)?
        ),
        Action::Ack { id, agent } => {
            if !store.acknowledge(id, agent)? {
                return Err(
                    "message is missing, already acknowledged, or not addressed to this agent"
                        .into(),
                );
            }
            println!("acknowledged {id}");
        }
    }
    Ok(())
}
