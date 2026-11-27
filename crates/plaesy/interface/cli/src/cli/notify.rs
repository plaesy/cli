//! `plaesy notify`: durable local notifications for input needed, merge readiness, merged, and
//! closed events. Records stay in `.plaesy/notifications.jsonl` until they are removed.
use super::{common::find_root, CliResult};
use clap::{Args as ClapArgs, Subcommand};
use plaesy_common::notify::{self, Kind, Notification, Target};

#[derive(Debug, ClapArgs)]
pub struct Args {
    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Record a notification. Kinds: input-needed, ready-to-merge, merged, closed
    Add {
        #[arg(long, value_parser = parse_kind)]
        kind: Kind,
        #[arg(long)]
        session: String,
        #[arg(long)]
        message: String,
    },
    /// List notifications, oldest first (use --unread to show only unread)
    Ls {
        #[arg(long)]
        unread: bool,
    },
    /// Mark one notification (--id) or all of them (--all) as read; nothing is deleted
    Read {
        #[arg(long)]
        id: Option<String>,
        #[arg(long)]
        all: bool,
    },
    /// Delete one notification (--id) or all of them (--all)
    Clear {
        #[arg(long)]
        id: Option<String>,
        #[arg(long)]
        all: bool,
    },
}

fn parse_kind(text: &str) -> Result<Kind, String> {
    Kind::parse(text).ok_or_else(|| {
        format!(
            "unknown kind `{text}`; use one of: {}",
            Kind::ALL.map(Kind::as_str).join(", ")
        )
    })
}

/// One line for a notification: id, kind, read state, session, time, then the message.
#[must_use]
pub fn format_line(item: &Notification) -> String {
    format!(
        "{} [{}] {} session={} created={} {}",
        item.id,
        item.kind.as_str(),
        if item.read_at.is_some() {
            "read"
        } else {
            "unread"
        },
        item.session,
        item.created_at,
        item.message
    )
}

pub fn run(args: &Args) -> CliResult {
    let Some(command) = &args.command else {
        return super::print_group_help("notify");
    };
    match command {
        Command::Add {
            kind,
            session,
            message,
        } => {
            let root = find_root()?;
            let item = notify::add(&root, *kind, session, message)?;
            println!("Recorded notification {} ({})", item.id, item.kind.as_str());
        }
        Command::Ls { unread } => {
            let root = find_root()?;
            let list = notify::list(&root)?;
            let shown: Vec<_> = list
                .iter()
                .filter(|item| !*unread || item.read_at.is_none())
                .collect();
            if shown.is_empty() {
                println!("No notifications.");
            }
            for item in shown {
                println!("{}", format_line(item));
            }
        }
        Command::Read { id, all } => {
            let root = find_root()?;
            let changed = notify::mark_read(&root, target(id.as_deref(), *all)?)?;
            println!("Marked {changed} notification(s) as read.");
        }
        Command::Clear { id, all } => {
            let root = find_root()?;
            let removed = notify::remove(&root, target(id.as_deref(), *all)?)?;
            println!("Removed {removed} notification(s).");
        }
    }
    Ok(())
}

/// `--id <id>` or `--all`, and exactly one of them.
fn target(id: Option<&str>, all: bool) -> Result<Target<'_>, String> {
    match (id, all) {
        (Some(id), false) => Ok(Target::One(id)),
        (None, true) => Ok(Target::All),
        _ => Err("give exactly one of --id <id> or --all".into()),
    }
}

#[cfg(test)]
#[path = "notify_test.rs"]
mod notify_test;
