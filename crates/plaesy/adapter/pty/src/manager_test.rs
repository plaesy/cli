use super::*;
use crate::spec::MAX_TERMINAL_DIMENSION;

fn config(max_sessions: usize, output_buffer_bytes: usize) -> PtyConfig {
    PtyConfig {
        max_sessions,
        output_buffer_bytes,
    }
}

#[test]
fn default_config_builds_a_manager() {
    assert!(PtyConfig::default().validate().is_ok());
    assert!(PtyManager::new(PtyConfig::default()).is_ok());
}

#[test]
fn session_count_bounds_are_inclusive() {
    assert!(config(1, 1).validate().is_ok());
    assert!(config(MAX_SESSIONS_LIMIT, MAX_OUTPUT_BUFFER_BYTES)
        .validate()
        .is_ok());
}

#[test]
fn out_of_range_config_is_rejected_before_any_session_exists() {
    let cases = [
        config(0, 1024),
        config(MAX_SESSIONS_LIMIT + 1, 1024),
        config(4, 0),
        config(4, MAX_OUTPUT_BUFFER_BYTES + 1),
    ];
    for cfg in cases {
        assert!(
            matches!(PtyManager::new(cfg), Err(PtyError::InvalidConfig(_))),
            "{cfg:?} was accepted"
        );
    }
}

#[test]
fn open_rejects_invalid_command_before_spawning() {
    let manager = PtyManager::new(PtyConfig::default()).unwrap();
    let result = manager.open(&CommandSpec::new("  "), TermSize::default());
    assert!(matches!(
        result,
        Err(PtyError::InvalidCommand("program is empty"))
    ));
}

#[test]
fn open_rejects_invalid_size_before_spawning() {
    let manager = PtyManager::new(PtyConfig::default()).unwrap();
    let result = manager.open(&CommandSpec::new("sh"), TermSize { rows: 0, cols: 80 });
    assert!(matches!(
        result,
        Err(PtyError::InvalidSize { rows: 0, cols: 80 })
    ));
}

#[test]
fn operations_on_unknown_session_report_not_found() {
    let manager = PtyManager::new(PtyConfig::default()).unwrap();
    let id = SessionId::new(7);

    assert!(matches!(
        manager.write(id, b"ls\n"),
        Err(PtyError::SessionNotFound(found)) if found == id
    ));
    assert!(matches!(
        manager.read_output(id),
        Err(PtyError::SessionNotFound(_))
    ));
    assert!(matches!(
        manager.resize(id, TermSize::default()),
        Err(PtyError::SessionNotFound(_))
    ));
    assert!(matches!(
        manager.status(id),
        Err(PtyError::SessionNotFound(_))
    ));
    assert!(matches!(
        manager.close(id),
        Err(PtyError::SessionNotFound(_))
    ));
}

#[test]
fn resize_validates_size_before_looking_up_session() {
    let manager = PtyManager::new(PtyConfig::default()).unwrap();
    let result = manager.resize(
        SessionId::new(1),
        TermSize {
            rows: MAX_TERMINAL_DIMENSION + 1,
            cols: 80,
        },
    );
    assert!(matches!(result, Err(PtyError::InvalidSize { .. })));
}
