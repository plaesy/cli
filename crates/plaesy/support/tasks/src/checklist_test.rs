//! Unit tests for `tests` split out of `checklist.rs`.
//! Kept as a separate file so they are easy to find; included via `#[path]`.

use chrono::TimeZone;

use super::*;

fn now() -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 10, 22, 12, 0, 0).unwrap()
}

fn item(id: &str, run: &str, expected: &str) -> Item {
    Item {
        id: id.into(),
        check: String::new(),
        run: run.into(),
        expected: expected.into(),
        status: Status::Pending,
        histories: Vec::new(),
        receipt: None,
        evidence: None,
    }
}

fn ok(stdout: &str) -> Result<CommandOutput, String> {
    Ok(CommandOutput {
        exit_code: 0,
        stdout: stdout.into(),
    })
}

#[test]
fn failures_are_kept_in_histories() {
    let mut list = Checklist {
        id: "t".into(),
        checklist: vec![
            item("C1", "ok", "pass"),
            item("C2", "count", ">= 3"),
            item("C3", "broken", "pass"),
        ],
    };
    run(
        &mut list,
        |cmd| match cmd {
            "ok" => ok(""),
            "count" => ok("5\n"),
            _ => Err("no shell".into()),
        },
        now(),
    );

    assert_eq!(list.checklist[0].status, Status::Passed);
    assert_eq!(list.checklist[1].status, Status::Passed);
    let c3 = &list.checklist[2];
    assert_eq!(c3.status, Status::Failed);
    assert_eq!(
        c3.histories,
        [History {
            at: "2026-10-22T12:00:00Z".into(),
            actual: "no shell".into(),
            run: "broken".into(),
        }]
    );
    assert!(!is_done(&list));
}

#[test]
fn failures_accumulate_and_a_later_pass_keeps_them() {
    let mut list = Checklist {
        id: "t".into(),
        checklist: vec![item("C1", "x", "pass")],
    };
    let fail = |_: &str| ok_code(1);
    run(&mut list, fail, now());
    run(&mut list, |_| ok_code(1), now());
    assert_eq!(list.checklist[0].histories.len(), 2);

    run(&mut list, |_| ok(""), now());
    assert!(is_done(&list));
    assert_eq!(list.checklist[0].histories.len(), 2);
    assert_eq!(progress(&list), (1, 1));
}

fn ok_code(exit_code: i32) -> Result<CommandOutput, String> {
    Ok(CommandOutput {
        exit_code,
        stdout: String::new(),
    })
}

#[test]
fn sentence_expectation_names_the_accepted_values() {
    let mut list = Checklist {
        id: "t".into(),
        checklist: vec![item(
            "R10",
            "go test ./internal/x/ -count=1",
            "exit code 2 while running; turn ends when complete",
        )],
    };
    run(&mut list, |_| ok("ok example 0.012s\n"), now());

    assert_eq!(list.checklist[0].status, Status::Failed);
    let msg = &list.checklist[0].histories[0].actual;
    for want in ["ok example 0.012s", "pass", "zero", "all", ">= 9"] {
        assert!(msg.contains(want), "history {msg:?} must contain {want:?}");
    }
}

#[cfg(unix)]
#[test]
fn handle_load_and_save_stay_in_pinned_directory_after_path_replacement() {
    use std::os::unix::fs::symlink;

    let temp = tempfile::tempdir().unwrap();
    let task = temp.path().join("task");
    std::fs::create_dir(&task).unwrap();
    let contract = task.join("contracts.json");
    std::fs::write(
            &contract,
            r#"{"id":"t","checklist":[{"id":"C1","check":"","run":"true","expected":"pass","status":"pending"}]}"#,
        )
        .unwrap();
    let external = temp.path().join("external");
    std::fs::create_dir(&external).unwrap();
    std::fs::write(external.join("contracts.json"), "external sentinel").unwrap();
    let handle = crate::memory::fs_safety::DirHandle::open_nofollow(&task).unwrap();
    let displaced = temp.path().join("task-original");
    std::fs::rename(&task, &displaced).unwrap();
    symlink(&external, &task).unwrap();

    let mut checklist = load_from_handle(&handle, "contracts.json").unwrap();
    checklist.checklist[0].status = Status::Passed;
    save_to_handle(&handle, "contracts.json", &checklist).unwrap();

    let saved = std::fs::read_to_string(displaced.join("contracts.json")).unwrap();
    assert!(saved.contains("passed"));
    assert_eq!(
        std::fs::read_to_string(external.join("contracts.json")).unwrap(),
        "external sentinel"
    );
}

