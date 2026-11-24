//! Port untuk interpreter flow-ir/1 (ADR 0002). Domain tidak melakukan I/O: setiap efek
//! (memanggil operasi, menunggu persetujuan, memanggil subflow, membaca waktu) lewat trait ini.
//! Implementasi nyata ada di adapter atau di runtime.

use serde_json::Value;
use std::time::Duration;

/// Kategori galat operasi. Hanya `Transient` yang boleh dicoba ulang.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorKind {
    Transient,
    Terminal,
}

impl ErrorKind {
    /// Nama stabil untuk ekspresi `error.kind`.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Transient => "transient",
            Self::Terminal => "terminal",
        }
    }
}

/// Galat dari satu operasi atau langkah.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActionError {
    pub kind: ErrorKind,
    pub code: String,
    pub message: String,
}

impl ActionError {
    #[must_use]
    pub fn terminal(code: &str, message: &str) -> Self {
        Self {
            kind: ErrorKind::Terminal,
            code: code.to_owned(),
            message: message.to_owned(),
        }
    }

    #[must_use]
    pub fn transient(code: &str, message: &str) -> Self {
        Self {
            kind: ErrorKind::Transient,
            code: code.to_owned(),
            message: message.to_owned(),
        }
    }
}

/// Menjalankan satu operasi bernama dengan input bertipe JSON.
pub trait ActionRunner {
    /// # Errors
    ///
    /// Galat dengan kategori. Galat `Transient` boleh dicoba ulang oleh interpreter.
    fn run(&self, operation: &str, input: &Value) -> Result<Value, ActionError>;
}

/// Hasil menunggu persetujuan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApprovalOutcome {
    /// Jumlah persetujuan yang sudah terkumpul mencapai `required_events`.
    Approved {
        events: u64,
    },
    Rejected,
    TimedOut,
}

/// Kanal persetujuan (ADR 0002: kanal utama adalah API dan halaman web).
pub trait ApprovalGate {
    /// Menunggu `required` persetujuan untuk `module_id`, paling lama `timeout`.
    fn await_events(&self, module_id: &str, required: u64, timeout: Duration) -> ApprovalOutcome;
}

/// Memanggil flow lain dengan id dan versi.
pub trait SubflowRunner {
    /// # Errors
    ///
    /// Pesan galat dari subflow.
    fn run(&self, flow_id: &str, flow_version: u64, inputs: &Value) -> Result<Value, String>;
}

/// Jam untuk batas waktu dan jeda retry. Sumber waktu dan tidur harus bisa diganti di tes.
pub trait Clock {
    /// Waktu monoton sejak titik awal yang dipilih implementasi.
    fn now(&self) -> Duration;
    /// Menunggu selama `duration`.
    fn sleep(&self, duration: Duration);
}

/// Semua port yang dibutuhkan satu eksekusi flow.
#[derive(Clone, Copy)]
pub struct Ports<'a> {
    pub actions: &'a dyn ActionRunner,
    pub approvals: &'a dyn ApprovalGate,
    pub subflows: &'a dyn SubflowRunner,
    pub clock: &'a dyn Clock,
}
