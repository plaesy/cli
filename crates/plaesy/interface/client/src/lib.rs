//! Permukaan editor flow visual (ADR 0011).
//!
//! Struktur mengikuti ADR 0011. Modul yang belum diisi pekerjaan lain hanya berupa kerangka.
//! Modul `state` pure Rust: tanpa Dioxus, tanpa I/O.

pub mod adapter;
pub mod features;
pub mod platform;
pub mod shared;
pub mod shell;
pub mod state;
pub mod ui;
