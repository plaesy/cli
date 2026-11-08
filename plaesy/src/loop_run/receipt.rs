//! Receipts: the evidence a passed checklist item stores, and the replay that
//! checks it still holds.
//!
//! A receipt stores the exit code and the SHA-256 of the item's output after
//! normalising it (ANSI codes, CR, and durations removed), so a rerun that
//! changes only timing still matches.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use chrono::{DateTime, Utc};
use regex::Regex;
use sha2::{Digest, Sha256};

use crate::checklist::{self, Checklist, CommandOutput, Item, Receipt, Status};

/// Where an item's output is stored, relative to the project root. The path depends
/// only on the task and item names, so it survives a task move.
#[must_use]
pub fn receipt_output_path(task: &str, item: &str) -> String {
    format!(".plaesy/receipts/{task}/{item}.out")
}

fn ansi() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"\x1b\[[0-9;?]*[ -/]*[@-~]|\x1b\][^\x07\x1b]*(?:\x07|\x1b\\)")
            .expect("static regex")
    })
}

fn ok_line() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"(?m)^(ok[ \t]+\S+)[ \t]+[0-9]+(?:\.[0-9]+)?(?:ns|µs|us|ms|s)[ \t]*$")
            .expect("static regex")
    })
}

fn paren_duration() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"\([0-9]+(?:\.[0-9]+)?(?:ns|µs|us|ms|s)\)").expect("static regex")
    })
}

fn bare_duration() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"\b[0-9]+(?:\.[0-9]+)?(?:ns|µs|us|ms|s)\b").expect("static regex")
    })
}

/// Removes what changes between runs without changing the result: ANSI codes, CR of
/// CRLF, and durations. Everything else is kept.
#[must_use]
pub fn normalise_output(out: &str) -> String {
    let out = out.replace("\r\n", "\n");
    let out = ansi().replace_all(&out, "");
    let out = ok_line().replace_all(&out, "$1");
    let out = paren_duration().replace_all(&out, "");
    bare_duration().replace_all(&out, "").into_owned()
}

/// SHA-256 (hex) of a stored output file after [`normalise_output`].
///
/// # Errors
///
/// The file cannot be read.
pub fn output_sha256(path: &Path) -> io::Result<String> {
    let data = fs::read_to_string(path)?;
    let digest = Sha256::digest(normalise_output(&data).as_bytes());
    Ok(digest.iter().map(|b| format!("{b:02x}")).collect())
}

/// The receipt of a run that wrote `rel` (relative to `root`) and exited with `exit_code`.
///
/// # Errors
///
/// The output file cannot be read.
pub fn receipt_for(root: &Path, rel: &str, exit_code: i32) -> io::Result<Receipt> {
    let sha256 = output_sha256(&root.join(rel))?;
    Ok(Receipt {
        output: rel.replace('\\', "/"),
        exit_code,
        sha256,
    })
}

/// Rejects a passed item that has no receipt, or whose receipt names a missing output file.
///
/// # Errors
///
/// A message listing every passed item without a valid receipt.
pub fn check_receipts(
    root: &Path,
    items: &[Item],
    receipts: &std::collections::BTreeMap<String, Receipt>,
) -> Result<(), String> {
    let mut problems = Vec::new();
    for item in items.iter().filter(|i| i.status == Status::Passed) {
        match receipts.get(&item.id) {
            None => problems.push(format!("{}: no receipt", item.id)),
            Some(rc) if !root.join(&rc.output).exists() => {
                problems.push(format!(
                    "{}: receipt output missing: {}",
                    item.id, rc.output
                ));
            }
            Some(_) => {}
        }
    }
    if problems.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "passed item(s) without a valid receipt: {}",
            problems.join("; ")
        ))
    }
}

/// Runs the item again and compares the exit code and output hash with the receipt.
///
/// # Errors
///
/// Any mismatch, a command that cannot run, or a missing output file.
pub fn replay(
    root: &Path,
    item: &Item,
    receipt: &Receipt,
    run: &mut dyn FnMut(&str) -> Result<CommandOutput, String>,
) -> Result<(), String> {
    let result = run(&item.run).map_err(|e| format!("{}: cannot run: {e}", item.id))?;
    if result.exit_code != receipt.exit_code {
        return Err(format!(
            "{}: exit code {}, receipt has {}",
            item.id, result.exit_code, receipt.exit_code
        ));
    }
    let sum = output_sha256(&root.join(&receipt.output))
        .map_err(|e| format!("{}: output missing: {e}", item.id))?;
    if sum != receipt.sha256 {
        return Err(format!(
            "{}: output hash changed since the receipt was written",
            item.id
        ));
    }
    Ok(())
}

/// Runs every item and settles its receipt. A passed item with no receipt gets one from
/// this run. A passed item with a receipt is replayed, and a mismatch fails it. The
/// receipt is kept, so a changed output cannot be accepted without someone removing it.
pub fn settle(
    root: &Path,
    task: &str,
    checklist: &mut Checklist,
    run: &mut dyn FnMut(&str) -> Result<CommandOutput, String>,
    now: DateTime<Utc>,
) {
    for index in 0..checklist.checklist.len() {
        let prior = checklist.checklist[index].receipt.clone();
        let item_id = checklist.checklist[index].id.clone();
        let output_rel = receipt_output_path(task, &item_id);

        let mut one = Checklist {
            id: checklist.id.clone(),
            checklist: vec![checklist.checklist[index].clone()],
        };
        checklist::run(
            &mut one,
            |cmd| {
                let result = run(cmd)?;
                let _ = write_output(root, &output_rel, &result.stdout);
                Ok(result)
            },
            now,
        );
        checklist.checklist[index] = one.checklist.remove(0);
        if checklist.checklist[index].status != Status::Passed {
            continue;
        }

        let item = checklist.checklist[index].clone();
        match prior {
            None => match receipt_for(root, &output_rel, 0) {
                Ok(receipt) => checklist.checklist[index].receipt = Some(receipt),
                Err(e) => fail(&mut checklist.checklist[index], now, e.to_string()),
            },
            Some(prior) => {
                if let Err(message) = replay(root, &item, &prior, run) {
                    fail(&mut checklist.checklist[index], now, message);
                }
            }
        }
    }
}

fn fail(item: &mut Item, now: DateTime<Utc>, message: String) {
    item.status = Status::Failed;
    item.histories.push(checklist::History {
        at: now.to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
        actual: message,
        run: item.run.clone(),
    });
}

fn write_output(root: &Path, rel: &str, out: &str) -> io::Result<()> {
    let path: PathBuf = root.join(rel);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn durations_and_ansi_do_not_change_the_hash_input() {
        let a = normalise_output("\x1b[32mok\x1b[0m\r\nok  \tpkg/x\t0.81s\nran in 12ms (0.02s)\n");
        let b = normalise_output("ok\nok  \tpkg/x\t3.10s\nran in 9ms (0.01s)\n");
        assert_eq!(a, b);
    }

    #[test]
    fn a_changed_result_still_changes_the_normalised_output() {
        assert_ne!(normalise_output("PASS"), normalise_output("FAIL"));
    }

    #[test]
    fn receipt_output_path_depends_only_on_names() {
        assert_eq!(
            receipt_output_path("auth", "C1"),
            ".plaesy/receipts/auth/C1.out"
        );
    }
}
