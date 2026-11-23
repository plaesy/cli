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
//! Skeleton only. The Dioxus web editor is added by `high_feature-implement-visual-flow-dioxus-web-editor`.
//! `ui::flow_view` is a framework-free HTML renderer until then.

pub mod ui;

pub mod features;
