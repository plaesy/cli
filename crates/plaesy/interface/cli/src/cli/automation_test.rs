//! Unit tests for `automation.rs`, included via `#[path]`.

use super::*;
use chrono::TimeZone;

fn at(hour: u32, minute: u32) -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 1, 1, hour, minute, 0)
        .single()
        .expect("valid test time")
}

fn make_interval(dir: &Path, name: &str) -> Automation {
    let spec = NewAutomation {
        name,
        prompt: "check the inbox",
        agent: "claude",
        role: "reviewer",
        cron: None,
        every: Some("30m"),
    };
    create(dir, spec, at(10, 0)).expect("create interval automation")
}

#[test]
fn create_writes_an_enabled_definition_and_refuses_a_duplicate() {
    let temp = tempfile::tempdir().expect("tempdir");
    let dir = store_dir(temp.path());
    let made = make_interval(&dir, "inbox-check");
    assert!(made.enabled);
    assert_eq!(made.last_run, None);
    assert_eq!(load(&dir, "inbox-check").expect("load"), made);
    let spec = NewAutomation {
        name: "inbox-check",
        prompt: "x",
        agent: "a",
        role: "r",
        cron: None,
        every: Some("1h"),
    };
    let again = create(&dir, spec, at(11, 0));
    assert!(again.is_err(), "duplicate name must be refused");
}

#[test]
fn prompt_limit_counts_utf8_bytes() {
    assert!(validate_prompt(&"a".repeat(PROMPT_MAX_BYTES)).is_ok());
    assert!(validate_prompt(&"a".repeat(PROMPT_MAX_BYTES + 1)).is_err());
    // 2049 two-byte characters are 4098 bytes, over the limit even though the count is lower.
    assert!(validate_prompt(&"é".repeat(2049)).is_err());
    assert!(validate_prompt(&"é".repeat(2048)).is_ok());
    assert!(validate_prompt("   ").is_err());
}

#[test]
fn exactly_one_schedule_is_required() {
    assert!(validate_schedule(None, None).is_err());
    assert!(validate_schedule(Some("0 9 * * 1-5"), Some("30m")).is_err());
    assert!(validate_schedule(Some("0 9 * * 1-5"), None).is_ok());
    assert!(validate_schedule(None, Some("6h")).is_ok());
}

#[test]
fn an_invalid_cron_or_interval_is_refused() {
    assert!(validate_schedule(Some("*/0 * * * *"), None).is_err());
    assert!(validate_schedule(Some("0 9 * *"), None).is_err());
    assert!(validate_schedule(None, Some("0m")).is_err());
    assert!(validate_schedule(None, Some("soon")).is_err());
}

#[test]
fn names_that_could_escape_the_store_are_refused() {
    assert!(validate_name("../escape").is_err());
    assert!(validate_name("a/b").is_err());
    assert!(validate_name("").is_err());
    assert!(validate_name(&"n".repeat(65)).is_err());
    assert!(validate_name("daily-triage_2").is_ok());
}

#[test]
fn an_interval_is_queued_once_per_period() {
    let temp = tempfile::tempdir().expect("tempdir");
    let dir = store_dir(temp.path());
    make_interval(&dir, "inbox-check");

    let first = run_due(&dir, at(10, 0)).expect("pass").expect("lock free");
    assert_eq!(first.len(), 1, "a never-run automation is due at once");
    assert!(run_due(&dir, at(10, 0))
        .expect("pass")
        .expect("lock free")
        .is_empty());
    assert!(run_due(&dir, at(10, 29))
        .expect("pass")
        .expect("lock free")
        .is_empty());
    let later = run_due(&dir, at(10, 30)).expect("pass").expect("lock free");
    assert_eq!(later.len(), 1);
}

#[test]
fn a_cron_job_is_queued_only_at_its_occurrence() {
    let temp = tempfile::tempdir().expect("tempdir");
    let dir = store_dir(temp.path());
    let spec = NewAutomation {
        name: "quarter",
        prompt: "summarize",
        agent: "claude",
        role: "writer",
        cron: Some("*/15 * * * *"),
        every: None,
    };
    create(&dir, spec, at(10, 0)).expect("create cron automation");

    assert_eq!(
        run_due(&dir, at(10, 0)).expect("pass").expect("lock").len(),
        1
    );
    assert!(run_due(&dir, at(10, 7))
        .expect("pass")
        .expect("lock")
        .is_empty());
    assert_eq!(
        run_due(&dir, at(10, 15))
            .expect("pass")
            .expect("lock")
            .len(),
        1
    );
}

#[test]
fn a_paused_automation_is_not_queued_until_resumed() {
    let temp = tempfile::tempdir().expect("tempdir");
    let dir = store_dir(temp.path());
    make_interval(&dir, "inbox-check");
    set_enabled(&dir, "inbox-check", false).expect("pause");
    assert!(run_due(&dir, at(10, 0))
        .expect("pass")
        .expect("lock")
        .is_empty());
    set_enabled(&dir, "inbox-check", true).expect("resume");
    assert_eq!(
        run_due(&dir, at(10, 0)).expect("pass").expect("lock").len(),
        1
    );
}

#[test]
fn queued_runs_are_recorded_and_removal_deletes_them() {
    let temp = tempfile::tempdir().expect("tempdir");
    let dir = store_dir(temp.path());
    make_interval(&dir, "inbox-check");
    run_due(&dir, at(10, 0)).expect("pass").expect("lock");
    run_due(&dir, at(10, 30)).expect("pass").expect("lock");

    let runs = history(&dir, "inbox-check").expect("history");
    assert_eq!(runs.len(), 2);
    assert!(runs.iter().all(|r| r.status == "queued"));

    remove(&dir, "inbox-check").expect("remove");
    assert!(load(&dir, "inbox-check").is_err());
    assert!(
        !runs_for(&dir, "inbox-check").exists(),
        "history goes with the automation"
    );
}

#[test]
fn a_held_lock_makes_the_pass_skip_without_queuing() {
    let temp = tempfile::tempdir().expect("tempdir");
    let dir = store_dir(temp.path());
    make_interval(&dir, "inbox-check");
    fs::write(dir.join(LOCK_FILE), "").expect("write lock");
    assert!(run_due(&dir, at(10, 0)).expect("pass").is_none());
    assert_eq!(history(&dir, "inbox-check").expect("history").len(), 0);
}

#[test]
fn a_missing_store_has_nothing_due() {
    let temp = tempfile::tempdir().expect("tempdir");
    let dir = store_dir(temp.path());
    assert_eq!(run_due(&dir, at(10, 0)).expect("pass"), Some(Vec::new()));
    assert!(list(&dir).expect("list").is_empty());
}

#[test]
fn list_sorts_by_name_and_ignores_lock_and_history_files() {
    let temp = tempfile::tempdir().expect("tempdir");
    let dir = store_dir(temp.path());
    make_interval(&dir, "zeta");
    make_interval(&dir, "alpha");
    run_due(&dir, at(10, 0)).expect("pass").expect("lock");
    let names: Vec<String> = list(&dir)
        .expect("list")
        .into_iter()
        .map(|a| a.name)
        .collect();
    assert_eq!(names, vec!["alpha".to_owned(), "zeta".to_owned()]);
}
