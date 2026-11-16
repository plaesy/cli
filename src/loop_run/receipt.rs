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
pub fn receipt_output_path(task: &str, item: &str) -> io::Result<String> {
    fn valid_component(value: &str) -> bool {
        !value.contains('/')
            && !value.contains('\\')
            && !value.contains('\0')
            && matches!(
                Path::new(value).components().next(),
                Some(std::path::Component::Normal(_))
            )
            && Path::new(value).components().nth(1).is_none()
    }

    if !valid_component(task) || !valid_component(item) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "receipt task name and item id must be single path components",
        ));
    }
    Ok(format!(".plaesy/receipts/{task}/{item}.out"))
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
    let path = safe_receipt_path(root, rel)?;
    let sha256 = output_sha256(&path)?;
    Ok(Receipt {
        output: rel.replace('\\', "/"),
        exit_code,
        sha256,
    })
}

fn validate_receipt_rel(rel: &str) -> io::Result<&Path> {
    let path = Path::new(rel);
    let mut components = path.components();
    let valid = matches!(components.next(), Some(std::path::Component::Normal(p)) if p == ".plaesy")
        && matches!(components.next(), Some(std::path::Component::Normal(p)) if p == "receipts")
        && matches!(components.next(), Some(std::path::Component::Normal(_)))
        && matches!(components.next(), Some(std::path::Component::Normal(name)) if name.to_string_lossy().ends_with(".out"))
        && components.next().is_none()
        && !rel.contains('\\');
    if valid {
        Ok(path)
    } else {
        Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "receipt output must be .plaesy/receipts/<task>/<item>.out",
        ))
    }
}

fn safe_receipt_path(root: &Path, rel: &str) -> io::Result<PathBuf> {
    let path = validate_receipt_rel(rel)?;
    let root = root.canonicalize()?;
    let full = root.join(path);
    let parent = full
        .parent()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "receipt path has no parent"))?;
    let canonical_parent = parent.canonicalize()?;
    if !canonical_parent.starts_with(&root) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "receipt parent resolves outside project root",
        ));
    }
    Ok(full)
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
            Some(rc) => match safe_receipt_path(root, &rc.output) {
                Ok(path) if path.exists() => {}
                _ => problems.push(format!(
                    "{}: receipt output missing or invalid: {}",
                    item.id, rc.output
                )),
            },
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
    let path = safe_receipt_path(root, &receipt.output)
        .map_err(|e| format!("{}: invalid receipt output: {e}", item.id))?;
    let sum = output_sha256(&path).map_err(|e| format!("{}: output missing: {e}", item.id))?;
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
        let output_rel = match receipt_output_path(task, &item_id) {
            Ok(path) => path,
            Err(error) => {
                fail(&mut checklist.checklist[index], now, error.to_string());
                continue;
            }
        };

        let mut one = Checklist {
            id: checklist.id.clone(),
            checklist: vec![checklist.checklist[index].clone()],
        };
        let mut output_error = None;
        checklist::run(
            &mut one,
            |cmd| {
                let result = run(cmd)?;
                if let Err(error) = write_output(root, &output_rel, &result.stdout) {
                    output_error = Some(error);
                }
                Ok(result)
            },
            now,
        );
        checklist.checklist[index] = one.checklist.remove(0);
        if let Some(error) = output_error {
            fail(&mut checklist.checklist[index], now, error.to_string());
            continue;
        }
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
    validate_receipt_rel(rel)?;
    let root = root.canonicalize()?;
    let path = root.join(rel);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    // Resolve the created parent and reject symlink escapes before opening output.
    // This is not atomic against concurrent replacement.
    let path = safe_receipt_path(&root, rel)?;
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
    fn unsafe_receipt_output_is_rejected_without_reading_outside_root() {
        let root = tempfile::tempdir().unwrap();
        for rel in [
            "../../secret",
            ".plaesy/receipts/task/../../secret.out",
            "/etc/passwd",
            ".plaesy/receipts/task/other.txt",
            ".plaesy/receipts/task/item.out/extra",
        ] {
            assert!(
                safe_receipt_path(root.path(), rel).is_err(),
                "accepted {rel:?}"
            );
        }

        let item = Item {
            id: "C1".into(),
            check: String::new(),
            run: "true".into(),
            expected: "pass".into(),
            status: Status::Passed,
            histories: Vec::new(),
            receipt: None,
            evidence: None,
        };
        let receipt = Receipt {
            output: "../../secret".into(),
            exit_code: 0,
            sha256: String::new(),
        };
        assert!(replay(root.path(), &item, &receipt, &mut |_| {
            Ok(CommandOutput {
                exit_code: 0,
                stdout: String::new(),
            })
        })
        .unwrap_err()
        .contains("invalid receipt output"));
    }

    #[cfg(unix)]
    #[test]
    fn symlinked_receipt_directories_are_rejected() {
        use std::os::unix::fs::symlink;

        for link_component in ["receipts", "task"] {
            let root = tempfile::tempdir().unwrap();
            let outside = tempfile::tempdir().unwrap();
            let receipts = root.path().join(".plaesy/receipts");
            std::fs::create_dir_all(receipts.parent().unwrap()).unwrap();
            if link_component == "receipts" {
                symlink(outside.path(), &receipts).unwrap();
            } else {
                std::fs::create_dir(&receipts).unwrap();
                symlink(outside.path(), receipts.join("task")).unwrap();
            }

            let rel = receipt_output_path("task", "C1").unwrap();
            assert!(safe_receipt_path(root.path(), &rel).is_err());
            assert!(outside.path().read_dir().unwrap().next().is_none());
        }
    }

    #[cfg(unix)]
    #[test]
    fn write_output_rejects_symlinked_receipt_directories() {
        use std::os::unix::fs::symlink;

        for link_component in ["receipts", "task"] {
            let root = tempfile::tempdir().unwrap();
            let outside = tempfile::tempdir().unwrap();
            let sentinel = outside.path().join("C1.out");
            fs::write(&sentinel, "untouched").unwrap();
            let receipts = root.path().join(".plaesy/receipts");
            fs::create_dir_all(receipts.parent().unwrap()).unwrap();
            if link_component == "receipts" {
                symlink(outside.path(), &receipts).unwrap();
            } else {
                fs::create_dir(&receipts).unwrap();
                symlink(outside.path(), receipts.join("task")).unwrap();
            }

            let rel = receipt_output_path("task", "C1").unwrap();
            assert!(write_output(root.path(), &rel, "overwritten").is_err());
            assert_eq!(fs::read_to_string(sentinel).unwrap(), "untouched");
        }
    }

    #[test]
    fn receipt_output_path_depends_only_on_names() {
        assert_eq!(
            receipt_output_path("auth", "C1").unwrap(),
            ".plaesy/receipts/auth/C1.out"
        );
        assert!(receipt_output_path("auth", "../outside").is_err());
        assert!(receipt_output_path("../outside", "C1").is_err());
    }
}
