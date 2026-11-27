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
    /// Wait for the next pending message; does not acknowledge it.
    Wait {
        #[arg(long)]
        agent: String,
        /// Maximum seconds to wait; zero waits indefinitely.
        #[arg(long, default_value_t = 60)]
        timeout: u64,
        /// Poll interval in milliseconds.
        #[arg(long, default_value_t = 500)]
        poll_interval: u64,
    },
    /// Atomically claim the next available message with a bounded lease.
    Claim {
        #[arg(long)]
        agent: String,
        #[arg(long, default_value_t = 300)]
        lease_seconds: u64,
    },
    /// Acknowledge a pending message addressed to one agent.
    Ack {
        #[arg(long)]
        id: String,
        /// Sender of the message. IDs are unique per sender, so it is required.
        #[arg(long)]
        from: String,
        #[arg(long)]
        agent: String,
        /// Opaque lease token returned by `claim`; required for claimed messages.
        #[arg(long = "lease-token")]
        lease_token: Option<String>,
    },
}

/// Execute the direct-message CLI action.
pub fn run(args: &Args) -> CliResult {
    if let Some(parent) = args.db.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent)?;
    }
    let mut store = crate::relay::MessageStore::open(&args.db)?;
    match &args.command {
        Action::Send { id, from, to, body } => {
            let mut message = store.send(id, from, to, body)?;
            message.lease_owner = None;
            println!("{}", serde_json::to_string(&message)?);
        }
        Action::Inbox { agent, limit } => {
            let mut messages = store.inbox(agent, *limit)?;
            for message in &mut messages {
                message.lease_owner = None;
            }
            println!("{}", serde_json::to_string_pretty(&messages)?);
        }
        Action::Wait {
            agent,
            timeout,
            poll_interval,
        } => {
            if *poll_interval == 0 {
                return Err("poll interval must be greater than zero milliseconds".into());
            }
            let deadline = (*timeout > 0)
                .then(|| std::time::Instant::now() + std::time::Duration::from_secs(*timeout));
            loop {
                let messages = store.inbox(agent, 1)?;
                if let Some(mut message) = messages.into_iter().next() {
                    message.lease_owner = None;
                    println!("{}", serde_json::to_string(&message)?);
                    break;
                }
                if deadline.is_some_and(|end| std::time::Instant::now() >= end) {
                    println!("null");
                    break;
                }
                let interval = std::time::Duration::from_millis(*poll_interval);
                let sleep_for = deadline
                    .map(|end| {
                        interval.min(end.saturating_duration_since(std::time::Instant::now()))
                    })
                    .unwrap_or(interval);
                std::thread::sleep(sleep_for);
            }
        }
        Action::Claim {
            agent,
            lease_seconds,
        } => {
            let message = store.claim_next(agent, *lease_seconds)?;
            println!("{}", serde_json::to_string(&message)?);
        }
        Action::Ack {
            id,
            from,
            agent,
            lease_token,
        } => {
            let acknowledged = if let Some(token) = lease_token {
                store.acknowledge_claim(from, id, agent, token)?
            } else {
                store.acknowledge(from, id, agent)?
            };
            if !acknowledged {
                return Err(
                    "message is missing, already acknowledged, leased, or lease token is stale"
                        .into(),
                );
            }
            println!("acknowledged {id}");
        }
    }
    Ok(())
}
