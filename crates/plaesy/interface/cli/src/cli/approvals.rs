//! `plaesy approvals`: persisted approvals for external actions (spec R9).
//!
//! Approvals are appended to `.plaesy/approvals.log`, so they survive a restart. The latest
//! record for an action id decides whether it is approved. Each approve, revoke, and check is
//! also recorded in `.plaesy/audit.log`, which `plaesy audit list` reads.
use super::{common::find_root, CliResult};
use clap::{Args as ClapArgs, Subcommand};
use plaesy_adapter_runstore::file::RunLog;
use plaesy_domain_workflow::policy::approvals::ApprovalStore;
use plaesy_domain_workflow::runstore::audit::{AuditRecord, AuditTrail};

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

/// Appends one decision to `.plaesy/audit.log` under `root`.
///
/// The audit entry is written before the approval change, so a failed audit write leaves the
/// approval unchanged (fail closed).
fn audit_at(
    root: &std::path::Path,
    action: &str,
    allowed: bool,
    reason: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let dir = root.join(".plaesy");
    std::fs::create_dir_all(&dir)?;
    let at = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_secs();
    let project = root
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    let record = AuditRecord {
        at,
        principal: "cli".into(),
        project,
        action: action.into(),
        allowed,
        reason: reason.into(),
    };
    AuditTrail::new(RunLog::open(dir.join("audit.log"))).append(&record)?;
    Ok(())
}

/// Runs an approval command against `root`, writing the audit entry first.
fn apply_at(root: &std::path::Path, command: &Command) -> Result<(), Box<dyn std::error::Error>> {
    match command {
        Command::Approve { action_id } => {
            audit_at(root, "approval:approve", true, "approved")?;
            store_at(root)?.approve(action_id)?;
            println!("approved {action_id}");
        }
        Command::Revoke { action_id } => {
            audit_at(root, "approval:revoke", true, "revoked")?;
            store_at(root)?.revoke(action_id)?;
            println!("revoked {action_id}");
        }
        Command::Check { action_id } => {
            let approved = store_at(root)?.is_approved(action_id)?;
            let reason = if approved { "approved" } else { "not approved" };
            audit_at(root, "approval:check", approved, reason)?;
            if approved {
                println!("approved");
            } else {
                println!("not approved");
                std::process::exit(1);
            }
        }
    }
    Ok(())
}

pub fn run(args: &Args) -> CliResult {
    let Some(command) = &args.command else {
        return super::print_group_help("approvals");
    };
    apply_at(&find_root()?, command)
}

#[cfg(test)]
#[path = "approvals_test.rs"]
mod approvals_test;
