//! Unit tests for `tests` split out of `log.rs`.
//! Kept as a separate file so they are easy to find; included via `#[path]`.

use super::*;

#[test]
fn labels_and_streams_match_the_go_logger() {
    assert_eq!(Level::Warning.label(), "WARNING");
    assert!(Level::Error.goes_to_stderr());
    assert!(!Level::Info.goes_to_stderr());
}

#[test]
fn appends_plain_lines_to_the_log_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("nested").join("plaesy.log");
    let logger = Logger::new("test").with_file(&path);
    logger.info("hello");
    logger.error("boom");
    let text = fs::read_to_string(&path).unwrap();
    assert_eq!(text.lines().count(), 2);
    assert!(text.contains("[INFO] [test] hello"));
    assert!(
        !text.contains('\x1b'),
        "file output must not carry color codes"
    );
}

#[test]
fn debug_is_suppressed_unless_enabled() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("d.log");
    Logger::new("t").with_file(&path).debug("hidden");
    assert!(!path.exists());
    Logger::new("t")
        .with_debug(true)
        .with_file(&path)
        .debug("shown");
    assert!(fs::read_to_string(&path).unwrap().contains("shown"));
}
