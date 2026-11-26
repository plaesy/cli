//! `plaesy approvals`: persisted approvals for external actions (spec R9).
//!
//! Approvals are appended to `.plaesy/approvals.log`, so they survive a restart. The latest
//! record for an action id decides whether it is approved.
use super::{common::find_root, CliResult};
use clap::{Args as ClapArgs, Subcommand};
use plaesy_adapter_runstore::file::RunLog;
use plaesy_domain_workflow::policy::approvals::ApprovalStore;

#[derive(Debug, ClapArgs)]
pub struct Args {
    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Approve one external action id.
    Approve { action_id: String },
    /// Revoke an earlier approval.
    Revoke { action_id: String },
    /// Print `approved` or `not approved`; exits with status 1 when not approved.
    Check { action_id: String },
}

fn store_at(root: &std::path::Path) -> Result<ApprovalStore<RunLog>, Box<dyn std::error::Error>> {
    let dir = root.join(".plaesy");
    std::fs::create_dir_all(&dir)?;
    Ok(ApprovalStore::new(RunLog::open(dir.join("approvals.log"))))
}

fn store() -> Result<ApprovalStore<RunLog>, Box<dyn std::error::Error>> {
    store_at(&find_root()?)
}

/// Records approval for a tool action in the repository's approval log.
pub fn approve_at(root: &std::path::Path, action_id: &str) -> Result<(), String> {
    store_at(root)
        .map_err(|err| err.to_string())?
        .approve(action_id)
        .map_err(|err| err.to_string())
}

/// Checks the latest approval record for a tool action in the repository.
pub fn is_approved_at(root: &std::path::Path, action_id: &str) -> Result<bool, String> {
    store_at(root)
        .map_err(|err| err.to_string())?
        .is_approved(action_id)
        .map_err(|err| err.to_string())
}

pub fn run(args: &Args) -> CliResult {
    let Some(command) = &args.command else {
        return super::print_group_help("approvals");
    };
    match command {
        Command::Approve { action_id } => {
            store()?.approve(action_id)?;
            println!("approved {action_id}");
        }
        Command::Revoke { action_id } => {
            store()?.revoke(action_id)?;
            println!("revoked {action_id}");
        }
        Command::Check { action_id } => {
            if store()?.is_approved(action_id)? {
                println!("approved");
            } else {
                println!("not approved");
                std::process::exit(1);
            }
        }
    }
    Ok(())
}
