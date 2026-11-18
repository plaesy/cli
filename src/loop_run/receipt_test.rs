//! Unit tests for `tests` split out of `receipt.rs`.
//! Kept as a separate file so they are easy to find; included via `#[path]`.

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
