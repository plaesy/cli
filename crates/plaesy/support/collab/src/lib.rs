//! Kolaborasi lokal untuk catatan teks (ADR 0003: support).
//!
//! Modul murni tanpa I/O. Teks disimpan sebagai RGA (Replicated Growable Array)
//! dengan jam Lamport. Ini implementasi kecil sendiri, bukan CRDT eksternal.

pub mod error;
pub mod rga;

pub use error::CollabError;
pub use rga::{Op, OpId, Replica, ReplicaId};
