//! Sesi terminal PTY lokal (ADR 0003: adapter/pty).
//!
//! `PtyManager` membuka proses di pseudo-terminal (ConPTY di Windows, PTY
//! POSIX di sistem lain) lewat `portable-pty`. Keluaran dibaca di thread
//! latar dan disimpan di buffer berbatas; buffer yang penuh membuang byte
//! tertua dan mencatat jumlah byte yang terbuang.

mod error;
mod manager;
mod output;
mod spec;
mod table;

pub use error::PtyError;
pub use manager::{
    PtyConfig, PtyManager, SessionStatus, MAX_OUTPUT_BUFFER_BYTES, MAX_SESSIONS_LIMIT,
};
pub use output::OutputChunk;
pub use spec::{CommandSpec, TermSize, MAX_TERMINAL_DIMENSION};
pub use table::SessionId;