#[cfg(unix)]
#[test]
fn failed_handle_save_removes_temporary_file_and_preserves_destination() {
    let temp = tempfile::tempdir().unwrap();
    let task = temp.path().join("task");
    std::fs::create_dir(&task).unwrap();
    std::fs::create_dir(task.join("contracts.json")).unwrap();
    let handle = crate::memory::fs_safety::DirHandle::open_nofollow(&task).unwrap();
    let checklist = Checklist {
        id: "t".into(),
        checklist: Vec::new(),
    };

    assert!(save_to_handle(&handle, "contracts.json", &checklist).is_err());
    let entries: Vec<_> = std::fs::read_dir(&task)
        .unwrap()
        .map(|entry| entry.unwrap().file_name())
        .collect();
    assert_eq!(entries, [std::ffi::OsString::from("contracts.json")]);
    assert!(task.join("contracts.json").is_dir());
}

#[test]
fn load_rejects_unknown_fields_and_incomplete_items() {
    let dir = tempfile::tempdir().unwrap();
    let typo = dir.path().join("typo.json");
    fs::write(
            &typo,
            r#"{"id":"t","checklist":[{"id":"C1","check":"","run":"x","expected":"pass","status":"pending","histroies":[]}]}"#,
        )
        .unwrap();
    assert!(matches!(load(&typo), Err(ChecklistError::Parse { .. })));

    let empty_run = dir.path().join("empty.json");
    fs::write(
            &empty_run,
            r#"{"id":"t","checklist":[{"id":"C1","check":"","run":"","expected":"pass","status":"pending"}]}"#,
        )
        .unwrap();
    assert!(matches!(
        load(&empty_run),
        Err(ChecklistError::Incomplete { .. })
    ));
}

#[test]
fn load_rejects_path_like_item_ids() {
    let dir = tempfile::tempdir().unwrap();
    for id in ["../outside", "../../outside", ".", "..", "a/b", "a\\\\b"] {
        let path = dir.path().join("contracts.json");
        let json = format!(
            r#"{{"id":"t","checklist":[{{"id":{id:?},"check":"","run":"true","expected":"pass","status":"pending"}}]}}"#
        );
        fs::write(&path, json).unwrap();
        assert!(
            matches!(load(&path), Err(ChecklistError::Incomplete { .. })),
            "accepted {id:?}"
        );
    }

    let path = dir.path().join("valid.json");
    fs::write(
            &path,
            r#"{"id":"t","checklist":[{"id":"C1","check":"","run":"true","expected":"pass","status":"pending"}]}"#,
        )
        .unwrap();
    assert_eq!(load(&path).unwrap().checklist[0].id, "C1");
}

#[test]
fn save_then_load_round_trips() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("contracts.json");
    let list = Checklist {
        id: "t".into(),
        checklist: vec![item("C1", "x", "pass")],
    };
    save(&path, &list).unwrap();
    assert_eq!(load(&path).unwrap(), list);
}

#[test]
fn contract_document_loads_without_external_assets() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("contracts.json");
    fs::write(
            &path,
            r#"{"id":"example","checklist":[{"id":"C1","check":"command succeeds","run":"true","expected":"pass","status":"pending"}]}"#,
        )
        .unwrap();
    load(&path).expect("contract document loads");
}
