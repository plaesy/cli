//! Unit tests for `tests` split out of `mod.rs`.
//! Kept as a separate file so they are easy to find; included via `#[path]`.

use super::*;
use crate::memory::config::{JobSpec, ScheduleConfig};
use crate::memory::storage::{Database, DatabaseConfig};
use std::sync::Arc;

async fn engine(root: &Path) -> SearchEngine {
    let db = Arc::new(
        Database::new(DatabaseConfig {
            db_path: ":memory:".into(),
            ..DatabaseConfig::default()
        })
        .await
        .expect("db"),
    );
    let e = SearchEngine::new(db, None, crate::memory::config::SearchConfig::default())
        .await
        .expect("engine");
    e.set_watch_root(root.to_path_buf());
    e
}

fn spec(name: &str, every: &str, action: &str) -> JobSpec {
    JobSpec {
        name: name.into(),
        every: every.into(),
        action: action.into(),
    }
}

#[test]
fn intervals_parse_and_bad_ones_are_rejected() {
    assert_eq!(parse_every("30m").unwrap(), Duration::minutes(30));
    assert_eq!(parse_every(" 6h ").unwrap(), Duration::hours(6));
    assert_eq!(parse_every("1d").unwrap(), Duration::days(1));
    for bad in ["", "h", "0h", "-1h", "5x", "1.5h", "ééh"] {
        assert!(parse_every(bad).is_err(), "{bad} should be rejected");
    }
}

#[test]
fn unknown_action_and_duplicate_names_fail_at_config_time() {
    assert!(Action::parse("reindex").is_err());
    let dup = ScheduleConfig {
        jobs: vec![spec("a", "1h", "index"), spec("a", "2h", "hygiene")],
    };
    assert!(jobs_from_config(&dup).is_err());
    let ok = ScheduleConfig {
        jobs: vec![spec("a", "1h", "index"), spec("b", "1d", "missing")],
    };
    assert_eq!(jobs_from_config(&ok).unwrap().len(), 2);
    assert_eq!(
        jobs_from_config(&ScheduleConfig::default()).unwrap()[0].action,
        Action::Index
    );
}

#[test]
fn due_only_after_the_interval() {
    let now = Utc::now();
    assert!(is_due(None, Duration::hours(1), now));
    assert!(!is_due(Some(now), Duration::hours(1), now));
    assert!(is_due(
        Some(now - Duration::hours(1)),
        Duration::hours(1),
        now
    ));
}

#[tokio::test]
async fn once_twice_second_run_is_not_due() {
    let dir = tempfile::tempdir().expect("tmp");
    std::fs::write(dir.path().join("a.md"), "# A\n\nAlpha note.\n").expect("w");
    let e = engine(dir.path()).await;
    let jobs = jobs_from_config(&ScheduleConfig::default()).unwrap();
    let now = Utc::now();
    assert_eq!(
        run_due(&e, dir.path(), &jobs, now, None)
            .await
            .unwrap()
            .len(),
        1
    );
    assert!(run_due(&e, dir.path(), &jobs, now, None)
        .await
        .unwrap()
        .is_empty());
    let later = now + Duration::hours(1) + Duration::seconds(1);
    assert_eq!(
        run_due(&e, dir.path(), &jobs, later, None)
            .await
            .unwrap()
            .len(),
        1
    );
}

#[tokio::test]
async fn failed_job_does_not_stop_others() {
    let dir = tempfile::tempdir().expect("tmp");
    let missing_root = dir.path().join("no-such-vault");
    let e = engine(dir.path()).await;
    let jobs = vec![
        Job {
            name: "broken".into(),
            every: Duration::hours(1),
            action: Action::Index,
        },
        Job {
            name: "audit".into(),
            every: Duration::hours(1),
            action: Action::Hygiene,
        },
    ];
    let log = dir.path().join("logs").join("schedule.log");
    let now = Utc::now();
    let out = run_due(&e, &missing_root, &jobs, now, Some(&log))
        .await
        .unwrap();
    // Both jobs share the root, so both fail; the point is that the second still runs and logs.
    assert_eq!(out.len(), 2);
    assert!(out.iter().all(|o| !o.ok));
    let text = std::fs::read_to_string(&log).unwrap();
    assert_eq!(text.lines().count(), 2);
    assert!(text.contains("job=broken outcome=failed"));
    assert!(text.contains("job=audit outcome=failed"));
    assert!(text.contains("not a directory"));
}

#[test]
fn schedule_log_line_is_one_line() {
    let o = Outcome {
        job: "index".into(),
        ok: true,
        summary: "indexed=1\nunchanged=2".into(),
    };
    let line = o.line(Utc::now());
    assert!(!line.contains('\n'));
    assert!(line.ends_with("indexed=1 unchanged=2"));
}

#[test]
fn schedule_log_appends_and_creates_its_folder() {
    let dir = tempfile::tempdir().expect("tmp");
    let path = dir.path().join("sub").join("schedule.log");
    append_log(&path, "first").unwrap();
    append_log(&path, "second").unwrap();
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "first\nsecond\n");
}

#[tokio::test]
async fn one_line_per_job_per_run() {
    let dir = tempfile::tempdir().expect("tmp");
    std::fs::write(dir.path().join("a.md"), "# A\n\nAlpha.\n").expect("w");
    let e = engine(dir.path()).await;
    let jobs = vec![
        Job {
            name: "index".into(),
            every: Duration::hours(1),
            action: Action::Index,
        },
        Job {
            name: "audit".into(),
            every: Duration::hours(1),
            action: Action::Hygiene,
        },
        Job {
            name: "gaps".into(),
            every: Duration::hours(1),
            action: Action::Missing,
        },
    ];
    let log = dir.path().join("schedule.log");
    run_due(&e, dir.path(), &jobs, Utc::now(), Some(&log))
        .await
        .unwrap();
    let text = std::fs::read_to_string(&log).unwrap();
    assert_eq!(text.lines().count(), jobs.len());
    assert!(text.lines().all(|l| l.contains("outcome=ok")));
}
