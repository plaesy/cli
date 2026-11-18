//! Unit tests for `tests` split out of `driver.rs`.
//! Kept as a separate file so they are easy to find; included via `#[path]`.

use std::sync::{Arc, Mutex};

use super::*;
use crate::loop_run::measure::Measurement;

/// A writer that keeps its bytes so a test can read them afterwards.
#[derive(Clone, Default)]
struct Capture(Arc<Mutex<Vec<u8>>>);

impl Write for Capture {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(buf);
        Ok(buf.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn options(dir: &std::path::Path, step_clean: bool, config: Config) -> (Options, Capture) {
    let out = Capture::default();
    let opt = Options {
        state_path: dir.join("state.json"),
        report_dir: dir.join("reports"),
        config,
        step: Box::new(move |_, _| {
            Ok(StepResult {
                clean: step_clean,
                ..StepResult::default()
            })
        }),
        measure: Box::new(|name| Measurement {
            name: name.to_owned(),
            value: 95.0,
            known: true,
            note: "measured".into(),
        }),
        dirty: None,
        learnings_path: None,
        out: Box::new(out.clone()),
        now: Box::new(Utc::now),
    };
    (opt, out)
}

#[test]
fn clean_cycles_with_met_thresholds_complete_the_run() {
    let dir = tempfile::tempdir().unwrap();
    let config = Config {
        clean_cycles_for_completion: 2,
        ..Config::default()
    };
    let (opt, _out) = options(dir.path(), true, config);
    let mut driver = Driver::new(opt).unwrap();
    driver.run().unwrap();
    assert_eq!(driver.status(), "complete");
    assert!(dir
        .path()
        .join("reports")
        .join(COMPLETION_REPORT_FILE)
        .exists());
}

#[test]
fn consecutive_failures_pause_the_run_with_a_stop_report() {
    let dir = tempfile::tempdir().unwrap();
    let (opt, _out) = options(dir.path(), false, Config::default());
    let mut driver = Driver::new(opt).unwrap();
    driver.run().unwrap();
    assert_eq!(driver.status(), "paused");
    assert!(dir.path().join("reports").join(STOP_REPORT_FILE).exists());
}

#[test]
fn a_completed_run_is_an_exit_success_and_a_paused_run_is_not() {
    assert!(exit_error("complete").is_ok());
    assert!(exit_error("paused")
        .unwrap_err()
        .contains("did not complete"));
}
