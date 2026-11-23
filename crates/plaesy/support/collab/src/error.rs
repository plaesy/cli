//! Kesalahan kolaborasi teks.

use thiserror::Error;

/// Kesalahan yang dikembalikan operasi replika.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum CollabError {
    /// Id peserta kosong.
    #[error("id replika tidak boleh kosong")]
    EmptyReplicaId,
    /// Indeks di luar rentang karakter yang terlihat.
    #[error("indeks {index} di luar rentang (panjang {len})")]
    IndexOutOfRange { index: usize, len: usize },
    /// Jam logis melampaui batas `u64`.
    #[error("jam logis melampaui batas")]
    ClockOverflow,
    /// Operasi dari replika lain tidak konsisten.
    #[error("operasi tidak valid: {0}")]
    InvalidOp(&'static str),
}
