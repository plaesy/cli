//! `plaesy tasks validate`: runs each task's checklist, records the results, and moves finished
//! tasks to done.

use std::path::Path;

use chrono::Utc;

use super::common::{find_root, run_bash};
use super::CliResult;
use crate::checklist::{self, CommandOutput};
use crate::loop_run::receipt;
use crate::tasks::{self, contracts::ContractRef};

/// Runs the contracts.json checklist of each task folder, or of one task folder.
#[derive(Debug, clap::Args)]
pub struct Args {
    /// Task folder name (default: every task)
    pub task: Option<String>,
}

/// A task whose checklist ran but is not finished, or has nothing to prove it done.
#[derive(Debug, thiserror::Error)]
enum NotPassed {
    #[error("checklist not passed")]
    Unfinished,
    #[error("no checks defined")]
    NoChecks,
    #[error("items lack behaviour-level evidence")]
    NoBehaviour,
}

/// Runs the checklists. `spec_flag` limits the search to one spec when non-empty.
pub fn run(args: &Args, spec_flag: &str) -> CliResult {
    println!(
        "[INFO] plaesy {} at {}",
        crate::version(),
        std::env::current_exe()?.display()
    );
    let root = find_root()?;
    let tasks_dir = tasks::tasks_dir(&root);
    let refs = tasks::contracts::list_contracts(&tasks_dir, spec_flag)?;

    let mut checked = 0usize;
    let mut failed = 0usize;
    for contract in &refs {
        if args.task.as_ref().is_some_and(|t| *t != contract.task) {
            continue;
        }
        checked += 1;
        if let Err(e) = validate_task(&root, &tasks_dir, contract) {
            failed += 1;
            println!("  [!] {}/{}: {e}", contract.spec, contract.task);
        }
    }
    if checked == 0 {
        println!("[!] No task with a contracts.json was found");
        return Err("no task with a contracts.json was found".into());
    }
    println!("\n[INFO] {checked} task(s) checked, {failed} not passed");
    if failed > 0 {
        return Err(format!("{failed} task checklist(s) not passed").into());
    }
    Ok(())
}

fn validate_task(
    root: &Path,
    tasks_dir: &Path,
    contract: &ContractRef,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut list = checklist::load(&contract.path)?;
    if list.checklist.is_empty() {
        println!(
            "{}/{} ({}): {}",
            contract.spec,
            contract.task,
            contract.status,
            NotPassed::NoChecks
        );
        return Err(Box::new(NotPassed::NoChecks));
    }

    // Each passed item keeps a receipt, and a receipt that does not replay is not passed.
    let mut exec = |command: &str| -> Result<CommandOutput, String> {
        let (exit_code, stdout) = run_bash(root, command)?;
        Ok(CommandOutput { exit_code, stdout })
    };
    receipt::settle(root, &contract.task, &mut list, &mut exec, Utc::now());
    checklist::save(&contract.path, &list)?;

    let (passed, total) = checklist::progress(&list);
    println!(
        "{}/{} ({}): {passed}/{total} checks passed",
        contract.spec, contract.task, contract.status
    );
    if !checklist::is_done(&list) {
        return Err(Box::new(NotPassed::Unfinished));
    }
    if contract.status != tasks::Status::Done {
        let mut gaps = 0usize;
        for item in &list.checklist {
            if !is_exempt(item) && !invokes_plaesy(&item.run) {
                println!(
                    "  [!] {}: item {} lacks behaviour-level evidence: run {:?} does not invoke the plaesy binary",
                    contract.task, item.id, item.run
                );
                gaps += 1;
            }
        }
        if gaps > 0 {
            return Err(Box::new(NotPassed::NoBehaviour));
        }
        tasks::move_task(
            tasks_dir,
            &contract.task,
            contract.status,
            tasks::Status::Done,
        )?;
        println!("  moved to done");
    }
    Ok(())
}

/// Build and document checks are exempt from the behaviour-evidence rule; every other item
/// must invoke the plaesy binary.
pub fn is_exempt(item: &checklist::Item) -> bool {
    matches!(item.evidence.as_deref(), Some("build" | "doc"))
}

/// Whether a run command starts the plaesy binary: a segment of the command line, split on `&&`,
/// `;`, or `|`, whose first word is `plaesy`, `plaesy.exe`, or `plaesy-repo.exe`.
#[must_use]
pub fn invokes_plaesy(run: &str) -> bool {
    let split = run.replace("&&", "\n").replace([';', '|'], "\n");
    split.lines().any(|segment| {
        segment.split_whitespace().next().is_some_and(|first| {
            let word = first.trim_matches(|c| c == '"' || c == '\'');
            word == "plaesy" || word.ends_with("plaesy.exe") || word.ends_with("plaesy-repo.exe")
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_and_doc_items_are_exempt_from_the_behaviour_rule() {
        let mut item = crate::checklist::Item {
            id: "x".into(),
            check: String::new(),
            run: "cargo test".into(),
            expected: "pass".into(),
            status: crate::checklist::Status::Pending,
            histories: Vec::new(),
            receipt: None,
            evidence: None,
        };
        assert!(!is_exempt(&item));
        item.evidence = Some("build".into());
        assert!(is_exempt(&item));
        item.evidence = Some("doc".into());
        assert!(is_exempt(&item));
        item.evidence = Some("other".into());
        assert!(!is_exempt(&item));
    }

    #[test]
    fn a_plaesy_segment_counts_as_behaviour_evidence() {
        assert!(invokes_plaesy("cargo build && plaesy status"));
        assert!(invokes_plaesy("\"plaesy.exe\" validate"));
        assert!(!invokes_plaesy("go test ./..."));
    }
}
