use super::*;

fn cargo() -> PathBuf {
    PathBuf::from(env!("CARGO"))
}

#[test]
fn bounded_runtime_native_allowed_program_reports_evidence() {
    let dir = tempfile::tempdir().unwrap();
    let mut adapter = NativeProcessAdapter::new(
        cargo(),
        vec!["--version".into()],
        dir.path(),
        Duration::from_secs(60),
        vec![cargo()],
    )
    .with_external_effects(true);
    match adapter.execute("task-1", &CancelFlag::new()) {
        Outcome::Succeeded { evidence } => {
            assert!(evidence.starts_with("exit=0 stdout_sha256="))
        }
        other => panic!("expected success, got {other:?}"),
    }
}

#[test]
fn bounded_runtime_native_program_outside_allowlist_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let mut adapter = NativeProcessAdapter::new(
        cargo(),
        vec!["--version".into()],
        dir.path(),
        Duration::from_secs(60),
        Vec::new(),
    )
    .with_external_effects(true);
    assert_eq!(
        adapter.execute("task-1", &CancelFlag::new()),
        Outcome::Failed {
            reason: "program_not_allowed".into()
        }
    );
}

#[test]
fn bounded_runtime_native_disabled_by_default() {
    let dir = tempfile::tempdir().unwrap();
    let mut adapter = NativeProcessAdapter::new(
        cargo(),
        vec!["--version".into()],
        dir.path(),
        Duration::from_secs(60),
        vec![cargo()],
    );
    assert_eq!(
        adapter.execute("task-1", &CancelFlag::new()),
        Outcome::Failed {
            reason: "external_effects_disabled".into()
        }
    );
}

#[test]
fn bounded_runtime_native_timeout_kills_process() {
    let dir = tempfile::tempdir().unwrap();
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let mut adapter = NativeProcessAdapter::new(
        cargo(),
        vec!["metadata".into(), "--format-version".into(), "1".into()],
        manifest_dir,
        Duration::from_millis(1),
        vec![cargo()],
    )
    .with_external_effects(true);
    let _ = dir;
    assert_eq!(
        adapter.execute("task-1", &CancelFlag::new()),
        Outcome::TimedOut
    );
}

#[test]
fn bounded_runtime_native_cancel_before_start_spawns_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let mut adapter = NativeProcessAdapter::new(
        cargo(),
        vec!["--version".into()],
        dir.path(),
        Duration::from_secs(60),
        vec![cargo()],
    )
    .with_external_effects(true);
    let cancel = CancelFlag::new();
    cancel.cancel();
    assert_eq!(adapter.execute("task-1", &cancel), Outcome::Cancelled);
}
