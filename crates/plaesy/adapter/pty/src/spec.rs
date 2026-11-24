use std::path::PathBuf;

use portable_pty::PtySize;

use crate::error::PtyError;

/// Batas dimensi layar per sisi (baris atau kolom).
pub const MAX_TERMINAL_DIMENSION: u16 = 1000;

/// Perintah yang dijalankan di dalam sesi terminal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandSpec {
    pub program: String,
    pub args: Vec<String>,
    pub cwd: Option<PathBuf>,
}

impl CommandSpec {
    /// Perintah tanpa argumen dan tanpa direktori kerja khusus.
    pub fn new(program: impl Into<String>) -> Self {
        Self {
            program: program.into(),
            args: Vec::new(),
            cwd: None,
        }
    }

    /// Memeriksa perintah sebelum proses dibuat. Tidak menyentuh sistem berkas.
    pub fn validate(&self) -> Result<(), PtyError> {
        if self.program.trim().is_empty() {
            return Err(PtyError::InvalidCommand("program is empty"));
        }
        if has_nul(&self.program) || self.args.iter().any(|arg| has_nul(arg)) {
            return Err(PtyError::InvalidCommand(
                "program or argument contains a NUL byte",
            ));
        }
        if let Some(cwd) = &self.cwd {
            if cwd.as_os_str().is_empty() {
                return Err(PtyError::InvalidCommand("working directory is empty"));
            }
            if has_nul(&cwd.to_string_lossy()) {
                return Err(PtyError::InvalidCommand(
                    "working directory contains a NUL byte",
                ));
            }
        }
        Ok(())
    }
}

/// Ukuran layar terminal dalam sel teks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TermSize {
    pub rows: u16,
    pub cols: u16,
}

impl Default for TermSize {
    fn default() -> Self {
        Self { rows: 24, cols: 80 }
    }
}

impl TermSize {
    pub fn validate(self) -> Result<(), PtyError> {
        let valid = |dim: u16| (1..=MAX_TERMINAL_DIMENSION).contains(&dim);
        if valid(self.rows) && valid(self.cols) {
            Ok(())
        } else {
            Err(PtyError::InvalidSize {
                rows: self.rows,
                cols: self.cols,
            })
        }
    }

    pub(crate) fn to_pty_size(self) -> PtySize {
        PtySize {
            rows: self.rows,
            cols: self.cols,
            pixel_width: 0,
            pixel_height: 0,
        }
    }
}

fn has_nul(s: &str) -> bool {
    s.contains('\0')
}

#[cfg(test)]
#[path = "spec_test.rs"]
mod spec_test;
