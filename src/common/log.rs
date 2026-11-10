//! Leveled, colored, timestamped logging.

use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

const RED: &str = "\x1b[0;31m";
const GREEN: &str = "\x1b[0;32m";
const YELLOW: &str = "\x1b[1;33m";
const BLUE: &str = "\x1b[0;34m";
const GRAY: &str = "\x1b[0;37m";
const RESET: &str = "\x1b[0m";

/// Log severity. `Warning` and `Error` go to stderr, the rest to stdout.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Level {
    Debug,
    Info,
    Success,
    Warning,
    Error,
}

impl Level {
    /// Label printed in the bracketed level field.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Debug => "DEBUG",
            Self::Info => "INFO",
            Self::Success => "SUCCESS",
            Self::Warning => "WARNING",
            Self::Error => "ERROR",
        }
    }

    fn color(self) -> &'static str {
        match self {
            Self::Debug => GRAY,
            Self::Info => BLUE,
            Self::Success => GREEN,
            Self::Warning => YELLOW,
            Self::Error => RED,
        }
    }

    fn goes_to_stderr(self) -> bool {
        matches!(self, Self::Warning | Self::Error)
    }
}

/// Writes log lines to the terminal and, optionally, to a file.
#[derive(Debug, Clone)]
pub struct Logger {
    name: String,
    debug: bool,
    file: Option<PathBuf>,
}

impl Logger {
    /// Creates a logger that tags lines with `name`.
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            debug: false,
            file: None,
        }
    }

    /// Enables `Debug` output.
    #[must_use]
    pub fn with_debug(mut self, debug: bool) -> Self {
        self.debug = debug;
        self
    }

    /// Also appends plain (uncolored) lines to `path`.
    #[must_use]
    pub fn with_file(mut self, path: impl Into<PathBuf>) -> Self {
        self.file = Some(path.into());
        self
    }

    /// Reads `PLAESY_DEBUG` (`"true"` enables debug) and `PLAESY_LOG_FILE`.
    #[must_use]
    pub fn from_env(name: impl Into<String>) -> Self {
        let mut logger =
            Self::new(name).with_debug(std::env::var("PLAESY_DEBUG").is_ok_and(|v| v == "true"));
        if let Some(path) = std::env::var_os("PLAESY_LOG_FILE").filter(|p| !p.is_empty()) {
            logger = logger.with_file(path);
        }
        logger
    }

    /// Logs `message` at `level`. `Debug` is dropped unless debug is enabled.
    pub fn log(&self, level: Level, message: &str) {
        if level == Level::Debug && !self.debug {
            return;
        }
        let timestamp = chrono::Local::now().format("%Y-%m-%d %H:%M:%S");
        let plain = format!(
            "[{timestamp}] [{}] [{}] {message}",
            level.label(),
            self.name
        );
        let colored = format!("{}{plain}{RESET}", level.color());
        // Terminal output is best-effort: a closed pipe must not abort the command.
        let _ = if level.goes_to_stderr() {
            writeln!(io::stderr(), "{colored}")
        } else {
            writeln!(io::stdout(), "{colored}")
        };
        if let Some(path) = &self.file {
            let _ = append_line(path, &plain);
        }
    }

    /// Logs at `Debug`.
    pub fn debug(&self, message: &str) {
        self.log(Level::Debug, message);
    }

    /// Logs at `Info`.
    pub fn info(&self, message: &str) {
        self.log(Level::Info, message);
    }

    /// Logs at `Success`.
    pub fn success(&self, message: &str) {
        self.log(Level::Success, message);
    }

    /// Logs at `Warning`.
    pub fn warning(&self, message: &str) {
        self.log(Level::Warning, message);
    }

    /// Logs at `Error`.
    pub fn error(&self, message: &str) {
        self.log(Level::Error, message);
    }
}

fn append_line(path: &Path, line: &str) -> io::Result<()> {
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        fs::create_dir_all(parent)?;
    }
    let mut file = OpenOptions::new().create(true).append(true).open(path)?;
    writeln!(file, "{line}")
}

#[cfg(test)]
mod tests {
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
}
