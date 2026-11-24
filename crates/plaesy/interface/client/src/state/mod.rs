//! State global bersama (ADR 0011): session, preferensi, bus realtime, dan notifikasi.
//! Semua pure Rust. Tidak ada I/O dan tidak ada ketergantungan ke UI.

pub mod notifications;
pub mod preferences;
pub mod realtime;
pub mod session;
